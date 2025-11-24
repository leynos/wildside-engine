//! Command-line interface for Wildside's offline tooling.
#![forbid(unsafe_code)]

use bzip2::read::MultiBzDecoder;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::File};
use clap::{Parser, Subcommand};
use ortho_config::{OrthoConfig, SubcmdConfigMerge};
use serde::{Deserialize, Serialize};
use std::io::BufReader;
use std::sync::Arc;
use thiserror::Error;
use wildside_core::{
    PointOfInterest,
    store::{SpatialIndexWriteError, write_spatial_index},
};
use wildside_data::wikidata::etl::{
    EntityClaims, PoiEntityLinks, WikidataEtlError, extract_linked_entity_claims,
};
use wildside_data::wikidata::store::{PersistClaimsError, persist_claims_to_path};
use wildside_data::{
    OsmIngestError, OsmIngestSummary, PersistPoisError, ingest_osm_pbf_report,
    persist_pois_to_sqlite,
};

const ARG_OSM_PBF: &str = "osm-pbf";
const ARG_WIKIDATA_DUMP: &str = "wikidata-dump";
const ARG_OUTPUT_DIR: &str = "output-dir";
const ENV_OSM_PBF: &str = "WILDSIDE_CMDS_INGEST_OSM_PBF";
const ENV_WIKIDATA_DUMP: &str = "WILDSIDE_CMDS_INGEST_WIKIDATA_DUMP";

/// Run the Wildside CLI with the current process arguments and environment.
pub fn run() -> Result<(), CliError> {
    let cli = Cli::try_parse().map_err(CliError::ArgumentParsing)?;
    match cli.command {
        Command::Ingest(args) => {
            let _outcome = run_ingest(args)?;
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
        index_size: report.pois.len(),
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
    extract_linked_entity_claims(reader, &links).map_err(CliError::WikidataEtl)
}

fn open_wikidata_dump(path: &Utf8Path) -> Result<Box<dyn std::io::Read>, CliError> {
    let file = File::open_ambient(path, ambient_authority()).map_err(|source| {
        CliError::OpenWikidataDump {
            path: path.to_path_buf(),
            source,
        }
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
    pub index_size: usize,
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
    MissingSourceFile {
        field: &'static str,
        path: Utf8PathBuf,
    },
    /// The output directory exists but is not a directory.
    #[error("output directory {path:?} is not a directory")]
    OutputDirectoryNotDirectory { path: Utf8PathBuf },
    /// OSM ingestion failed.
    #[error("failed to ingest OSM data: {0}")]
    OsmIngest(#[from] OsmIngestError),
    /// Persisting POIs to SQLite failed.
    #[error("failed to persist POIs to {path:?}: {source}")]
    PersistPois {
        path: Utf8PathBuf,
        source: PersistPoisError,
    },
    /// Opening the Wikidata dump failed.
    #[error("failed to open Wikidata dump at {path:?}: {source}")]
    OpenWikidataDump {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Extracting linked claims from the Wikidata dump failed.
    #[error("failed to extract Wikidata claims: {0}")]
    WikidataEtl(#[from] WikidataEtlError),
    /// Persisting Wikidata claims to SQLite failed.
    #[error("failed to persist Wikidata claims into {path:?}: {source}")]
    PersistClaims {
        path: Utf8PathBuf,
        source: PersistClaimsError,
    },
    /// Writing the spatial index artefact failed.
    #[error("failed to write spatial index to {path:?}: {source}")]
    WriteSpatialIndex {
        path: Utf8PathBuf,
        source: SpatialIndexWriteError,
    },
}

#[cfg(test)]
mod tests;
