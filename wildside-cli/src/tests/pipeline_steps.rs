//! Behavioural coverage for the end-to-end ingest pipeline.

#![cfg(feature = "store-sqlite")]

use super::helpers::{decode_pbf_fixture, write_wikidata_dump};
use super::*;
use camino::Utf8PathBuf;
use geo::{Coord, Rect};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use tempfile::TempDir;
use wildside_core::{PoiStore, SqlitePoiStore};

#[derive(Debug)]
struct PipelineWorld {
    workspace: TempDir,
    osm_path: RefCell<Option<Utf8PathBuf>>,
    wikidata_path: RefCell<Option<Utf8PathBuf>>,
    outcome: RefCell<Option<Result<IngestOutcome, CliError>>>,
    output_dir: Utf8PathBuf,
}

impl PipelineWorld {
    fn new() -> Self {
        let workspace = TempDir::new().expect("create workspace");
        let root =
            Utf8PathBuf::from_path_buf(workspace.path().to_path_buf()).expect("utf-8 workspace");
        let output_dir = root.join("artefacts");
        Self {
            workspace,
            osm_path: RefCell::new(None),
            wikidata_path: RefCell::new(None),
            outcome: RefCell::new(None),
            output_dir,
        }
    }

    fn osm_path(&self) -> Utf8PathBuf {
        self.osm_path
            .borrow()
            .as_ref()
            .cloned()
            .expect("OSM path should be initialised")
    }

    fn wikidata_path(&self) -> Utf8PathBuf {
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
    let root =
        Utf8PathBuf::from_path_buf(world.workspace.path().to_path_buf()).expect("utf-8 workspace");
    let osm = decode_pbf_fixture(&root, "poi_tags");
    let wikidata = write_wikidata_dump(&root);
    world.osm_path.replace(Some(osm));
    world.wikidata_path.replace(Some(wikidata));
}

#[given("a valid OSM fixture and a missing Wikidata dump")]
fn missing_wikidata(#[from(pipeline_world)] world: &PipelineWorld) {
    let root =
        Utf8PathBuf::from_path_buf(world.workspace.path().to_path_buf()).expect("utf-8 workspace");
    let osm = decode_pbf_fixture(&root, "poi_tags");
    let missing = root.join("missing.json");
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
    assert!(outcome.pois_db.exists(), "pois.db should exist");
    assert!(outcome.spatial_index.exists(), "pois.rstar should exist");
}

#[then("the spatial index matches the ingested POI count")]
fn index_matches_pois(#[from(pipeline_world)] world: &PipelineWorld) {
    let outcome_borrow = world.outcome.borrow();
    let outcome = outcome_borrow
        .as_ref()
        .expect("outcome should exist")
        .as_ref()
        .expect("pipeline should succeed");
    let store = SqlitePoiStore::open(
        outcome.pois_db.as_std_path(),
        outcome.spatial_index.as_std_path(),
    )
    .expect("open POI store");
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
