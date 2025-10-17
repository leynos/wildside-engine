//! High-level operations for selecting and downloading Wikidata dumps.

use simd_json::serde::from_reader;
use std::{
    fs,
    io::{self, BufRead, Write},
    path::Path,
};
use tempfile::Builder;
use url::Url;

use super::source::DumpSource;
use super::{
    BaseUrl, DownloadLog, DownloadOptions, DownloadReport, DumpDescriptor, DumpFileName, DumpUrl,
    WikidataDumpError,
};

const JSON_DUMP_SUFFIX: &str = "-all.json.bz2";

/// Download the latest Wikidata dump using the supplied source.
///
/// Provide a [`DownloadLog`] to persist audit entries; pass `None` to skip
/// logging when durability is unnecessary. Enable `overwrite` to truncate any
/// existing archive at `output_path` before writing new bytes.
///
/// # Examples
/// ```
/// # use tempfile::tempdir;
/// # use wildside_data::wikidata::dump::{
/// #     download_latest_dump, BaseUrl, DownloadLog, StubSource, WikidataDumpError,
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
///         download_latest_dump(&source, output_path.as_path(), Some(&log), false).await
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
    overwrite: bool,
) -> Result<DownloadReport, WikidataDumpError> {
    let descriptor = resolve_latest_descriptor(source).await?;
    let options = log
        .map_or_else(
            || DownloadOptions::new(output_path),
            |entry| DownloadOptions::new(output_path).with_log(entry),
        )
        .with_overwrite(overwrite);
    download_descriptor(source, descriptor, options).await
}

/// Download a specific dump described by `descriptor`.
///
/// Supplying a [`DownloadOptions`] instance captures logging and overwrite
/// preferences while keeping call sites free of positional argument overload.
///
/// # Examples
/// ```
/// # use tempfile::tempdir;
/// # use wildside_data::wikidata::dump::{
/// #     download_descriptor, BaseUrl, DownloadLog, DownloadOptions, DumpDescriptor, DumpFileName,
/// #     DumpUrl, StubSource, WikidataDumpError,
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
///     .block_on(async {
///         let options = DownloadOptions::new(output_path.as_path()).with_log(&log);
///         download_descriptor(&source, descriptor.clone(), options).await
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
    options: DownloadOptions<'_>,
) -> Result<DownloadReport, WikidataDumpError> {
    let output_path = options.output_path;
    let log = options.log;
    let overwrite = options.overwrite;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| WikidataDumpError::CreateDir {
            source,
            path: parent.to_path_buf(),
        })?;
    }
    if !overwrite && output_path.exists() {
        return Err(WikidataDumpError::WriteDump {
            source: io::Error::new(io::ErrorKind::AlreadyExists, "output file exists"),
            path: output_path.to_path_buf(),
        });
    }
    let parent_dir = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temp_file = Builder::new()
        .prefix("wikidata-etl-")
        .tempfile_in(parent_dir)
        .map_err(|source| WikidataDumpError::WriteDump {
            source,
            path: output_path.to_path_buf(),
        })?;
    let bytes_written = source
        .download_archive(&descriptor.url, temp_file.as_file_mut())
        .await
        .map_err(|source| WikidataDumpError::Download { source })?;
    temp_file
        .as_file_mut()
        .flush()
        .map_err(|source| WikidataDumpError::WriteDump {
            source,
            path: output_path.to_path_buf(),
        })?;
    if overwrite && output_path.exists() {
        fs::remove_file(output_path).map_err(|source| WikidataDumpError::WriteDump {
            source,
            path: output_path.to_path_buf(),
        })?;
    }
    temp_file
        .persist(output_path)
        .map_err(|error| WikidataDumpError::WriteDump {
            source: error.error,
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
/// #     resolve_latest_descriptor, BaseUrl, StubSource, WikidataDumpError,
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

pub(crate) fn select_dump(
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
        .filter(|(file_name, _)| file_name.ends_with(JSON_DUMP_SUFFIX))
        .filter_map(|(file_name, entry)| {
            DumpDescriptor::from_manifest_entry(file_name, entry, base_url)
        })
        .max_by(|left, right| left.file_name.as_ref().cmp(right.file_name.as_ref()))
        .ok_or(WikidataDumpError::MissingDump)
}

pub(crate) fn normalise_url(
    base_url: &BaseUrl,
    relative: &str,
) -> Result<DumpUrl, url::ParseError> {
    if relative.starts_with("http://") || relative.starts_with("https://") {
        return Url::parse(relative).map(Into::into);
    }
    let base = Url::parse(base_url.as_ref())?;
    base.join(relative).map(Into::into)
}

#[derive(Debug, serde::Deserialize)]
struct DumpStatus {
    jobs: std::collections::HashMap<String, DumpJob>,
}

#[derive(Debug, serde::Deserialize)]
struct DumpJob {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    files: std::collections::HashMap<String, DumpFile>,
}

impl DumpJob {
    fn is_done(&self) -> bool {
        self.status
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("done"))
    }
}

#[derive(Debug, serde::Deserialize)]
struct DumpFile {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    sha1: Option<String>,
}

impl DumpDescriptor {
    fn from_manifest_entry(file_name: &str, entry: &DumpFile, base_url: &BaseUrl) -> Option<Self> {
        let relative = entry.url.as_deref()?;
        let url = normalise_url(base_url, relative).ok()?;
        Some(Self {
            file_name: DumpFileName::from(file_name),
            url,
            size: entry.size,
            sha1: entry.sha1.clone(),
        })
    }
}
