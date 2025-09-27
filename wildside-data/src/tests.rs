use super::*;
use rstest::{fixture, rstest};
use std::path::PathBuf;
use tempfile::TempPath;

mod support {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support.rs"));
}

use support::{assert_close, decode_fixture};

#[fixture]
fn fixtures_dir() -> PathBuf {
    support::fixtures_dir()
}

#[fixture]
fn valid_pbf(#[from(fixtures_dir)] dir: PathBuf) -> TempPath {
    decode_fixture(&dir, "triangle")
}

#[fixture]
fn invalid_pbf(#[from(fixtures_dir)] dir: PathBuf) -> TempPath {
    decode_fixture(&dir, "invalid")
}

#[fixture]
fn poi_pbf(#[from(fixtures_dir)] dir: PathBuf) -> TempPath {
    decode_fixture(&dir, "poi_tags")
}

#[fixture]
fn poi_pbf_with_invalid_coords(#[from(fixtures_dir)] dir: PathBuf) -> TempPath {
    decode_fixture(&dir, "invalid_coords")
}

#[rstest]
fn summarises_small_fixture(valid_pbf: TempPath) -> Result<(), OsmIngestError> {
    let summary = ingest_osm_pbf(valid_pbf.as_ref())?;
    assert_eq!(summary.nodes, 3, "expected three nodes");
    assert_eq!(summary.ways, 1, "expected one way");
    assert_eq!(summary.relations, 1, "expected one relation");

    let bounds = summary.bounds.expect("expected bounds for sample nodes");
    let min = bounds.min();
    let max = bounds.max();
    assert_close(min.x, 11.62564468943);
    assert_close(max.x, 11.63101926915);
    assert_close(min.y, 52.11989910567);
    assert_close(max.y, 52.12240315616);
    Ok(())
}

#[rstest]
fn extracts_relevant_pois(poi_pbf: TempPath) -> Result<(), OsmIngestError> {
    let report = ingest_osm_pbf_report(poi_pbf.as_ref())?;
    assert_eq!(report.summary.nodes, 4, "expected four nodes in fixture");
    assert_eq!(report.summary.ways, 3, "expected three ways in fixture");
    assert_eq!(
        report.summary.relations, 1,
        "expected one relation in fixture"
    );
    assert_eq!(
        report.pois.len(),
        4,
        "expected four POIs (three nodes, one way) to be emitted"
    );

    let names: Vec<&str> = report
        .pois
        .iter()
        .filter_map(|poi| poi.tags.get("name").map(String::as_str))
        .collect();
    assert!(names.contains(&"Brandenburg Gate"));
    assert!(names.contains(&"Pergamon Museum"));
    assert!(names.contains(&"Victory Column"));
    assert!(names.contains(&"Museum Island Walk"));

    let dual_tag = report
        .pois
        .iter()
        .find(|poi| poi.tags.get("name").map(String::as_str) == Some("Victory Column"))
        .expect("dual-tag POI should be present");
    assert_eq!(
        dual_tag.tags.get("historic").map(String::as_str),
        Some("monument")
    );
    assert_eq!(
        dual_tag.tags.get("tourism").map(String::as_str),
        Some("attraction")
    );

    let walk = report
        .pois
        .iter()
        .find(|poi| poi.tags.get("name").map(String::as_str) == Some("Museum Island Walk"))
        .expect("way POI should be present");
    assert_eq!(
        walk.tags.get("tourism").map(String::as_str),
        Some("attraction")
    );
    assert_close(walk.location.x, 13.404954);
    assert_close(walk.location.y, 52.520008);

    let ruins_count = report
        .pois
        .iter()
        .filter(|poi| poi.tags.get("historic").map(String::as_str) == Some("ruins"))
        .count();
    assert_eq!(
        ruins_count, 0,
        "ways without resolvable nodes should be ignored"
    );
    Ok(())
}

#[rstest]
fn skips_pois_with_invalid_coordinates(
    poi_pbf_with_invalid_coords: TempPath,
) -> Result<(), OsmIngestError> {
    let report = ingest_osm_pbf_report(poi_pbf_with_invalid_coords.as_ref())?;
    assert_eq!(report.summary.nodes, 4, "expected four nodes in fixture");
    assert_eq!(report.summary.ways, 1, "expected one way in fixture");
    assert_eq!(
        report.summary.relations, 0,
        "expected no relations in fixture",
    );
    assert_eq!(
        report.pois.len(),
        2,
        "expected only valid POIs (one node, one way) to be emitted",
    );

    let names: Vec<&str> = report
        .pois
        .iter()
        .filter_map(|poi| poi.tags.get("name").map(String::as_str))
        .collect();
    assert!(
        !names.contains(&"Imaginary Summit"),
        "invalid latitude should skip the POI",
    );
    assert!(
        !names.contains(&"Far East Museum"),
        "invalid longitude should skip the POI",
    );
    assert!(
        names.contains(&"Valid Landmark"),
        "valid POI should be included"
    );
    assert!(
        names.contains(&"Invalid Loop"),
        "way POI should be anchored to a valid node",
    );

    let way = report
        .pois
        .iter()
        .find(|poi| poi.tags.get("name").map(String::as_str) == Some("Invalid Loop"))
        .expect("expected way POI");
    assert_close(way.location.x, -0.12);
    assert_close(way.location.y, 51.5);

    Ok(())
}

#[rstest]
fn propagates_open_error(#[from(fixtures_dir)] dir: PathBuf) {
    let missing = dir.join("missing.osm.pbf");
    let err = ingest_osm_pbf(&missing).expect_err("expected failure for missing file");
    match err {
        OsmIngestError::Open { path, .. } => assert_eq!(path, missing),
        other => panic!("expected open error, got {other:?}"),
    }
}

#[rstest]
fn rejects_invalid_payload(invalid_pbf: TempPath) {
    let err = ingest_osm_pbf(invalid_pbf.as_ref())
        .expect_err("expected failure when decoding invalid data");
    match err {
        OsmIngestError::Decode { source, path } => {
            let extension = path.extension().and_then(|ext| ext.to_str());
            assert_eq!(extension, Some("pbf"), "unexpected path in error: {path:?}");
            assert!(
                !source.to_string().is_empty(),
                "decode error should preserve the source message"
            );
        }
        other => panic!("expected decode error, got {other:?}"),
    }
}
