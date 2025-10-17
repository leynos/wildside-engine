use std::{ops::Deref, path::PathBuf};

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
