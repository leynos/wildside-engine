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
        Ok(Self {
            connection,
            location: path.to_path_buf(),
        })
    }

    /// Record a completed download in the log.
    pub fn record(&self, report: &DownloadReport) -> Result<(), WikidataDumpError> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| WikidataDumpError::RecordLog {
                source: rusqlite::Error::ToSqlConversionFailure(Box::new(err)),
            })?;
        let timestamp =
            i64::try_from(duration.as_secs()).map_err(|err| WikidataDumpError::RecordLog {
                source: rusqlite::Error::ToSqlConversionFailure(Box::new(err)),
            })?;
        let size = report
            .descriptor
            .size
            .map(|value| {
                i64::try_from(value).map_err(|err| WikidataDumpError::RecordLog {
                    source: rusqlite::Error::ToSqlConversionFailure(Box::new(err)),
                })
            })
            .transpose()?;
        let bytes =
            i64::try_from(report.bytes_written).map_err(|err| WikidataDumpError::RecordLog {
                source: rusqlite::Error::ToSqlConversionFailure(Box::new(err)),
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
                    &report.descriptor.file_name,
                    &report.descriptor.url,
                    sha1,
                    size,
                    bytes,
                    output_path,
                    timestamp
                ],
            )
            .map_err(|source| WikidataDumpError::RecordLog { source })?;
        Ok(())
    }

    /// Location of the underlying SQLite database.
    pub fn path(&self) -> &Path {
        &self.location
    }

    #[cfg(test)]
    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }
}
