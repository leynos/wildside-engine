use super::helpers::{DatasetFiles, LayerOverrides, merge_layers};
use super::*;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

#[fixture]
fn dataset_files() -> DatasetFiles {
    DatasetFiles::new()
}

#[fixture]
fn cli_args() -> RefCell<Vec<String>> {
    RefCell::new(Vec::new())
}

#[fixture]
fn cli_result() -> RefCell<Option<Result<IngestConfig, CliError>>> {
    RefCell::new(None)
}

#[fixture]
fn config_layer() -> RefCell<Option<LayerOverrides>> {
    RefCell::new(None)
}

#[fixture]
fn env_layer() -> RefCell<Option<LayerOverrides>> {
    RefCell::new(None)
}

#[given("dataset files exist on disk")]
fn dataset_exists(#[from(dataset_files)] _dataset: &DatasetFiles) {}

#[given("I pass the dataset file paths with CLI flags")]
fn cli_provides_paths(
    #[from(dataset_files)] dataset: &DatasetFiles,
    #[from(cli_args)] args: &RefCell<Vec<String>>,
) {
    let mut guard = args.borrow_mut();
    guard.extend([
        format!("--{ARG_OSM_PBF}"),
        dataset.osm().display().to_string(),
        format!("--{ARG_WIKIDATA_DUMP}"),
        dataset.wikidata().display().to_string(),
    ]);
}

#[given("I omit all dataset configuration")]
fn omit_configuration(
    #[from(cli_args)] args: &RefCell<Vec<String>>,
    #[from(config_layer)] config: &RefCell<Option<LayerOverrides>>,
    #[from(env_layer)] env_layer: &RefCell<Option<LayerOverrides>>,
) {
    args.borrow_mut().clear();
    *config.borrow_mut() = None;
    *env_layer.borrow_mut() = None;
}

#[given("the dataset file paths are provided via a config file")]
fn provided_via_config(
    #[from(dataset_files)] dataset: &DatasetFiles,
    #[from(config_layer)] config: &RefCell<Option<LayerOverrides>>,
) {
    *config.borrow_mut() = Some(LayerOverrides {
        osm_pbf: Some(dataset.config_osm().to_path_buf()),
        wikidata_dump: Some(dataset.config_wikidata().to_path_buf()),
    });
}

#[given("the Wikidata path is overridden via environment variables")]
fn wikidata_overridden_by_env(
    #[from(dataset_files)] dataset: &DatasetFiles,
    #[from(env_layer)] env_layer: &RefCell<Option<LayerOverrides>>,
) {
    *env_layer.borrow_mut() = Some(LayerOverrides {
        wikidata_dump: Some(dataset.env_wikidata().to_path_buf()),
        ..LayerOverrides::default()
    });
}

#[given("I pass only the OSM CLI flag")]
fn cli_only_osm(
    #[from(dataset_files)] dataset: &DatasetFiles,
    #[from(cli_args)] args: &RefCell<Vec<String>>,
) {
    let mut guard = args.borrow_mut();
    guard.extend([
        format!("--{ARG_OSM_PBF}"),
        dataset.osm().display().to_string(),
    ]);
}

#[when("I configure the ingest command")]
fn configure_ingest(
    #[from(cli_args)] args: &RefCell<Vec<String>>,
    #[from(cli_result)] result: &RefCell<Option<Result<IngestConfig, CliError>>>,
    #[from(config_layer)] config: &RefCell<Option<LayerOverrides>>,
    #[from(env_layer)] env_layer: &RefCell<Option<LayerOverrides>>,
) {
    let mut invocation = vec!["wildside".to_string(), "ingest".to_string()];
    invocation.extend(args.borrow().iter().cloned());
    let file_layer = config.borrow().clone();
    let env_layer = env_layer.borrow().clone();
    let outcome = Cli::try_parse_from(invocation)
        .map_err(CliError::ArgumentParsing)
        .and_then(|cli| match cli.command {
            Command::Ingest(cmd) => {
                if file_layer.is_some() || env_layer.is_some() {
                    merge_layers(cmd, file_layer, env_layer)
                } else {
                    run_ingest(cmd)
                }
            }
        });
    *result.borrow_mut() = Some(outcome);
}

#[then("the ingest plan uses the CLI-provided dataset paths")]
fn plan_uses_cli_paths(
    #[from(cli_result)] result: &RefCell<Option<Result<IngestConfig, CliError>>>,
    #[from(dataset_files)] dataset: &DatasetFiles,
) {
    let borrowed = result.borrow();
    let config = borrowed
        .as_ref()
        .expect("result recorded")
        .as_ref()
        .expect("expected success");
    assert_eq!(config.osm_pbf, dataset.osm().to_path_buf());
    assert_eq!(config.wikidata_dump, dataset.wikidata().to_path_buf());
}

#[then("the CLI reports that the \"osm-pbf\" flag is missing")]
fn reports_missing_osm(
    #[from(cli_result)] result: &RefCell<Option<Result<IngestConfig, CliError>>>,
) {
    let borrowed = result.borrow();
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
fn precedence_holds(
    #[from(cli_result)] result: &RefCell<Option<Result<IngestConfig, CliError>>>,
    #[from(dataset_files)] dataset: &DatasetFiles,
) {
    let borrowed = result.borrow();
    let config = borrowed
        .as_ref()
        .expect("result recorded")
        .as_ref()
        .expect("expected success");
    assert_eq!(config.osm_pbf, dataset.osm().to_path_buf());
    assert_eq!(config.wikidata_dump, dataset.env_wikidata().to_path_buf());
}

macro_rules! register_ingest_scenario {
    ($fn_name:ident, $scenario_title:literal) => {
        #[scenario(path = "tests/features/ingest_command.feature", name = $scenario_title)]
        fn $fn_name(
            #[from(dataset_files)] dataset: DatasetFiles,
            #[from(cli_args)] args: RefCell<Vec<String>>,
            #[from(cli_result)] result: RefCell<Option<Result<IngestConfig, CliError>>>,
            #[from(config_layer)] config: RefCell<Option<LayerOverrides>>,
            #[from(env_layer)] env_layer: RefCell<Option<LayerOverrides>>,
        ) {
            let _ = dataset;
            let _ = args;
            let _ = result;
            let _ = config;
            let _ = env_layer;
        }
    };
}

register_ingest_scenario!(cli_flag_selection, "selecting dataset paths via CLI flags");
register_ingest_scenario!(rejecting_missing_args, "rejecting missing arguments");
register_ingest_scenario!(
    layering_cli_config_env,
    "layering CLI, config file, and environment values"
);
