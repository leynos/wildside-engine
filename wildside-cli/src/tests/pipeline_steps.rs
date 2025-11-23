//! Behavioural coverage for the end-to-end ingest pipeline.

use super::helpers::{decode_pbf_fixture, write_wikidata_dump};
use super::*;
use geo::{Coord, Rect};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::{cell::RefCell, path::PathBuf};
use tempfile::TempDir;
use wildside_core::{PoiStore, SqlitePoiStore};

#[derive(Debug)]
struct PipelineWorld {
    workspace: TempDir,
    osm_path: RefCell<Option<PathBuf>>,
    wikidata_path: RefCell<Option<PathBuf>>,
    outcome: RefCell<Option<Result<IngestOutcome, CliError>>>,
    output_dir: PathBuf,
}

impl PipelineWorld {
    fn new() -> Self {
        let workspace = TempDir::new().expect("create workspace");
        let output_dir = workspace.path().join("artefacts");
        Self {
            workspace,
            osm_path: RefCell::new(None),
            wikidata_path: RefCell::new(None),
            outcome: RefCell::new(None),
            output_dir,
        }
    }

    fn osm_path(&self) -> PathBuf {
        self.osm_path
            .borrow()
            .as_ref()
            .cloned()
            .expect("OSM path should be initialised")
    }

    fn wikidata_path(&self) -> PathBuf {
        self.wikidata_path
            .borrow()
            .as_ref()
            .cloned()
            .expect("Wikidata path should be initialised")
    }
}

#[fixture]
fn pipeline_world() -> PipelineWorld {
    PipelineWorld::new()
}

#[given("a valid OSM fixture and Wikidata dump")]
fn valid_inputs(#[from(pipeline_world)] world: &PipelineWorld) {
    let osm = decode_pbf_fixture(world.workspace.path(), "poi_tags");
    let wikidata = write_wikidata_dump(world.workspace.path());
    world.osm_path.replace(Some(osm));
    world.wikidata_path.replace(Some(wikidata));
}

#[given("a valid OSM fixture and a missing Wikidata dump")]
fn missing_wikidata(#[from(pipeline_world)] world: &PipelineWorld) {
    let osm = decode_pbf_fixture(world.workspace.path(), "poi_tags");
    let missing = world.workspace.path().join("missing.json");
    world.osm_path.replace(Some(osm));
    world.wikidata_path.replace(Some(missing));
}

#[when("I run the ingest pipeline")]
fn run_pipeline(#[from(pipeline_world)] world: &PipelineWorld) {
    let args = IngestArgs {
        osm_pbf: Some(world.osm_path()),
        wikidata_dump: Some(world.wikidata_path()),
        output_dir: Some(world.output_dir.clone()),
    };
    let outcome = run_ingest(args);
    world.outcome.replace(Some(outcome));
}

#[then("the pois.db and pois.rstar artefacts are created")]
fn artefacts_created(#[from(pipeline_world)] world: &PipelineWorld) {
    let outcome_borrow = world.outcome.borrow();
    let outcome = outcome_borrow
        .as_ref()
        .expect("outcome should exist")
        .as_ref()
        .expect("pipeline should succeed");
    let artefacts = outcome.artefacts();
    assert!(artefacts.pois_db().exists(), "pois.db should exist");
    assert!(
        artefacts.spatial_index().exists(),
        "pois.rstar should exist"
    );
}

#[then("the spatial index matches the ingested POI count")]
fn index_matches_pois(#[from(pipeline_world)] world: &PipelineWorld) {
    let outcome_borrow = world.outcome.borrow();
    let outcome = outcome_borrow
        .as_ref()
        .expect("outcome should exist")
        .as_ref()
        .expect("pipeline should succeed");
    let artefacts = outcome.artefacts();
    let store = SqlitePoiStore::open(artefacts.pois_db(), artefacts.spatial_index())
        .expect("open POI store");
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

#[then("the CLI reports a missing Wikidata dump")]
fn reports_missing_dump(#[from(pipeline_world)] world: &PipelineWorld) {
    let outcome_borrow = world.outcome.borrow();
    let error = outcome_borrow
        .as_ref()
        .expect("outcome should exist")
        .as_ref()
        .expect_err("pipeline should fail");
    match error {
        CliError::MissingSourceFile { field, .. } => assert_eq!(*field, ARG_WIKIDATA_DUMP),
        other => panic!("unexpected error {other:?}"),
    }
}

#[scenario(
    path = "tests/features/ingest_pipeline.feature",
    name = "building artefacts from valid inputs"
)]
fn build_artefacts(#[from(pipeline_world)] world: PipelineWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/ingest_pipeline.feature",
    name = "failing when the Wikidata dump is missing"
)]
fn missing_dump(#[from(pipeline_world)] world: PipelineWorld) {
    let _ = world;
}
