//! Behavioural coverage for feature-flag fallbacks.

#![cfg(not(feature = "store-sqlite"))]

use super::helpers::write_utf8;
use super::*;
use camino::Utf8PathBuf;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use tempfile::TempDir;

#[derive(Debug)]
struct FeatureFlagWorld {
    workspace: TempDir,
    osm_path: RefCell<Option<Utf8PathBuf>>,
    wikidata_path: RefCell<Option<Utf8PathBuf>>,
    output_dir: Utf8PathBuf,
    outcome: RefCell<Option<Result<IngestOutcome, CliError>>>,
}

impl FeatureFlagWorld {
    fn new() -> Self {
        let workspace = TempDir::new().expect("create workspace");
        let root =
            Utf8PathBuf::from_path_buf(workspace.path().to_path_buf()).expect("utf-8 workspace");
        let output_dir = root.join("artefacts");
        Self {
            workspace,
            osm_path: RefCell::new(None),
            wikidata_path: RefCell::new(None),
            output_dir,
            outcome: RefCell::new(None),
        }
    }

    fn osm_path(&self) -> Utf8PathBuf {
        self.osm_path
            .borrow()
            .as_ref()
            .cloned()
            .expect("OSM path should be set")
    }

    fn wikidata_path(&self) -> Utf8PathBuf {
        self.wikidata_path
            .borrow()
            .as_ref()
            .cloned()
            .expect("Wikidata path should be set")
    }
}

#[fixture]
fn feature_flag_world() -> FeatureFlagWorld {
    FeatureFlagWorld::new()
}

#[given("valid ingest inputs exist")]
fn valid_inputs(#[from(feature_flag_world)] world: &FeatureFlagWorld) {
    let root =
        Utf8PathBuf::from_path_buf(world.workspace.path().to_path_buf()).expect("utf-8 workspace");
    let osm = root.join("fixture.pbf");
    let wikidata = root.join("wikidata.json");
    write_utf8(&osm, b"fixture");
    write_utf8(&wikidata, b"fixture");
    world.osm_path.replace(Some(osm));
    world.wikidata_path.replace(Some(wikidata));
}

#[when("I run the ingest command")]
fn run_ingest_command(#[from(feature_flag_world)] world: &FeatureFlagWorld) {
    let args = IngestArgs {
        osm_pbf: Some(world.osm_path()),
        wikidata_dump: Some(world.wikidata_path()),
        output_dir: Some(world.output_dir.clone()),
    };
    let outcome = run_ingest(args);
    world.outcome.replace(Some(outcome));
}

#[then("the command fails because store-sqlite is disabled")]
fn command_fails_missing_store(#[from(feature_flag_world)] world: &FeatureFlagWorld) {
    let outcome = world.outcome.borrow();
    let error = outcome
        .as_ref()
        .expect("outcome captured")
        .as_ref()
        .expect_err("expected error");
    match error {
        CliError::MissingFeature { feature, action } => {
            assert_eq!(*feature, "store-sqlite");
            assert_eq!(*action, "ingest");
        }
        other => panic!("expected MissingFeature, found {other:?}"),
    }
}

#[scenario(
    path = "tests/features/feature_flags.feature",
    name = "Ingest requires the store-sqlite feature"
)]
fn ingest_requires_store_sqlite(#[from(feature_flag_world)] world: FeatureFlagWorld) {
    let _ = world;
}
