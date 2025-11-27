//! Error types raised while computing popularity scores.
#![forbid(unsafe_code)]

use camino::Utf8PathBuf;
use thiserror::Error;

/// Errors raised while computing or persisting popularity scores.
#[derive(Debug, Error)]
pub enum PopularityError {
    /// Opening the `SQLite` database failed.
    #[error("failed to open SQLite database at {path}")]
    OpenDatabase {
        /// Requested database path.
        path: Utf8PathBuf,
        /// Source error from `rusqlite`.
        #[source]
        source: rusqlite::Error,
    },
    /// Preparing or executing a database query failed.
    #[error("failed to query {operation}")]
    Query {
        /// Description of the failed operation.
        operation: &'static str,
        /// Source error from `rusqlite`.
        #[source]
        source: rusqlite::Error,
    },
    /// A POI identifier could not be represented as `u64`.
    #[error("POI id {poi_id} is outside the supported range")]
    PoiIdOutOfRange {
        /// Identifier read from `SQLite`.
        poi_id: i64,
    },
    /// Parsing a POI's tag payload failed.
    #[error("failed to parse tags for POI {poi_id}")]
    ParseTags {
        /// Identifier of the affected POI.
        poi_id: u64,
        /// Source error from `serde_json`.
        #[source]
        source: serde_json::Error,
    },
    /// A sitelink count was present but unusable.
    #[error("sitelink count {raw} for POI {poi_id} is invalid")]
    InvalidSitelinkCount {
        /// Identifier of the affected POI.
        poi_id: u64,
        /// Raw value found in the database.
        raw: i64,
    },
    /// Creating the parent directory for the output file failed.
    #[error("failed to create parent directory {path}")]
    CreateParent {
        /// Path of the directory that could not be created.
        path: Utf8PathBuf,
        /// Source error from std I/O.
        #[source]
        source: std::io::Error,
    },
    /// Writing the popularity artefact failed.
    #[error("failed to write popularity file at {path}")]
    WriteFile {
        /// Target file path.
        path: Utf8PathBuf,
        /// Source error from std I/O.
        #[source]
        source: std::io::Error,
    },
    /// Serialising the scores to `bincode` failed.
    #[error("failed to serialise popularity scores into {path}")]
    Serialise {
        /// Target file path.
        path: Utf8PathBuf,
        /// Source error from `bincode`.
        #[source]
        source: bincode::Error,
    },
}
