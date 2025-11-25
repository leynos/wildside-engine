//! Behaviour-driven step definitions driving the ingest CLI scenarios.

use super::helpers::{DatasetFiles, LayerOverrides, merge_layers};
use super::*;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

/// Aggregates ingest CLI scenario state so each step only needs a single world
/// argument, keeping clippy's arity checks satisfied and the fixtures readable.
#[derive(Debug)]
struct IngestWorld {
    dataset_files: DatasetFiles,
    cli_args: RefCell<Vec<String>>,
    cli_result: RefCell<Option<Result<IngestConfig, CliError>>>,
    config_layer: RefCell<Option<LayerOverrides>>,
    env_layer: RefCell<Option<LayerOverrides>>,
}

impl IngestWorld {
    fn new() -> Self {
        Self {
            dataset_files: DatasetFiles::new(),
            cli_args: RefCell::new(Vec::new()),
            cli_result: RefCell::new(None),
            config_layer: RefCell::new(None),
            env_layer: RefCell::new(None),
        }
    }

    fn dataset_files(&self) -> &DatasetFiles {
        &self.dataset_files
    }

    fn cli_args(&self) -> &RefCell<Vec<String>> {
        &self.cli_args
    }

    fn cli_result(&self) -> &RefCell<Option<Result<IngestConfig, CliError>>> {
        &self.cli_result
    }

    fn config_layer(&self) -> &RefCell<Option<LayerOverrides>> {
        &self.config_layer
    }

    fn env_layer(&self) -> &RefCell<Option<LayerOverrides>> {
        &self.env_layer
    }
}

#[fixture]
fn world() -> IngestWorld {
    IngestWorld::new()
}

#[given("dataset files exist on disk")]
fn dataset_exists(#[from(world)] world: &IngestWorld) {
    let dataset = world.dataset_files();
    assert!(
        dataset.osm().exists(),
        "expected dataset files to exist on disk",
    );
    assert!(
        dataset.wikidata().exists(),
        "expected dataset files to exist on disk",
    );
}

#[given("I pass the dataset file paths with CLI flags")]
fn cli_provides_paths(#[from(world)] world: &IngestWorld) {
    let dataset = world.dataset_files();
    let mut guard = world.cli_args().borrow_mut();
    guard.extend([
        format!("--{ARG_OSM_PBF}"),
        dataset.osm().as_str().to_string(),
        format!("--{ARG_WIKIDATA_DUMP}"),
        dataset.wikidata().as_str().to_string(),
    ]);
}

#[given("I omit all dataset configuration")]
fn omit_configuration(#[from(world)] world: &IngestWorld) {
    world.cli_args().borrow_mut().clear();
    *world.config_layer().borrow_mut() = None;
    *world.env_layer().borrow_mut() = None;
}

#[given("the dataset file paths are provided via a config file")]
fn provided_via_config(#[from(world)] world: &IngestWorld) {
    let dataset = world.dataset_files();
    *world.config_layer().borrow_mut() = Some(LayerOverrides {
        osm_pbf: Some(dataset.config_osm().to_path_buf()),
        wikidata_dump: Some(dataset.config_wikidata().to_path_buf()),
        ..LayerOverrides::default()
    });
}

#[given("the Wikidata path is overridden via environment variables")]
fn wikidata_overridden_by_env(#[from(world)] world: &IngestWorld) {
    let dataset = world.dataset_files();
    *world.env_layer().borrow_mut() = Some(LayerOverrides {
        wikidata_dump: Some(dataset.env_wikidata().to_path_buf()),
        ..LayerOverrides::default()
    });
}

#[given("I pass only the OSM CLI flag")]
fn cli_only_osm(#[from(world)] world: &IngestWorld) {
    let dataset = world.dataset_files();
    let mut guard = world.cli_args().borrow_mut();
    guard.extend([
        format!("--{ARG_OSM_PBF}"),
        dataset.osm().as_str().to_string(),
    ]);
}

#[when("I configure the ingest command")]
fn configure_ingest(#[from(world)] world: &IngestWorld) {
    let mut invocation = vec!["wildside".to_string(), "ingest".to_string()];
    invocation.extend(world.cli_args().borrow().iter().cloned());
    let file_layer = world.config_layer().borrow().clone();
    let env_layer = world.env_layer().borrow().clone();
    let outcome = Cli::try_parse_from(invocation)
        .map_err(CliError::ArgumentParsing)
        .and_then(|cli| match cli.command {
            Command::Ingest(cmd) => {
                if file_layer.is_some() || env_layer.is_some() {
                    merge_layers(cmd, file_layer, env_layer)
                } else {
                    resolve_ingest_config(cmd)
                }
            }
        });
    world.cli_result().replace(Some(outcome));
}

#[then("the ingest plan uses the CLI-provided dataset paths")]
fn plan_uses_cli_paths(#[from(world)] world: &IngestWorld) {
    let borrowed = world.cli_result().borrow();
    let config = borrowed
        .as_ref()
        .expect("result recorded")
        .as_ref()
        .expect("expected success");
    assert_eq!(config.osm_pbf, world.dataset_files().osm().to_path_buf());
    assert_eq!(
        config.wikidata_dump,
        world.dataset_files().wikidata().to_path_buf()
    );
}

#[then("the CLI reports that the \"osm-pbf\" flag is missing")]
fn reports_missing_osm(#[from(world)] world: &IngestWorld) {
    let borrowed = world.cli_result().borrow();
    let error = borrowed
        .as_ref()
        .expect("result recorded")
        .as_ref()
        .expect_err("expected error");
    match error {
        CliError::MissingArgument { field, .. } => assert_eq!(*field, ARG_OSM_PBF),
        other => panic!("unexpected error {other:?}"),
    }
}

#[then("CLI and environment layers override configuration defaults")]
fn precedence_holds(#[from(world)] world: &IngestWorld) {
    let borrowed = world.cli_result().borrow();
    let config = borrowed
        .as_ref()
        .expect("result recorded")
        .as_ref()
        .expect("expected success");
    assert_eq!(config.osm_pbf, world.dataset_files().osm().to_path_buf());
    assert_eq!(
        config.wikidata_dump,
        world.dataset_files().env_wikidata().to_path_buf()
    );
}

macro_rules! register_ingest_scenario {
    ($fn_name:ident, $scenario_title:literal) => {
        #[scenario(path = "tests/features/ingest_command.feature", name = $scenario_title)]
        fn $fn_name(#[from(world)] world: IngestWorld) {
            let _ = world;
        }
    };
}

register_ingest_scenario!(cli_flag_selection, "selecting dataset paths via CLI flags");
register_ingest_scenario!(rejecting_missing_args, "rejecting missing arguments");
register_ingest_scenario!(
    layering_cli_config_env,
    "layering CLI, config file, and environment values"
);
