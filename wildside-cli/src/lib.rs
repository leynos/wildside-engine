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
            // Pipeline wiring pending; validation succeeds but result unused for now.
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
        if path.is_file() {
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
mod tests;
