//! Behavioural tests for the `ingest_osm_pbf` entry point.

use rstest::fixture;
use rstest_bdd_macros::{given, then, when};
use std::{
    cell::{Ref, RefCell},
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempPath;
use wildside_data::{OsmIngestError, OsmIngestReport, ingest_osm_pbf_report};

mod support;

use support::{assert_close, decode_fixture};

#[fixture]
fn fixtures_dir() -> PathBuf {
    support::fixtures_dir()
}

enum FixtureTarget {
    Existing(TempPath),
    Missing(PathBuf),
}

impl FixtureTarget {
    fn path(&self) -> &Path {
        match self {
            FixtureTarget::Existing(temp) => temp.as_ref(),
            FixtureTarget::Missing(path) => path.as_path(),
        }
    }
}

#[fixture]
fn target_fixture() -> RefCell<Option<FixtureTarget>> {
    RefCell::new(None)
}

#[fixture]
fn ingestion_result() -> RefCell<Option<Result<OsmIngestReport, OsmIngestError>>> {
    RefCell::new(None)
}

fn expect_report(
    result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) -> Ref<'_, OsmIngestReport> {
    Ref::map(result.borrow(), |option| {
        option
            .as_ref()
            .expect("ingestion was attempted")
            .as_ref()
            .expect("expected successful ingestion")
    })
}

#[given("a valid PBF file containing 3 nodes, 1 way and 1 relation")]
fn valid_dataset(
    #[from(fixtures_dir)] dir: PathBuf,
    #[from(target_fixture)] target: &RefCell<Option<FixtureTarget>>,
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) {
    let fixture = decode_fixture(&dir, "triangle");
    *target.borrow_mut() = Some(FixtureTarget::Existing(fixture));
    *result.borrow_mut() = None;
}

#[given("a path to a missing PBF file")]
fn missing_dataset(
    #[from(fixtures_dir)] dir: PathBuf,
    #[from(target_fixture)] target: &RefCell<Option<FixtureTarget>>,
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) {
    *target.borrow_mut() = Some(FixtureTarget::Missing(dir.join("missing.osm.pbf")));
    *result.borrow_mut() = None;
}

#[given("a path to a file containing invalid PBF data")]
fn invalid_dataset(
    #[from(fixtures_dir)] dir: PathBuf,
    #[from(target_fixture)] target: &RefCell<Option<FixtureTarget>>,
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) {
    let fixture = decode_fixture(&dir, "invalid");
    *target.borrow_mut() = Some(FixtureTarget::Existing(fixture));
    *result.borrow_mut() = None;
}

#[given("a PBF file containing tourism and historic features")]
fn tagged_dataset(
    #[from(fixtures_dir)] dir: PathBuf,
    #[from(target_fixture)] target: &RefCell<Option<FixtureTarget>>,
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) {
    let fixture = decode_fixture(&dir, "poi_tags");
    *target.borrow_mut() = Some(FixtureTarget::Existing(fixture));
    *result.borrow_mut() = None;
}

#[when("I ingest the PBF file")]
fn ingest_selected(
    #[from(target_fixture)] target: &RefCell<Option<FixtureTarget>>,
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) {
    let outcome = {
        let guard = target.borrow();
        let borrowed = guard.as_ref().expect("target path prepared");
        ingest_osm_pbf_report(borrowed.path())
    };
    *result.borrow_mut() = Some(outcome);
}

#[then("the summary includes 3 nodes, 1 way and 1 relation")]
fn summary_counts(
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) {
    let report = expect_report(result);
    let summary = &report.summary;
    assert_eq!(summary.nodes, 3, "expected three nodes");
    assert_eq!(summary.ways, 1, "expected one way");
    assert_eq!(summary.relations, 1, "expected one relation");
}

#[then("the summary bounding box spans the sample coordinates")]
fn summary_bounds(
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) {
    let summary = &expect_report(result).summary;
    let bounds = summary
        .bounds
        .as_ref()
        .expect("sample data should produce a bounding box");
    let min = bounds.min();
    let max = bounds.max();
    assert!(min.x <= max.x, "min.x must not exceed max.x");
    assert!(min.y <= max.y, "min.y must not exceed max.y");
    assert_close(min.x, 11.62564468943);
    assert_close(max.x, 11.63101926915);
    assert_close(min.y, 52.11989910567);
    assert_close(max.y, 52.12240315616);
}

#[then("the summary includes 3 nodes, 3 ways and 1 relation")]
fn tagged_summary_counts(
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) {
    let summary = &expect_report(result).summary;
    assert_eq!(summary.nodes, 3, "expected three nodes");
    assert_eq!(summary.ways, 3, "expected three ways");
    assert_eq!(summary.relations, 1, "expected one relation");
}

#[then("the report lists 3 points of interest")]
fn poi_count(
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) {
    let report = expect_report(result);
    assert_eq!(report.pois.len(), 3, "expected two nodes and one way");
}

#[then("the POI named \"Museum Island Walk\" uses the first node location")]
fn walkway_location(
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) {
    let report = expect_report(result);
    let walk = report
        .pois
        .iter()
        .find(|poi| poi.tags.get("name") == Some(&"Museum Island Walk".to_string()))
        .expect("expected way POI");
    assert_close(walk.location.x, 13.404954);
    assert_close(walk.location.y, 52.520008);
}

#[then("POIs referencing missing nodes are skipped")]
fn skips_missing_nodes(
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) {
    let report = expect_report(result);
    let ruins = report
        .pois
        .iter()
        .filter(|poi| poi.tags.get("historic") == Some(&"ruins".to_string()))
        .count();
    assert_eq!(ruins, 0, "missing node references should not produce POIs");
}

#[then("an open error is returned")]
fn open_error(
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) {
    let borrowed = result.borrow();
    let outcome = borrowed.as_ref().expect("ingestion was attempted");
    match outcome {
        Ok(_) => panic!("expected an error for the missing file"),
        Err(OsmIngestError::Open { path, .. }) => {
            assert!(
                path.ends_with("missing.osm.pbf"),
                "unexpected path in error: {path:?}"
            );
        }
        Err(other) => panic!("expected an open error, got {other:?}"),
    }
}

#[then("a decode error is returned")]
fn decode_error(
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) {
    let borrowed = result.borrow();
    let outcome = borrowed.as_ref().expect("ingestion was attempted");
    match outcome {
        Ok(_) => panic!("expected an error for the invalid data"),
        Err(OsmIngestError::Decode { source, path }) => {
            let extension = path.extension().and_then(|ext| ext.to_str());
            assert_eq!(extension, Some("pbf"), "unexpected path in error: {path:?}");
            assert!(
                !source.to_string().is_empty(),
                "decode error should preserve the source message"
            );
        }
        Err(other) => panic!("expected a decode error, got {other:?}"),
    }
}

#[test]
fn scenario_indices_follow_feature_order() {
    // rstest-bdd v0.1.0-alpha1 only exposes index-based bindings.
    // Guard the scenario order so edits to the feature file keep indices stable.
    let feature =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/features/ingest_osm_pbf.feature");
    let contents = fs::read_to_string(&feature).unwrap_or_else(|err| {
        panic!("failed to read feature file {feature:?}: {err}");
    });
    let titles: Vec<String> = contents
        .lines()
        .filter_map(|line| line.trim().strip_prefix("Scenario: "))
        .map(|title| title.to_owned())
        .collect();
    let expected = [
        "summarising a known dataset",
        "reporting a missing file",
        "rejecting a corrupted dataset",
        "extracting points of interest from tagged data",
    ];
    assert_eq!(
        titles.len(),
        expected.len(),
        "scenario count changed in feature file: {titles:?}"
    );
    for (index, expected_title) in expected.iter().enumerate() {
        let actual = titles.get(index).map(String::as_str);
        assert_eq!(
            actual,
            Some(*expected_title),
            "scenario at index {index} does not match feature order"
        );
    }
}
