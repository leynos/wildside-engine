//! Behavioural tests for the `ingest_osm_pbf` entry point.

use base64::{Engine as _, engine::general_purpose};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::{
    cell::RefCell,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::{Builder, TempPath};
use wildside_data::{OsmIngestError, OsmIngestSummary, ingest_osm_pbf};

#[fixture]
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn decode_fixture(dir: &Path, stem: &str) -> TempPath {
    let encoded_path = dir.join(format!("{stem}.osm.pbf.b64"));
    let encoded = fs::read_to_string(&encoded_path).unwrap_or_else(|err| {
        panic!("failed to read base64 fixture {encoded_path:?}: {err}");
    });
    let cleaned: String = encoded
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect();
    let decoded = general_purpose::STANDARD
        .decode(cleaned.as_bytes())
        .unwrap_or_else(|err| {
            panic!("failed to decode base64 fixture {encoded_path:?}: {err}");
        });
    let mut tempfile = Builder::new()
        .prefix(stem)
        .suffix(".osm.pbf")
        .tempfile()
        .unwrap_or_else(|err| {
            panic!("failed to create temporary fixture for {stem}: {err}");
        });
    tempfile.write_all(&decoded).unwrap_or_else(|err| {
        panic!("failed to write decoded fixture for {stem}: {err}");
    });
    tempfile.into_temp_path()
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
fn ingestion_result() -> RefCell<Option<Result<OsmIngestSummary, OsmIngestError>>> {
    RefCell::new(None)
}

fn expect_summary(
    result: &RefCell<Option<Result<OsmIngestSummary, OsmIngestError>>>,
) -> OsmIngestSummary {
    result
        .borrow()
        .as_ref()
        .expect("ingestion was attempted")
        .as_ref()
        .expect("expected successful ingestion")
        .clone()
}

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta <= 1.0e-7,
        "expected {expected}, got {actual} (|Δ| = {delta})"
    );
}

#[given("a valid PBF file containing 3 nodes, 1 way and 1 relation")]
fn valid_dataset(
    #[from(fixtures_dir)] dir: PathBuf,
    #[from(target_fixture)] target: &RefCell<Option<FixtureTarget>>,
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestSummary, OsmIngestError>>>,
) {
    let fixture = decode_fixture(&dir, "triangle");
    *target.borrow_mut() = Some(FixtureTarget::Existing(fixture));
    *result.borrow_mut() = None;
}

#[given("a path to a missing PBF file")]
fn missing_dataset(
    #[from(fixtures_dir)] dir: PathBuf,
    #[from(target_fixture)] target: &RefCell<Option<FixtureTarget>>,
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestSummary, OsmIngestError>>>,
) {
    *target.borrow_mut() = Some(FixtureTarget::Missing(dir.join("missing.osm.pbf")));
    *result.borrow_mut() = None;
}

#[given("a path to a file containing invalid PBF data")]
fn invalid_dataset(
    #[from(fixtures_dir)] dir: PathBuf,
    #[from(target_fixture)] target: &RefCell<Option<FixtureTarget>>,
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestSummary, OsmIngestError>>>,
) {
    let fixture = decode_fixture(&dir, "invalid");
    *target.borrow_mut() = Some(FixtureTarget::Existing(fixture));
    *result.borrow_mut() = None;
}

#[when("I ingest the PBF file")]
fn ingest_selected(
    #[from(target_fixture)] target: &RefCell<Option<FixtureTarget>>,
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestSummary, OsmIngestError>>>,
) {
    let outcome = {
        let guard = target.borrow();
        let borrowed = guard.as_ref().expect("target path prepared");
        ingest_osm_pbf(borrowed.path())
    };
    *result.borrow_mut() = Some(outcome);
}

#[then("the summary includes 3 nodes, 1 way and 1 relation")]

fn summary_counts(
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestSummary, OsmIngestError>>>,
) {
    let summary = expect_summary(result);
    assert_eq!(summary.nodes, 3, "expected three nodes");
    assert_eq!(summary.ways, 1, "expected one way");
    assert_eq!(summary.relations, 1, "expected one relation");
}

#[then("the summary bounding box spans the sample coordinates")]
fn summary_bounds(
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestSummary, OsmIngestError>>>,
) {
    let summary = expect_summary(result);
    let bounds = summary
        .bounds
        .as_ref()
        .expect("sample data should produce a bounding box");
    let min = bounds.min();
    let max = bounds.max();
    assert_close(min.x, 11.62564468943);
    assert_close(max.x, 11.63101926915);
    assert_close(min.y, 52.11989910567);
    assert_close(max.y, 52.12240315616);
}

#[then("an open error is returned")]
fn open_error(
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestSummary, OsmIngestError>>>,
) {
    let borrowed = result.borrow();
    let outcome = borrowed.as_ref().expect("ingestion was attempted");
    match outcome {
        Ok(_) => panic!("expected an error for the missing file"),
        Err(OsmIngestError::Open { .. }) => {}
        Err(other) => panic!("expected an open error, got {other:?}"),
    }
}

#[then("a decode error is returned")]
fn decode_error(
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestSummary, OsmIngestError>>>,
) {
    let borrowed = result.borrow();
    let outcome = borrowed.as_ref().expect("ingestion was attempted");
    match outcome {
        Ok(_) => panic!("expected an error for the invalid data"),
        Err(OsmIngestError::Decode { .. }) => {}
        Err(other) => panic!("expected a decode error, got {other:?}"),
    }
}

#[scenario(path = "tests/features/ingest_osm_pbf.feature", index = 0)]
fn summarising_known_dataset(
    fixtures_dir: PathBuf,
    target_fixture: RefCell<Option<FixtureTarget>>,
    ingestion_result: RefCell<Option<Result<OsmIngestSummary, OsmIngestError>>>,
) {
    let _ = (fixtures_dir, target_fixture, ingestion_result);
}

#[scenario(path = "tests/features/ingest_osm_pbf.feature", index = 1)]
fn reporting_missing_files(
    fixtures_dir: PathBuf,
    target_fixture: RefCell<Option<FixtureTarget>>,
    ingestion_result: RefCell<Option<Result<OsmIngestSummary, OsmIngestError>>>,
) {
    let _ = (fixtures_dir, target_fixture, ingestion_result);
}

#[scenario(path = "tests/features/ingest_osm_pbf.feature", index = 2)]
fn rejecting_invalid_payloads(
    fixtures_dir: PathBuf,
    target_fixture: RefCell<Option<FixtureTarget>>,
    ingestion_result: RefCell<Option<Result<OsmIngestSummary, OsmIngestError>>>,
) {
    let _ = (fixtures_dir, target_fixture, ingestion_result);
}
