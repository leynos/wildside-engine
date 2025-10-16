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
    path::{Path, PathBuf},
};
use thiserror::Error;
use tokio_util::io::{StreamReader, SyncIoBridge};

mod log;

pub use log::DownloadLog;

pub const DEFAULT_USER_AGENT: &str = "wildside-wikidata-etl/0.1";
const STATUS_PATH: &str = "/wikidatawiki/entities/dumpstatus.json";
const JSON_DUMP_SUFFIX: &str = "-all.json.bz2";

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
    /// Recording metadata in the download log failed.
    #[error("failed to record download metadata: {source}")]
    RecordLog { source: rusqlite::Error },
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
    pub file_name: String,
    /// Absolute download URL.
    pub url: String,
    /// Archive size in bytes (if present in the manifest).
    pub size: Option<u64>,
    /// SHA-1 checksum reported by the manifest.
    pub sha1: Option<String>,
}

impl DumpDescriptor {
    fn from_manifest_entry(file_name: &str, entry: &DumpFile, base_url: &str) -> Option<Self> {
        let relative = entry.url.as_deref()?;
        let url = normalise_url(base_url, relative);
        Some(Self {
            file_name: file_name.to_owned(),
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
    fn base_url(&self) -> &str;
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
    base_url: String,
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
            base_url: sanitise_base_url(base_url.into()),
            user_agent: DEFAULT_USER_AGENT.to_owned(),
        }
    }

    /// Override the default user agent string.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    fn status_url(&self) -> String {
        format!("{}{}", self.base_url, STATUS_PATH)
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
    fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn fetch_status(&self) -> Result<Box<dyn BufRead + Send>, TransportError> {
        let url = self.status_url();
        let response = self.call(&url).await?;
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
pub async fn download_latest_dump<S: DumpSource + ?Sized>(
    source: &S,
    output_path: &Path,
    log: Option<&DownloadLog>,
) -> Result<DownloadReport, WikidataDumpError> {
    let descriptor = resolve_latest_descriptor(source).await?;
    download_descriptor(source, descriptor, output_path, log).await
}

/// Download a specific dump described by `descriptor`.
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
    base_url: &str,
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

fn sanitise_base_url(url: String) -> String {
    let trimmed = url.trim_end_matches('/');
    if trimmed.is_empty() {
        "https://dumps.wikimedia.org".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn normalise_url(base_url: &str, relative: &str) -> String {
    if relative.starts_with("http://") || relative.starts_with("https://") {
        relative.to_owned()
    } else if relative.starts_with('/') {
        format!("{}{}", base_url, relative)
    } else {
        format!("{}/{relative}", base_url)
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
