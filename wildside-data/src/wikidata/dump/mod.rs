//! Facilities for discovering and downloading Wikidata dump artefacts.
#![forbid(unsafe_code)]

use async_trait::async_trait;
use futures_util::TryStreamExt;
use reqwest::header::USER_AGENT;
use reqwest::{Client, Response};
use serde::Deserialize;
use simd_json::serde::from_reader;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Write},
    ops::Deref,
    path::{Path, PathBuf},
};
use thiserror::Error;
use tokio_util::io::{StreamReader, SyncIoBridge};

mod log;
pub mod test_support;

pub use log::DownloadLog;

pub const DEFAULT_USER_AGENT: &str = "wildside-wikidata-etl/0.1";
const STATUS_PATH: &str = "/wikidatawiki/entities/dumpstatus.json";
const JSON_DUMP_SUFFIX: &str = "-all.json.bz2";

/// Base URL for the Wikidata dump endpoint.
///
/// # Examples
/// ```
/// # use wildside_data::wikidata::dump::BaseUrl;
/// let url = BaseUrl::new("https://dumps.wikimedia.org");
/// assert_eq!(url.as_ref(), "https://dumps.wikimedia.org");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseUrl(String);

impl BaseUrl {
    /// Construct a new [`BaseUrl`] from an owned or borrowed string.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Consume the wrapper and return the inner [`String`].
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<&str> for BaseUrl {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl AsRef<str> for BaseUrl {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for BaseUrl {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// File name reported by the Wikidata dump manifest.
///
/// # Examples
/// ```
/// # use wildside_data::wikidata::dump::DumpFileName;
/// let file = DumpFileName::new("wikidata-2024-01-01-all.json.bz2");
/// assert!(file.as_ref().ends_with(".bz2"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpFileName(String);

impl DumpFileName {
    /// Construct a new [`DumpFileName`] from an owned or borrowed string.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Consume the wrapper and return the inner [`String`].
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<&str> for DumpFileName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl AsRef<str> for DumpFileName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for DumpFileName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Fully qualified URL pointing to a dump artefact.
///
/// # Examples
/// ```
/// # use wildside_data::wikidata::dump::DumpUrl;
/// let url = DumpUrl::new("https://example.test/wikidata.json.bz2");
/// assert!(url.as_ref().starts_with("https://"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpUrl(String);

impl DumpUrl {
    /// Construct a new [`DumpUrl`] from an owned or borrowed string.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Consume the wrapper and return the inner [`String`].
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<&str> for DumpUrl {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl AsRef<str> for DumpUrl {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for DumpUrl {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Errors produced while preparing or downloading a Wikidata dump.
#[derive(Debug, Error)]
pub enum WikidataDumpError {
    /// The dump status manifest could not be fetched.
    #[error("failed to fetch dump status: {source}")]
    StatusFetch { source: TransportError },
    /// The dump archive could not be downloaded.
    #[error("failed to download dump archive: {source}")]
    Download { source: TransportError },
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
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// Transport-level errors encountered while issuing HTTP requests.
#[derive(Debug, Error)]
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

/// Describes the dump artefact that should be downloaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpDescriptor {
    /// The file name as reported by the manifest.
    pub file_name: DumpFileName,
    /// Absolute download URL.
    pub url: DumpUrl,
    /// Archive size in bytes (if present in the manifest).
    pub size: Option<u64>,
    /// SHA-1 checksum reported by the manifest.
    pub sha1: Option<String>,
}

impl DumpDescriptor {
    fn from_manifest_entry(file_name: &str, entry: &DumpFile, base_url: &BaseUrl) -> Option<Self> {
        let relative = entry.url.as_deref()?;
        let url = normalise_url(base_url, relative);
        Some(Self {
            file_name: DumpFileName::from(file_name),
            url,
            size: entry.size,
            sha1: entry.sha1.clone(),
        })
    }
}

/// Summary of the downloaded artefact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadReport {
    /// Descriptor derived from the manifest.
    pub descriptor: DumpDescriptor,
    /// Number of bytes written to disk.
    pub bytes_written: u64,
    /// Final location of the archive.
    pub output_path: PathBuf,
}

/// Source of dump status manifests and archive bytes.
#[async_trait(?Send)]
pub trait DumpSource {
    /// Base URL of the dump endpoint.
    fn base_url(&self) -> &BaseUrl;
    /// Fetch the dump status manifest.
    async fn fetch_status(&self) -> Result<Box<dyn BufRead + Send>, TransportError>;
    /// Stream the archive identified by `url` into `sink`.
    async fn download_archive(
        &self,
        url: &str,
        sink: &mut dyn Write,
    ) -> Result<u64, TransportError>;
}

/// HTTP implementation of [`DumpSource`].
#[derive(Debug)]
pub struct HttpDumpSource {
    client: Client,
    base_url: BaseUrl,
    user_agent: String,
}

impl HttpDumpSource {
    /// Construct an HTTP-backed dump source.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        let client = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(30))
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("client builder only fails with invalid configuration");
        Self {
            client,
            base_url: sanitise_base_url(base_url),
            user_agent: DEFAULT_USER_AGENT.to_owned(),
        }
    }

    /// Override the default user agent string.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    fn status_url(&self) -> DumpUrl {
        DumpUrl::new(format!("{}{}", self.base_url.as_ref(), STATUS_PATH))
    }

    async fn call(&self, url: &str) -> Result<Response, TransportError> {
        self.client
            .get(url)
            .header(USER_AGENT, self.user_agent.as_str())
            .send()
            .await
            .map_err(|err| convert_reqwest_error(err, url))?
            .error_for_status()
            .map_err(|err| convert_reqwest_error(err, url))
    }
}

#[async_trait(?Send)]
impl DumpSource for HttpDumpSource {
    fn base_url(&self) -> &BaseUrl {
        &self.base_url
    }

    async fn fetch_status(&self) -> Result<Box<dyn BufRead + Send>, TransportError> {
        let url = self.status_url();
        let response = self.call(url.as_ref()).await?;
        Ok(to_blocking_reader(response))
    }

    async fn download_archive(
        &self,
        url: &str,
        sink: &mut dyn Write,
    ) -> Result<u64, TransportError> {
        let response = self.call(url).await?;
        let mut reader = to_sync_reader(response);
        io::copy(&mut reader, sink).map_err(|source| TransportError::Network {
            url: url.to_owned(),
            source,
        })
    }
}

fn to_blocking_reader(response: Response) -> Box<dyn BufRead + Send> {
    Box::new(BufReader::new(into_blocking_stream(response)))
}

fn to_sync_reader(response: Response) -> Box<dyn Read + Send> {
    Box::new(into_blocking_stream(response))
}

fn into_blocking_stream(response: Response) -> impl Read + Send {
    let stream = response.bytes_stream().map_err(io::Error::other);
    SyncIoBridge::new(StreamReader::new(stream))
}

fn convert_reqwest_error(error: reqwest::Error, url: &str) -> TransportError {
    if let Some(status) = error.status() {
        return TransportError::Http {
            url: url.to_owned(),
            status: status.as_u16(),
            message: error.to_string(),
        };
    }

    let kind = if error.is_timeout() {
        io::ErrorKind::TimedOut
    } else {
        io::ErrorKind::Other
    };
    TransportError::Network {
        url: url.to_owned(),
        source: io::Error::new(kind, error),
    }
}

/// Download the latest Wikidata dump using the supplied source.
///
/// Provide a [`DownloadLog`] to persist audit entries; pass `None` to skip
/// logging when durability is unnecessary.
///
/// # Examples
/// ```
/// # use tempfile::tempdir;
/// # use wildside_data::wikidata::dump::{
/// #     download_latest_dump, test_support::StubSource, BaseUrl, DownloadLog,
/// #     WikidataDumpError,
/// # };
/// # fn example() -> Result<(), WikidataDumpError> {
/// let manifest = br#"{
///     "jobs": {
///         "json": {
///             "status": "done",
///             "files": {
///                 "wikidata-2024-01-01-all.json.bz2": {
///                     "url": "https://example.org/wikidata-2024-01-01-all.json.bz2",
///                     "size": 3
///                 }
///             }
///         }
///     }
/// }"#.to_vec();
/// let archive = b"etl".to_vec();
/// let expected_bytes = archive.len() as u64;
/// let source = StubSource::new(
///     BaseUrl::from("https://example.org"),
///     manifest,
///     archive,
/// );
/// let temp = tempdir().expect("create temp directory");
/// let output_path = temp.path().join("wikidata.json.bz2");
/// let log_path = temp.path().join("downloads.sqlite");
/// let log = DownloadLog::initialise(log_path.as_path())?;
/// let report = tokio::runtime::Runtime::new()
///     .expect("create Tokio runtime")
///     .block_on(async {
///         download_latest_dump(&source, output_path.as_path(), Some(&log)).await
///     })?;
/// assert_eq!(report.bytes_written, expected_bytes);
/// assert_eq!(report.output_path, output_path);
/// # Ok(())
/// # }
/// ```
pub async fn download_latest_dump<S: DumpSource + ?Sized>(
    source: &S,
    output_path: &Path,
    log: Option<&DownloadLog>,
) -> Result<DownloadReport, WikidataDumpError> {
    let descriptor = resolve_latest_descriptor(source).await?;
    download_descriptor(source, descriptor, output_path, log).await
}

/// Download a specific dump described by `descriptor`.
///
/// Supplying a [`DownloadLog`] captures a durable record of the download while
/// allowing callers to opt out by passing `None`.
///
/// # Examples
/// ```
/// # use tempfile::tempdir;
/// # use wildside_data::wikidata::dump::{
/// #     download_descriptor, test_support::StubSource, BaseUrl, DownloadLog,
/// #     DumpDescriptor, DumpFileName, DumpUrl, WikidataDumpError,
/// # };
/// # fn example() -> Result<(), WikidataDumpError> {
/// let manifest = br#"{
///     "jobs": {
///         "json": {
///             "status": "done",
///             "files": {
///                 "wikidata-2024-01-01-all.json.bz2": {
///                     "url": "https://example.org/wikidata-2024-01-01-all.json.bz2"
///                 }
///             }
///         }
///     }
/// }"#.to_vec();
/// let archive = b"etl".to_vec();
/// let expected_bytes = archive.len() as u64;
/// let descriptor = DumpDescriptor {
///     file_name: DumpFileName::new("wikidata-2024-01-01-all.json.bz2"),
///     url: DumpUrl::new("https://example.org/wikidata-2024-01-01-all.json.bz2"),
///     size: Some(expected_bytes),
///     sha1: None,
/// };
/// let source = StubSource::new(
///     BaseUrl::from("https://example.org"),
///     manifest,
///     archive,
/// );
/// let temp = tempdir().expect("create temp directory");
/// let output_path = temp.path().join("wikidata.json.bz2");
/// let log_path = temp.path().join("downloads.sqlite");
/// let log = DownloadLog::initialise(log_path.as_path())?;
/// let report = tokio::runtime::Runtime::new()
///     .expect("create Tokio runtime")
///     .block_on(async move {
///         download_descriptor(&source, descriptor.clone(), output_path.as_path(), Some(&log)).await
///     })?;
/// assert_eq!(report.bytes_written, expected_bytes);
/// assert_eq!(report.output_path, output_path);
/// assert_eq!(report.descriptor, descriptor);
/// # Ok(())
/// # }
/// ```
pub async fn download_descriptor<S: DumpSource + ?Sized>(
    source: &S,
    descriptor: DumpDescriptor,
    output_path: &Path,
    log: Option<&DownloadLog>,
) -> Result<DownloadReport, WikidataDumpError> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| WikidataDumpError::CreateDir {
            source,
            path: parent.to_path_buf(),
        })?;
    }
    let mut file = File::create(output_path).map_err(|source| WikidataDumpError::WriteDump {
        source,
        path: output_path.to_path_buf(),
    })?;
    let bytes_written = source
        .download_archive(&descriptor.url, &mut file)
        .await
        .map_err(|source| WikidataDumpError::Download { source })?;
    file.flush()
        .map_err(|source| WikidataDumpError::WriteDump {
            source,
            path: output_path.to_path_buf(),
        })?;
    if let Some(expected) = descriptor.size
        && expected != bytes_written
    {
        return Err(WikidataDumpError::SizeMismatch {
            expected,
            actual: bytes_written,
        });
    }
    let report = DownloadReport {
        descriptor,
        bytes_written,
        output_path: output_path.to_path_buf(),
    };
    if let Some(log) = log {
        log.record(&report)?;
    }
    Ok(report)
}

/// Resolve the descriptor describing the latest available dump archive.
///
/// This helper streams the manifest and applies the JSON dump heuristics used
/// by [`download_latest_dump`].
///
/// # Examples
/// ```
/// # use wildside_data::wikidata::dump::{
/// #     resolve_latest_descriptor, test_support::StubSource, BaseUrl,
/// #     WikidataDumpError,
/// # };
/// # fn example() -> Result<(), WikidataDumpError> {
/// let manifest = br#"{
///     "jobs": {
///         "json": {
///             "status": "done",
///             "files": {
///                 "wikidata-2024-01-01-all.json.bz2": {
///                     "url": "https://example.org/wikidata-2024-01-01-all.json.bz2",
///                     "size": 3
///                 }
///             }
///         }
///     }
/// }"#.to_vec();
/// let source = StubSource::new(
///     BaseUrl::from("https://example.org"),
///     manifest,
///     b"etl".to_vec(),
/// );
/// let descriptor = tokio::runtime::Runtime::new()
///     .expect("create Tokio runtime")
///     .block_on(async move { resolve_latest_descriptor(&source).await })?;
/// assert_eq!(descriptor.file_name.as_ref(), "wikidata-2024-01-01-all.json.bz2");
/// # Ok(())
/// # }
/// ```
pub async fn resolve_latest_descriptor<S: DumpSource + ?Sized>(
    source: &S,
) -> Result<DumpDescriptor, WikidataDumpError> {
    let mut manifest = source
        .fetch_status()
        .await
        .map_err(|source| WikidataDumpError::StatusFetch { source })?;
    select_dump(manifest.as_mut(), source.base_url())
}

fn select_dump(
    manifest_reader: &mut dyn BufRead,
    base_url: &BaseUrl,
) -> Result<DumpDescriptor, WikidataDumpError> {
    let status: DumpStatus = from_reader(manifest_reader)
        .map_err(|source| WikidataDumpError::ParseManifest { source })?;
    status
        .jobs
        .values()
        .filter(|job| job.is_done())
        .flat_map(|job| job.files.iter())
        .find_map(|(file_name, entry)| {
            if file_name.ends_with(JSON_DUMP_SUFFIX) {
                DumpDescriptor::from_manifest_entry(file_name, entry, base_url)
            } else {
                None
            }
        })
        .ok_or(WikidataDumpError::MissingDump)
}

fn sanitise_base_url(url: impl Into<String>) -> BaseUrl {
    let raw = url.into();
    let trimmed = raw.trim_end_matches('/');
    if trimmed.is_empty() {
        BaseUrl::from("https://dumps.wikimedia.org")
    } else {
        BaseUrl::new(trimmed.to_owned())
    }
}

fn normalise_url(base_url: &BaseUrl, relative: &str) -> DumpUrl {
    if relative.starts_with("http://") || relative.starts_with("https://") {
        DumpUrl::from(relative)
    } else if relative.starts_with('/') {
        DumpUrl::new(format!("{}{}", base_url.as_ref(), relative))
    } else {
        DumpUrl::new(format!("{}/{relative}", base_url.as_ref()))
    }
}

#[derive(Debug, Deserialize)]
struct DumpStatus {
    jobs: HashMap<String, DumpJob>,
}

#[derive(Debug, Deserialize)]
struct DumpJob {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    files: HashMap<String, DumpFile>,
}

impl DumpJob {
    fn is_done(&self) -> bool {
        self.status
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("done"))
    }
}

#[derive(Debug, Deserialize)]
struct DumpFile {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    sha1: Option<String>,
}

#[cfg(test)]
mod tests;
