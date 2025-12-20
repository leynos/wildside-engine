//! Error types emitted by the Wildside CLI.
//!
//! Some variants box their sources to keep `CliError` small enough for the
//! workspace `clippy::result_large_err` lint (many CLI helpers return
//! `Result<_, CliError>`).

use std::sync::Arc;

use camino::Utf8PathBuf;
use thiserror::Error;
use wildside_core::SolveError;
use wildside_core::SolveRequestValidationError;
use wildside_core::store::SpatialIndexWriteError;
use wildside_data::routing::ProviderBuildError;
use wildside_data::wikidata::etl::WikidataEtlError;
use wildside_data::wikidata::store::PersistClaimsError;
use wildside_data::{OsmIngestError, PersistPoisError};
use wildside_scorer::UserRelevanceError;

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
    /// A referenced input path does not exist on disk or is not a file.
    #[error("{field} path {path:?} does not exist or is not a file")]
    MissingSourceFile {
        field: &'static str,
        path: Utf8PathBuf,
    },
    /// The output directory exists but is not a directory.
    #[error("output directory {path:?} is not a directory")]
    OutputDirectoryNotDirectory { path: Utf8PathBuf },
    /// OSM ingestion failed.
    #[error("failed to ingest OSM data: {0}")]
    OsmIngest(#[source] Box<OsmIngestError>),
    /// Persisting POIs to SQLite failed.
    #[error("failed to persist POIs to {path:?}: {source}")]
    PersistPois {
        path: Utf8PathBuf,
        #[source]
        source: Box<PersistPoisError>,
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
    WikidataEtl(#[source] Box<WikidataEtlError>),
    /// Persisting Wikidata claims to SQLite failed.
    #[error("failed to persist Wikidata claims into {path:?}: {source}")]
    PersistClaims {
        path: Utf8PathBuf,
        #[source]
        source: Box<PersistClaimsError>,
    },
    /// Writing the spatial index artefact failed.
    #[error("failed to write spatial index to {path:?}: {source}")]
    WriteSpatialIndex {
        path: Utf8PathBuf,
        #[source]
        source: Box<SpatialIndexWriteError>,
    },
    /// Opening the solve request file failed.
    #[error("failed to open solve request at {path:?}: {source}")]
    OpenSolveRequest {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Solve request JSON could not be decoded.
    #[error("failed to parse solve request JSON at {path:?}: {source}")]
    ParseSolveRequest {
        path: Utf8PathBuf,
        #[source]
        source: serde_json::Error,
    },
    /// The solve request payload failed validation.
    #[error("solve request in {path:?} failed validation: {source}")]
    InvalidSolveRequest {
        path: Utf8PathBuf,
        #[source]
        source: SolveRequestValidationError,
    },
    /// Opening the POI store artefacts failed.
    #[error("failed to open POI store (db {database_path:?}, index {index_path:?}): {source}")]
    OpenPoiStore {
        database_path: Utf8PathBuf,
        index_path: Utf8PathBuf,
        #[source]
        source: Box<wildside_core::SqlitePoiStoreError>,
    },
    /// Constructing the user relevance scorer failed.
    #[error(
        "failed to build scorer (db {database_path:?}, popularity {popularity_path:?}): {source}"
    )]
    BuildScorer {
        database_path: Utf8PathBuf,
        popularity_path: Utf8PathBuf,
        #[source]
        source: Box<UserRelevanceError>,
    },
    /// Constructing the travel time provider failed.
    #[error("failed to build travel time provider for {base_url:?}: {source}")]
    BuildTravelTimeProvider {
        base_url: String,
        #[source]
        source: Box<ProviderBuildError>,
    },
    /// The solver rejected the request.
    #[error("solver failed: {source}")]
    Solve { source: SolveError },
    /// Serializing the solve response failed.
    #[error("failed to serialise solve response: {0}")]
    SerialiseSolveResponse(#[from] serde_json::Error),
    /// Writing the solve output failed.
    #[error("failed to write solve output: {0}")]
    WriteSolveOutput(#[source] std::io::Error),
}

impl From<OsmIngestError> for CliError {
    fn from(source: OsmIngestError) -> Self {
        Self::OsmIngest(Box::new(source))
    }
}

impl From<WikidataEtlError> for CliError {
    fn from(source: WikidataEtlError) -> Self {
        Self::WikidataEtl(Box::new(source))
    }
}
