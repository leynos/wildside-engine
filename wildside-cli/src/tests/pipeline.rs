//! Pipeline integration tests covering the ingest command flow.

use super::helpers::{decode_pbf_fixture, write_wikidata_dump};
use super::*;
use geo::{Coord, Rect};
use rstest::rstest;
use tempfile::TempDir;
use wildside_core::{PoiStore, SqlitePoiStore, Tags};

#[rstest]
fn ingest_pipeline_creates_artefacts() {
    let working = TempDir::new().expect("temp dir");
    let osm_path = decode_pbf_fixture(working.path(), "poi_tags");
    let wikidata_path = write_wikidata_dump(working.path());
    let output_dir = working.path().join("artefacts");

    let args = IngestArgs {
        osm_pbf: Some(osm_path),
        wikidata_dump: Some(wikidata_path),
        output_dir: Some(output_dir.clone()),
    };

    let outcome = run_ingest(args).expect("pipeline should succeed");
    let artefacts = outcome.artefacts();
    assert!(artefacts.pois_db().exists(), "expected pois.db artefact");
    assert!(
        artefacts.spatial_index().exists(),
        "expected pois.rstar artefact"
    );
    assert_eq!(outcome.poi_count(), outcome.index_size());

    let store = SqlitePoiStore::open(artefacts.pois_db(), artefacts.spatial_index())
        .expect("open SQLite POI store");
    let bbox = Rect::new(
        Coord {
            x: -180.0,
            y: -90.0,
        },
        Coord { x: 180.0, y: 90.0 },
    );
    let pois: Vec<_> = store.get_pois_in_bbox(&bbox).collect();
    assert_eq!(pois.len(), outcome.poi_count());
}

#[rstest]
fn ingest_errors_when_wikidata_missing() {
    let working = TempDir::new().expect("temp dir");
    let osm_path = decode_pbf_fixture(working.path(), "poi_tags");
    let missing_wikidata = working.path().join("absent.json");

    let args = IngestArgs {
        osm_pbf: Some(osm_path),
        wikidata_dump: Some(missing_wikidata),
        output_dir: Some(working.path().join("artefacts")),
    };

    let err = run_ingest(args).expect_err("missing dump should fail");
    match err {
        CliError::MissingSourceFile { field, .. } => assert_eq!(field, ARG_WIKIDATA_DUMP),
        other => panic!("unexpected error {other:?}"),
    }
}

#[rstest]
fn wikidata_claims_are_extracted_for_linked_entities() {
    let working = TempDir::new().expect("temp dir");
    let wikidata_path = write_wikidata_dump(working.path());
    let config = IngestConfig {
        osm_pbf: working.path().join("dummy.osm.pbf"),
        wikidata_dump: wikidata_path,
        output_dir: working.path().to_path_buf(),
    };
    let poi = PointOfInterest::new(
        7,
        Coord { x: 1.0, y: 2.0 },
        Tags::from([("wikidata".into(), "Q64".into())]),
    );

    let claims = ingest_wikidata_claims(&config, &[poi]).expect("extract claims");
    assert_eq!(claims.len(), 1, "expected one linked entity");
    assert_eq!(claims[0].entity_id, "Q64");
    assert_eq!(claims[0].linked_poi_ids, vec![7]);
    assert_eq!(claims[0].heritage_designations, vec!["Q9259".to_string()]);
}
