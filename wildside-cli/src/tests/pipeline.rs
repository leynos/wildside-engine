//! Pipeline integration tests covering the ingest command flow.

#![cfg(feature = "store-sqlite")]

use super::helpers::{decode_pbf_fixture, write_wikidata_dump};
use super::*;
use crate::is_bz2;
use bzip2::{Compression, write::BzEncoder};
use camino::Utf8PathBuf;
use geo::{Coord, Rect};
use rstest::rstest;
use rusqlite::Connection;
use std::fs;
use std::io::Write;
use tempfile::TempDir;
use wildside_core::{PoiStore, SqlitePoiStore, Tags};

#[rstest]
fn ingest_pipeline_creates_artefacts() {
    let working = TempDir::new().expect("temp dir");
    let workspace =
        Utf8PathBuf::from_path_buf(working.path().to_path_buf()).expect("utf-8 workspace path");
    let osm_path = decode_pbf_fixture(&workspace, "poi_tags");
    let wikidata_path = write_wikidata_dump(&workspace);
    let output_dir = workspace.join("artefacts");

    let args = IngestArgs {
        osm_pbf: Some(osm_path),
        wikidata_dump: Some(wikidata_path),
        output_dir: Some(output_dir.clone()),
    };

    let outcome = run_ingest(args).expect("pipeline should succeed");
    assert!(outcome.pois_db.exists(), "expected pois.db artefact");
    assert!(
        outcome.spatial_index.exists(),
        "expected pois.rstar artefact"
    );
    assert!(outcome.poi_count > 0);

    let store = SqlitePoiStore::open(
        outcome.pois_db.as_std_path(),
        outcome.spatial_index.as_std_path(),
    )
    .expect("open SQLite POI store");
    let bbox = Rect::new(
        Coord {
            x: -180.0,
            y: -90.0,
        },
        Coord { x: 180.0, y: 90.0 },
    );
    let pois: Vec<_> = store.get_pois_in_bbox(&bbox).collect();
    assert_eq!(pois.len(), outcome.poi_count);

    let conn = Connection::open(outcome.pois_db.as_std_path()).expect("open pois.db");
    let persisted_claims: i64 = conn
        .query_row("SELECT COUNT(*) FROM wikidata_entity_claims", [], |row| {
            row.get(0)
        })
        .expect("count persisted claims");
    assert_eq!(
        persisted_claims as usize, outcome.claims_count,
        "claims_count should reflect persisted claims"
    );
}

#[rstest]
fn ingest_errors_when_wikidata_missing() {
    let working = TempDir::new().expect("temp dir");
    let workspace =
        Utf8PathBuf::from_path_buf(working.path().to_path_buf()).expect("utf-8 workspace path");
    let osm_path = decode_pbf_fixture(&workspace, "poi_tags");
    let missing_wikidata = workspace.join("absent.json");

    let args = IngestArgs {
        osm_pbf: Some(osm_path),
        wikidata_dump: Some(missing_wikidata),
        output_dir: Some(workspace.join("artefacts")),
    };

    let err = run_ingest(args).expect_err("missing dump should fail");
    match err {
        CliError::MissingSourceFile { field, .. } => assert_eq!(field, ARG_WIKIDATA_DUMP),
        other => panic!("unexpected error {other:?}"),
    }
}

#[rstest]
fn ingest_pipeline_creates_artefacts_with_bz2_wikidata() {
    let working = TempDir::new().expect("temp dir");
    let workspace =
        Utf8PathBuf::from_path_buf(working.path().to_path_buf()).expect("utf-8 workspace path");
    let osm_path = decode_pbf_fixture(&workspace, "poi_tags");
    let wikidata_plain = write_wikidata_dump(&workspace);

    let bz2_path = workspace.join("wikidata.json.bz2");
    let plain = fs::read(&wikidata_plain).expect("read wikidata dump");
    let file = fs::File::create(&bz2_path).expect("create bz2 file");
    let mut encoder = BzEncoder::new(file, Compression::default());
    encoder.write_all(&plain).expect("compress wikidata");
    encoder.finish().expect("finish compression");

    let output_dir = workspace.join("artefacts");

    let args = IngestArgs {
        osm_pbf: Some(osm_path),
        wikidata_dump: Some(bz2_path),
        output_dir: Some(output_dir.clone()),
    };

    let outcome = run_ingest(args).expect("pipeline should succeed");
    assert!(outcome.pois_db.exists(), "expected pois.db artefact");
    assert!(
        outcome.spatial_index.exists(),
        "expected pois.rstar artefact"
    );
    assert!(outcome.poi_count > 0);

    let store = SqlitePoiStore::open(
        outcome.pois_db.as_std_path(),
        outcome.spatial_index.as_std_path(),
    )
    .expect("open SQLite POI store");
    let bbox = Rect::new(
        Coord {
            x: -180.0,
            y: -90.0,
        },
        Coord { x: 180.0, y: 90.0 },
    );
    let pois: Vec<_> = store.get_pois_in_bbox(&bbox).collect();
    assert_eq!(pois.len(), outcome.poi_count);
}

#[rstest]
fn wikidata_claims_are_extracted_for_linked_entities() {
    let working = TempDir::new().expect("temp dir");
    let workspace =
        Utf8PathBuf::from_path_buf(working.path().to_path_buf()).expect("utf-8 workspace path");
    let wikidata_path = write_wikidata_dump(&workspace);
    let config = IngestConfig {
        osm_pbf: workspace.join("dummy.osm.pbf"),
        wikidata_dump: wikidata_path,
        output_dir: workspace.clone(),
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

#[rstest]
fn wikidata_claims_are_empty_when_no_linked_entities() {
    let working = TempDir::new().expect("temp dir");
    let workspace =
        Utf8PathBuf::from_path_buf(working.path().to_path_buf()).expect("utf-8 workspace path");
    let wikidata_path = write_wikidata_dump(&workspace);
    let config = IngestConfig {
        osm_pbf: workspace.join("dummy.osm.pbf"),
        wikidata_dump: wikidata_path,
        output_dir: workspace.clone(),
    };

    let claims = ingest_wikidata_claims(&config, &[]).expect("extract claims without links");
    assert!(
        claims.is_empty(),
        "expected no claims when POIs contain no wikidata tags"
    );
}

#[test]
fn is_bz2_handles_case_insensitive_extensions() {
    let cases = [
        ("dump.bz2", true),
        ("dump.BZ2", true),
        ("dump.json.bz2", true),
        ("dump.json", false),
        ("dumpbz2", false),
    ];

    for (name, expected) in cases {
        let path = Utf8PathBuf::from(name);
        assert_eq!(is_bz2(&path), expected, "is_bz2({name})");
    }
}
