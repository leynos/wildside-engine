//! Maintains a SQLite-backed audit log for Wikidata dump downloads.
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, params};

use super::{DownloadReport, WikidataDumpError};

/// Captures a persisted audit trail of downloads.
#[derive(Debug)]
pub struct DownloadLog {
    connection: Connection,
    location: PathBuf,
}

impl DownloadLog {
    /// Open (or create) the download log at the supplied path.
    ///
    /// The log seeds uniqueness and timestamp indexes to keep repeated
    /// initialisation idempotent while supporting fast lookups.
    ///
    /// # Examples
    /// ```
    /// # use tempfile::tempdir;
    /// # use wildside_data::wikidata::dump::{DownloadLog, WikidataDumpError};
    /// # fn demo() -> Result<(), WikidataDumpError> {
    /// let temp = tempdir().expect("create temp directory");
    /// let db_path = temp.path().join("downloads.sqlite");
    /// let log = DownloadLog::initialise(db_path.as_path())?;
    /// assert_eq!(log.path(), db_path.as_path());
    /// # Ok(())
    /// # }
    /// ```
    pub fn initialise(path: &Path) -> Result<Self, WikidataDumpError> {
        let connection =
            Connection::open(path).map_err(|source| WikidataDumpError::InitialiseLog {
                source,
                path: path.to_path_buf(),
            })?;
        connection
            .execute(
                "CREATE TABLE IF NOT EXISTS downloads (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    file_name TEXT NOT NULL,
                    url TEXT NOT NULL,
                    sha1 TEXT,
                    size_bytes INTEGER,
                    bytes_written INTEGER NOT NULL,
                    output_path TEXT NOT NULL,
                    downloaded_at INTEGER NOT NULL
                )",
                [],
            )
            .map_err(|source| WikidataDumpError::InitialiseLog {
                source,
                path: path.to_path_buf(),
            })?;
        connection
            .execute(
                "CREATE UNIQUE INDEX IF NOT EXISTS \
                 u_downloads_file_output ON downloads(file_name, output_path)",
                [],
            )
            .map_err(|source| WikidataDumpError::InitialiseLog {
                source,
                path: path.to_path_buf(),
            })?;
        connection
            .execute(
                "CREATE INDEX IF NOT EXISTS ix_downloads_downloaded_at \
                 ON downloads(downloaded_at)",
                [],
            )
            .map_err(|source| WikidataDumpError::InitialiseLog {
                source,
                path: path.to_path_buf(),
            })?;
        Ok(Self {
            connection,
            location: path.to_path_buf(),
        })
    }

    /// Record a completed download in the log.
    ///
    /// # Examples
    /// ```
    /// # use rusqlite::Connection;
    /// # use tempfile::tempdir;
    /// # use wildside_data::wikidata::dump::{
    /// #     DownloadLog, DownloadReport, DumpDescriptor, DumpFileName,
    /// #     DumpUrl, WikidataDumpError,
    /// # };
    /// # fn demo() -> Result<(), WikidataDumpError> {
    /// let temp = tempdir().expect("create temp directory");
    /// let db_path = temp.path().join("downloads.sqlite");
    /// let output_path = temp.path().join("wikidata.json.bz2");
    /// let log = DownloadLog::initialise(db_path.as_path())?;
    /// let descriptor = DumpDescriptor {
    ///     file_name: DumpFileName::new("wikidata.json.bz2"),
    ///     url: DumpUrl::new("https://example.test/wikidata.json.bz2"),
    ///     size: Some(128),
    ///     sha1: None,
    /// };
    /// let report = DownloadReport {
    ///     descriptor,
    ///     bytes_written: 128,
    ///     output_path: output_path.clone(),
    /// };
    /// log.record(&report)?;
    /// let connection = Connection::open(log.path()).expect("open log for assertions");
    /// let recorded: i64 = connection
    ///     .query_row("SELECT COUNT(*) FROM downloads", [], |row| row.get(0))
    ///     .expect("count rows");
    /// assert_eq!(recorded, 1);
    /// # Ok(())
    /// # }
    /// ```
    pub fn record(&self, report: &DownloadReport) -> Result<(), WikidataDumpError> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|source| WikidataDumpError::RecordLogValue {
                what: "current time".to_owned(),
                source: Box::new(source),
            })?;
        let timestamp = i64::try_from(duration.as_secs()).map_err(|source| {
            WikidataDumpError::RecordLogValue {
                what: "timestamp".to_owned(),
                source: Box::new(source),
            }
        })?;
        let size = report
            .descriptor
            .size
            .map(|value| {
                i64::try_from(value).map_err(|source| WikidataDumpError::RecordLogValue {
                    what: "declared size".to_owned(),
                    source: Box::new(source),
                })
            })
            .transpose()?;
        let bytes = i64::try_from(report.bytes_written).map_err(|source| {
            WikidataDumpError::RecordLogValue {
                what: "bytes written".to_owned(),
                source: Box::new(source),
            }
        })?;
        let output_path = report.output_path.to_string_lossy().to_string();
        let sha1 = report.descriptor.sha1.clone();
        self.connection
            .execute(
                "INSERT INTO downloads (
                    file_name,
                    url,
                    sha1,
                    size_bytes,
                    bytes_written,
                    output_path,
                    downloaded_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    report.descriptor.file_name.as_ref(),
                    report.descriptor.url.as_ref(),
                    sha1,
                    size,
                    bytes,
                    output_path,
                    timestamp
                ],
            )
            .map_err(|source| WikidataDumpError::RecordLogSql { source })?;
        Ok(())
    }

    /// Location of the underlying SQLite database.
    ///
    /// # Examples
    /// ```
    /// # use tempfile::tempdir;
    /// # use wildside_data::wikidata::dump::{DownloadLog, WikidataDumpError};
    /// # fn demo() -> Result<(), WikidataDumpError> {
    /// let temp = tempdir().expect("create temp directory");
    /// let db_path = temp.path().join("downloads.sqlite");
    /// let log = DownloadLog::initialise(db_path.as_path())?;
    /// assert_eq!(log.path(), db_path.as_path());
    /// # Ok(())
    /// # }
    /// ```
    pub fn path(&self) -> &Path {
        &self.location
    }

    #[cfg(test)]
    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }
}
