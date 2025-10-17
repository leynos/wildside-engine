//! Domain wrappers for Wikidata dump endpoints, file names, and descriptors.
//! Provides small, typed newtypes with ergonomic trait impls and Rustdoc examples.

use std::{
    fmt,
    ops::Deref,
    path::{Path, PathBuf},
};

use url::Url;

use super::log::DownloadLog;

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

impl fmt::Display for BaseUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
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

impl fmt::Display for DumpFileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
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

impl fmt::Display for DumpUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<Url> for DumpUrl {
    fn from(value: Url) -> Self {
        Self(value.into())
    }
}

impl TryFrom<&str> for DumpUrl {
    type Error = url::ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Url::parse(value).map(Into::into)
    }
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

/// Options controlling how a dump is materialised on disk.
///
/// Builder helpers provide an ergonomic way to opt into logging and overwriting
/// existing archives without assembling several positional arguments.
///
/// # Examples
/// ```
/// # use tempfile::tempdir;
/// # use wildside_data::wikidata::dump::{DownloadLog, DownloadOptions};
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let temp = tempdir()?;
/// let output = temp.path().join("wikidata.json.bz2");
/// let log_path = temp.path().join("downloads.sqlite");
/// let log = DownloadLog::initialise(log_path.as_path())?;
/// let options = DownloadOptions::new(output.as_path())
///     .with_log(&log)
///     .with_overwrite(true);
/// assert!(options.log.is_some());
/// assert!(options.overwrite);
/// assert_eq!(options.output_path, output.as_path());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct DownloadOptions<'a> {
    /// Destination path for the downloaded artefact.
    pub output_path: &'a Path,
    /// Optional download log used for persistence.
    pub log: Option<&'a DownloadLog>,
    /// Whether an existing file should be overwritten.
    pub overwrite: bool,
}

impl<'a> DownloadOptions<'a> {
    /// Construct options targeting `output_path` with default settings.
    pub fn new(output_path: &'a Path) -> Self {
        Self {
            output_path,
            log: None,
            overwrite: false,
        }
    }

    /// Attach a download log used to persist audit entries.
    #[must_use]
    pub fn with_log(mut self, log: &'a DownloadLog) -> Self {
        self.log = Some(log);
        self
    }

    /// Toggle whether the download should overwrite existing files.
    #[must_use]
    pub fn with_overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }
}
