//! Behavioural tests for the `ingest_osm_pbf` entry point.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
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

fn store_fixture(
    target: &RefCell<Option<FixtureTarget>>,
    result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
    fixture: FixtureTarget,
) {
    *target.borrow_mut() = Some(fixture);
    *result.borrow_mut() = None;
}

fn load_fixture_by_name(
    dir: PathBuf,
    target: &RefCell<Option<FixtureTarget>>,
    result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
    name: &str,
) {
    let fixture = decode_fixture(&dir, name);
    store_fixture(target, result, FixtureTarget::Existing(fixture));
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

fn expect_error<F>(
    result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
    expectation: &str,
    mut inspect: F,
) where
    F: FnMut(&OsmIngestError),
{
    let borrowed = result.borrow();
    let outcome = borrowed.as_ref().expect("ingestion was attempted");
    match outcome {
        Ok(_) => panic!("expected {expectation}"),
        Err(error) => inspect(error),
    }
}

macro_rules! fixture_given {
    ($fn_name:ident, $annotation:literal, $fixture:literal) => {
        #[given($annotation)]
        fn $fn_name(
            #[from(fixtures_dir)] dir: PathBuf,
            #[from(target_fixture)] target: &RefCell<Option<FixtureTarget>>,
            #[from(ingestion_result)] result: &RefCell<
                Option<Result<OsmIngestReport, OsmIngestError>>,
            >,
        ) {
            load_fixture_by_name(dir, target, result, $fixture);
        }
    };
}

fixture_given!(
    valid_dataset,
    "a valid PBF file containing 3 nodes, 1 way and 1 relation",
    "triangle"
);

#[given("a path to a missing PBF file")]
fn missing_dataset(
    #[from(fixtures_dir)] dir: PathBuf,
    #[from(target_fixture)] target: &RefCell<Option<FixtureTarget>>,
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) {
    store_fixture(
        target,
        result,
        FixtureTarget::Missing(dir.join("missing.osm.pbf")),
    );
}

fixture_given!(
    invalid_dataset,
    "a path to a file containing invalid PBF data",
    "invalid"
);

fixture_given!(
    tagged_dataset,
    "a PBF file containing tourism and historic features",
    "poi_tags"
);

fixture_given!(
    mixed_tag_dataset,
    "a PBF file combining relevant and irrelevant tags",
    "poi_tags"
);

fixture_given!(
    irrelevant_dataset,
    "a PBF file containing only irrelevant tags",
    "irrelevant_tags"
);

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

fn assert_summary_counts(
    result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
    nodes: u64,
    ways: u64,
    relations: u64,
) {
    let summary = &expect_report(result).summary;
    assert_eq!(
        summary.nodes, nodes,
        "expected {} nodes, found {}",
        nodes, summary.nodes
    );
    assert_eq!(
        summary.ways, ways,
        "expected {} ways, found {}",
        ways, summary.ways
    );
    assert_eq!(
        summary.relations, relations,
        "expected {} relations, found {}",
        relations, summary.relations
    );
}

macro_rules! summary_then {
    ($fn_name:ident, $annotation:literal, $nodes:expr, $ways:expr, $relations:expr) => {
        #[then($annotation)]
        fn $fn_name(
            #[from(ingestion_result)] result: &RefCell<
                Option<Result<OsmIngestReport, OsmIngestError>>,
            >,
        ) {
            assert_summary_counts(result, $nodes, $ways, $relations);
        }
    };
}

macro_rules! report_then {
    ($fn_name:ident, $annotation:literal, |$report:ident| $body:block) => {
        #[then($annotation)]
        fn $fn_name(
            #[from(ingestion_result)] result: &RefCell<
                Option<Result<OsmIngestReport, OsmIngestError>>,
            >,
        ) {
            let $report = expect_report(result);
            $body
        }
    };
}

summary_then!(
    summary_counts,
    "the summary includes 3 nodes, 1 way and 1 relation",
    3,
    1,
    1
);

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

summary_then!(
    tagged_summary_counts,
    "the summary includes 4 nodes, 3 ways and 1 relation",
    4,
    3,
    1
);

report_then!(
    poi_count,
    "the report lists 4 points of interest",
    |report| {
        assert_eq!(
            report.pois.len(),
            4,
            "expected four POIs (three nodes, one way)",
        );
    }
);

report_then!(
    ignores_irrelevant_features_in_mixed_dataset,
    "irrelevant features within the dataset are ignored",
    |report| {
        assert!(
            report
                .pois
                .iter()
                .all(|poi| !poi.tags.contains_key("highway")),
            "expected highway-tagged features to be omitted from POIs",
        );
    }
);

report_then!(
    no_points_reported,
    "no points of interest are reported",
    |report| {
        assert!(
            report.pois.is_empty(),
            "expected no points of interest for irrelevant tags, found {}",
            report.pois.len()
        );
    }
);

report_then!(
    walkway_location,
    "the POI named \"Museum Island Walk\" uses the first node location",
    |report| {
        let walk = report
            .pois
            .iter()
            .find(|poi| poi.tags.get("name").map(String::as_str) == Some("Museum Island Walk"))
            .expect("expected way POI");
        assert_close(walk.location.x, 13.404954);
        assert_close(walk.location.y, 52.520008);
    }
);

report_then!(
    skips_missing_nodes,
    "POIs referencing missing nodes are skipped",
    |report| {
        let ruins = report
            .pois
            .iter()
            .filter(|poi| poi.tags.get("historic").map(String::as_str) == Some("ruins"))
            .count();
        assert_eq!(ruins, 0, "missing node references should not produce POIs");
    }
);

#[then("an open error is returned")]
fn open_error(
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) {
    expect_error(
        result,
        "an error for the missing file",
        |error| match error {
            OsmIngestError::Open { path, .. } => {
                assert!(
                    path.ends_with("missing.osm.pbf"),
                    "unexpected path in error: {path:?}"
                );
            }
            other => panic!("expected an open error, got {other:?}"),
        },
    );
}

#[then("a decode error is returned")]
fn decode_error(
    #[from(ingestion_result)] result: &RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
) {
    expect_error(
        result,
        "an error for the invalid data",
        |error| match error {
            OsmIngestError::Decode { source, path } => {
                let extension = path.extension().and_then(|ext| ext.to_str());
                assert_eq!(extension, Some("pbf"), "unexpected path in error: {path:?}");
                assert!(
                    !source.to_string().is_empty(),
                    "decode error should preserve the source message"
                );
            }
            other => panic!("expected a decode error, got {other:?}"),
        },
    );
}

#[test]
fn scenario_indices_follow_feature_order() {
    // rstest-bdd v0.1.0 binds scenarios by index for our macro usage.
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
        "filtering irrelevant features from a mixed dataset",
        "ignoring irrelevant tags",
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

macro_rules! register_ingest_scenario {
    ($name:ident, $index:literal) => {
        #[scenario(path = "tests/features/ingest_osm_pbf.feature", index = $index)]
        fn $name(
            fixtures_dir: PathBuf,
            target_fixture: RefCell<Option<FixtureTarget>>,
            ingestion_result: RefCell<Option<Result<OsmIngestReport, OsmIngestError>>>,
        ) {
            // The scenario macro wires the fixtures to the Given/When/Then steps.
            // Bind the parameters to suppress unused warnings; rstest-bdd drives the
            // step execution.
            let _ = (fixtures_dir, target_fixture, ingestion_result);
        }
    };
}

register_ingest_scenario!(summarising_known_dataset, 0);
register_ingest_scenario!(reporting_missing_files, 1);
register_ingest_scenario!(rejecting_invalid_payloads, 2);
register_ingest_scenario!(extracting_points_of_interest, 3);
register_ingest_scenario!(filtering_irrelevant_features_from_mixed_dataset, 4);
register_ingest_scenario!(ignoring_irrelevant_tags, 5);
