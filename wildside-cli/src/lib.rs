//! Command-line interface for Wildside's offline tooling.
#![forbid(unsafe_code)]

use bzip2::read::MultiBzDecoder;
use camino::{Utf8Path, Utf8PathBuf};
use clap::{Parser, Subcommand};
use ortho_config::{OrthoConfig, SubcmdConfigMerge};
use serde::{Deserialize, Serialize};
use std::io::BufReader;
use wildside_core::{PointOfInterest, store::write_spatial_index};
use wildside_data::wikidata::etl::{EntityClaims, PoiEntityLinks, extract_linked_entity_claims};
use wildside_data::wikidata::store::persist_claims_to_path;
use wildside_data::{OsmIngestSummary, ingest_osm_pbf_report, persist_pois_to_sqlite};
use wildside_fs::open_utf8_file;

mod error;
mod solve;
/// Errors emitted by the Wildside CLI.
pub use error::CliError;

use solve::SolveArgs;
#[cfg(test)]
use solve::{
    SolveConfig, SolveSolverBuilder, config_from_layers_for_test, load_solve_request,
    run_solve_with,
};

const ARG_OSM_PBF: &str = "osm-pbf";
const ARG_WIKIDATA_DUMP: &str = "wikidata-dump";
const ARG_OUTPUT_DIR: &str = "output-dir";
const ENV_OSM_PBF: &str = "WILDSIDE_CMDS_INGEST_OSM_PBF";
const ENV_WIKIDATA_DUMP: &str = "WILDSIDE_CMDS_INGEST_WIKIDATA_DUMP";
const ARG_SOLVE_REQUEST: &str = "request";
const ARG_SOLVE_ARTEFACTS_DIR: &str = "artefacts-dir";
const ARG_SOLVE_POIS_DB: &str = "pois-db";
const ARG_SOLVE_SPATIAL_INDEX: &str = "spatial-index";
const ARG_SOLVE_POPULARITY: &str = "popularity";
const ARG_SOLVE_OSRM_BASE_URL: &str = "osrm-base-url";
const ENV_SOLVE_REQUEST: &str = "WILDSIDE_CMDS_SOLVE_REQUEST_PATH";

/// Run the Wildside CLI with the current process arguments and environment.
pub fn run() -> Result<(), CliError> {
    let cli = Cli::try_parse().map_err(CliError::from)?;
    match cli.command {
        Command::Ingest(args) => {
            let _outcome = run_ingest(args)?;
        }
        Command::Solve(args) => {
            solve::run_solve(args)?;
        }
    }
    Ok(())
}

fn run_ingest(args: IngestArgs) -> Result<IngestOutcome, CliError> {
    let config = resolve_ingest_config(args)?;
    execute_ingest(&config)
}

fn resolve_ingest_config(args: IngestArgs) -> Result<IngestConfig, CliError> {
    let config = args.into_config()?;
    config.validate_sources()?;
    Ok(config)
}

fn execute_ingest(config: &IngestConfig) -> Result<IngestOutcome, CliError> {
    let pois_db = config.output_dir.join("pois.db");
    let spatial_index = config.output_dir.join("pois.rstar");
    let report = ingest_osm_pbf_report(config.osm_pbf.as_std_path())?;

    persist_pois_to_sqlite(&pois_db, &report.pois).map_err(|source| CliError::PersistPois {
        path: pois_db.clone(),
        source,
    })?;

    let claims = ingest_wikidata_claims(config, &report.pois)?;
    persist_claims_to_path(pois_db.as_std_path(), &claims).map_err(|source| {
        CliError::PersistClaims {
            path: pois_db.clone(),
            source,
        }
    })?;

    write_spatial_index(spatial_index.as_std_path(), &report.pois).map_err(|source| {
        CliError::WriteSpatialIndex {
            path: spatial_index.clone(),
            source,
        }
    })?;

    Ok(IngestOutcome {
        pois_db,
        spatial_index,
        poi_count: report.pois.len(),
        claims_count: claims.len(),
        summary: report.summary,
    })
}

fn ingest_wikidata_claims(
    config: &IngestConfig,
    pois: &[PointOfInterest],
) -> Result<Vec<EntityClaims>, CliError> {
    let links = PoiEntityLinks::from_pois(pois.iter());
    if links.is_empty() {
        return Ok(Vec::new());
    }
    let reader = open_wikidata_dump(&config.wikidata_dump)?;
    extract_linked_entity_claims(reader, &links).map_err(CliError::from)
}

fn open_wikidata_dump(path: &Utf8Path) -> Result<Box<dyn std::io::Read>, CliError> {
    let file = open_utf8_file(path).map_err(|source| CliError::OpenWikidataDump {
        path: path.to_path_buf(),
        source,
    })?;
    if is_bz2(path) {
        Ok(Box::new(BufReader::new(MultiBzDecoder::new(file))))
    } else {
        Ok(Box::new(BufReader::new(file)))
    }
}

fn is_bz2(path: &Utf8Path) -> bool {
    path.extension()
        .map(|ext| ext.eq_ignore_ascii_case("bz2"))
        .unwrap_or(false)
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
    /// Solve a tour request using pre-built artefacts.
    Solve(SolveArgs),
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
    osm_pbf: Option<Utf8PathBuf>,
    /// Path to the Wikidata dump file (JSON/BZ2).
    #[arg(long = ARG_WIKIDATA_DUMP, value_name = "path")]
    #[serde(default)]
    wikidata_dump: Option<Utf8PathBuf>,
    /// Directory to write the generated artefacts.
    #[arg(long = ARG_OUTPUT_DIR, value_name = "dir")]
    #[serde(default)]
    output_dir: Option<Utf8PathBuf>,
}

impl IngestArgs {
    fn into_config(self) -> Result<IngestConfig, CliError> {
        let merged = self.load_and_merge().map_err(CliError::Configuration)?;
        IngestConfig::try_from(merged)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IngestConfig {
    osm_pbf: Utf8PathBuf,
    wikidata_dump: Utf8PathBuf,
    output_dir: Utf8PathBuf,
}

impl IngestConfig {
    fn validate_sources(&self) -> Result<(), CliError> {
        Self::require_existing(&self.osm_pbf, ARG_OSM_PBF)?;
        Self::require_existing(&self.wikidata_dump, ARG_WIKIDATA_DUMP)?;
        if self.output_dir.exists() && !self.output_dir.is_dir() {
            return Err(CliError::OutputDirectoryNotDirectory {
                path: self.output_dir.clone(),
            });
        }
        Ok(())
    }

    fn require_existing(path: &Utf8Path, field: &'static str) -> Result<(), CliError> {
        match wildside_fs::file_is_file(path) {
            Ok(true) => Ok(()),
            Ok(false) | Err(_) => Err(CliError::MissingSourceFile {
                field,
                path: path.to_path_buf(),
            }),
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
        let output_dir = args.output_dir.unwrap_or_else(|| Utf8PathBuf::from("."));
        Ok(Self {
            osm_pbf,
            wikidata_dump,
            output_dir,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct IngestOutcome {
    pub pois_db: Utf8PathBuf,
    pub spatial_index: Utf8PathBuf,
    pub poi_count: usize,
    pub claims_count: usize,
    pub summary: OsmIngestSummary,
}

#[cfg(test)]
mod tests;
