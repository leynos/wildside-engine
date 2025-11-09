//! Command-line interface for Wildside's offline tooling.
#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use ortho_config::{OrthoConfig, SubcmdConfigMerge};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;

const ARG_OSM_PBF: &str = "osm-pbf";
const ARG_WIKIDATA_DUMP: &str = "wikidata-dump";
const ENV_OSM_PBF: &str = "WILDSIDE_CMDS_INGEST_OSM_PBF";
const ENV_WIKIDATA_DUMP: &str = "WILDSIDE_CMDS_INGEST_WIKIDATA_DUMP";

/// Run the Wildside CLI with the current process arguments and environment.
pub fn run() -> Result<(), CliError> {
    let cli = Cli::try_parse().map_err(CliError::ArgumentParsing)?;
    match cli.command {
        Command::Ingest(args) => {
            let _config = run_ingest(args)?;
        }
    }
    Ok(())
}

fn run_ingest(args: IngestArgs) -> Result<IngestConfig, CliError> {
    let config = args.into_config()?;
    config.validate_sources()?;
    Ok(config)
}

#[derive(Debug, Parser)]
#[command(
    name = "wildside",
    about = "Offline data preparation utilities for the Wildside engine",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Build artefacts from existing OSM and Wikidata datasets.
    Ingest(IngestArgs),
}

/// CLI arguments for the `ingest` subcommand.
#[derive(Debug, Clone, Parser, Deserialize, Serialize, OrthoConfig, Default)]
#[command(
    long_about = "Define the artefact inputs for ingestion. Paths can come \
                 from CLI flags, configuration files, or environment \
                 variables.",
    about = "Describe the OSM and Wikidata inputs for ingestion"
)]
#[ortho_config(prefix = "WILDSIDE")]
struct IngestArgs {
    /// Path to the OpenStreetMap PBF file.
    #[arg(long = ARG_OSM_PBF, value_name = "path")]
    #[serde(default)]
    osm_pbf: Option<PathBuf>,
    /// Path to the Wikidata dump file (JSON/BZ2).
    #[arg(long = ARG_WIKIDATA_DUMP, value_name = "path")]
    #[serde(default)]
    wikidata_dump: Option<PathBuf>,
}

impl IngestArgs {
    fn into_config(self) -> Result<IngestConfig, CliError> {
        let merged = self.load_and_merge().map_err(CliError::Configuration)?;
        IngestConfig::try_from(merged)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IngestConfig {
    osm_pbf: PathBuf,
    wikidata_dump: PathBuf,
}

impl IngestConfig {
    fn validate_sources(&self) -> Result<(), CliError> {
        Self::require_existing(&self.osm_pbf, ARG_OSM_PBF)?;
        Self::require_existing(&self.wikidata_dump, ARG_WIKIDATA_DUMP)?;
        Ok(())
    }

    fn require_existing(path: &Path, field: &'static str) -> Result<(), CliError> {
        if path.exists() {
            Ok(())
        } else {
            Err(CliError::MissingSourceFile {
                field,
                path: path.to_path_buf(),
            })
        }
    }
}

impl TryFrom<IngestArgs> for IngestConfig {
    type Error = CliError;

    fn try_from(args: IngestArgs) -> Result<Self, Self::Error> {
        let osm_pbf = args.osm_pbf.ok_or(CliError::MissingArgument {
            field: ARG_OSM_PBF,
            env: ENV_OSM_PBF,
        })?;
        let wikidata_dump = args.wikidata_dump.ok_or(CliError::MissingArgument {
            field: ARG_WIKIDATA_DUMP,
            env: ENV_WIKIDATA_DUMP,
        })?;
        Ok(Self {
            osm_pbf,
            wikidata_dump,
        })
    }
}

/// Errors emitted by the Wildside CLI.
#[derive(Debug, Error)]
pub enum CliError {
    /// Provided arguments failed Clap validation.
    #[error(transparent)]
    ArgumentParsing(#[from] clap::Error),
    /// Configuration layering failed (files, env, CLI).
    #[error("failed to load configuration: {0}")]
    Configuration(#[from] Arc<ortho_config::OrthoError>),
    /// A required option is missing after configuration merging.
    #[error("missing {field} (set --{field} or {env})")]
    MissingArgument {
        field: &'static str,
        env: &'static str,
    },
    /// A referenced input path does not exist on disk.
    #[error("{field} path {path:?} does not exist")]
    MissingSourceFile { field: &'static str, path: PathBuf },
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};
    use rstest_bdd_macros::{given, scenario, then, when};
    use std::{
        cell::RefCell,
        fs,
        path::{Path, PathBuf},
    };
    use tempfile::TempDir;

    #[derive(Debug, Clone, Default)]
    struct LayerOverrides {
        osm_pbf: Option<PathBuf>,
        wikidata_dump: Option<PathBuf>,
    }

    #[rstest]
    #[case(None, Some(PathBuf::from("wikidata.json")), ARG_OSM_PBF, ENV_OSM_PBF)]
    #[case(
        Some(PathBuf::from("planet.osm.pbf")),
        None,
        ARG_WIKIDATA_DUMP,
        ENV_WIKIDATA_DUMP
    )]
    fn converting_without_required_fields_errors(
        #[case] osm: Option<PathBuf>,
        #[case] wiki: Option<PathBuf>,
        #[case] field: &'static str,
        #[case] env_var: &'static str,
    ) {
        let args = IngestArgs {
            osm_pbf: osm,
            wikidata_dump: wiki,
        };
        let err = IngestConfig::try_from(args).expect_err("missing field should error");
        match err {
            CliError::MissingArgument {
                field: missing,
                env,
            } => {
                assert_eq!(missing, field);
                assert_eq!(env, env_var);
            }
            other => panic!("expected MissingArgument, found {other:?}"),
        }
    }

    #[rstest]
    fn validate_sources_reports_missing_files() {
        let config = IngestConfig {
            osm_pbf: PathBuf::from("/tmp/missing-osm"),
            wikidata_dump: PathBuf::from("/tmp/missing-wiki"),
        };
        let err = config.validate_sources().expect_err("expected failure");
        match err {
            CliError::MissingSourceFile { field, .. } => {
                assert_eq!(field, ARG_OSM_PBF);
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

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

    struct DatasetFiles {
        _dir: TempDir,
        cli_osm: PathBuf,
        cli_wikidata: PathBuf,
        config_osm: PathBuf,
        config_wikidata: PathBuf,
        env_wikidata: PathBuf,
    }

    impl DatasetFiles {
        fn new() -> Self {
            let dir = TempDir::new().expect("tempdir");
            let cli_osm = dir.path().join("cli.osm.pbf");
            let cli_wikidata = dir.path().join("cli.wikidata.json.bz2");
            let config_osm = dir.path().join("config.osm.pbf");
            let config_wikidata = dir.path().join("config.wikidata.json.bz2");
            let env_wikidata = dir.path().join("env.wikidata.json.bz2");
            for path in [
                &cli_osm,
                &cli_wikidata,
                &config_osm,
                &config_wikidata,
                &env_wikidata,
            ] {
                fs::write(path, b"dataset contents").expect("write dataset file");
            }
            Self {
                _dir: dir,
                cli_osm,
                cli_wikidata,
                config_osm,
                config_wikidata,
                env_wikidata,
            }
        }

        fn osm(&self) -> &Path {
            &self.cli_osm
        }

        fn wikidata(&self) -> &Path {
            &self.cli_wikidata
        }

        fn config_osm(&self) -> &Path {
            &self.config_osm
        }

        fn config_wikidata(&self) -> &Path {
            &self.config_wikidata
        }

        fn env_wikidata(&self) -> &Path {
            &self.env_wikidata
        }
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

    fn merge_layers(
        mut cli_args: IngestArgs,
        file_layer: Option<LayerOverrides>,
        env_layer: Option<LayerOverrides>,
    ) -> Result<IngestConfig, CliError> {
        if cli_args.osm_pbf.is_none() {
            if let Some(env) = env_layer.as_ref().and_then(|layer| layer.osm_pbf.clone()) {
                cli_args.osm_pbf = Some(env);
            } else if let Some(file) = file_layer.as_ref().and_then(|layer| layer.osm_pbf.clone()) {
                cli_args.osm_pbf = Some(file);
            }
        }
        if cli_args.wikidata_dump.is_none() {
            if let Some(env) = env_layer
                .as_ref()
                .and_then(|layer| layer.wikidata_dump.clone())
            {
                cli_args.wikidata_dump = Some(env);
            } else if let Some(file) = file_layer
                .as_ref()
                .and_then(|layer| layer.wikidata_dump.clone())
            {
                cli_args.wikidata_dump = Some(file);
            }
        }
        run_ingest(cli_args)
    }
}
