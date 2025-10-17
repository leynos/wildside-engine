//! Error types produced by the Wikidata dump helpers.

use std::{error::Error as StdError, io, path::PathBuf};

use thiserror::Error;

/// Errors produced while preparing or downloading a Wikidata dump.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WikidataDumpError {
    /// The dump status manifest could not be fetched.
    #[error("failed to fetch dump status: {source}")]
    StatusFetch { source: super::TransportError },
    /// The dump archive could not be downloaded.
    #[error("failed to download dump archive: {source}")]
    Download { source: super::TransportError },
    /// Parsing the manifest failed.
    #[error("failed to parse dump status manifest: {source}")]
    ParseManifest { source: simd_json::Error },
    /// The manifest did not contain a completed dump.
    #[error("manifest did not contain a completed JSON dump")]
    MissingDump,
    /// Preparing the output directory failed.
    #[error("failed to create output directory {path:?}: {source}")]
    CreateDir { source: io::Error, path: PathBuf },
    /// Writing the dump to disk failed.
    #[error("failed to write dump to {path:?}: {source}")]
    WriteDump { source: io::Error, path: PathBuf },
    /// The downloaded archive size did not match the manifest metadata.
    #[error("downloaded size {actual} did not match manifest size {expected}")]
    SizeMismatch { expected: u64, actual: u64 },
    /// Initialising the download log failed.
    #[error("failed to initialise download log at {path:?}: {source}")]
    InitialiseLog {
        source: rusqlite::Error,
        path: PathBuf,
    },
    /// Recording metadata failed when interacting with SQLite.
    #[error("failed to record download metadata: {source}")]
    RecordLogSql { source: rusqlite::Error },
    /// Serialising metadata into SQLite-compatible values failed.
    #[error("failed to prepare download metadata for persistence ({what}): {source}")]
    RecordLogValue {
        /// Description of the value that failed to serialise.
        what: String,
        /// Underlying conversion error.
        source: Box<dyn StdError + Send + Sync>,
    },
}

/// Transport-level errors encountered while issuing HTTP requests.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TransportError {
    /// The server returned an HTTP error status.
    #[error("request to {url} failed with status {status}: {message}")]
    Http {
        /// Fully qualified request URL.
        url: String,
        /// HTTP status code.
        status: u16,
        /// Short error description supplied by the server.
        message: String,
    },
    /// The request failed due to an I/O error.
    #[error("network error contacting {url}: {source}")]
    Network {
        /// Fully qualified request URL.
        url: String,
        /// I/O error reported by the transport.
        source: io::Error,
    },
}
