//! SQLite persistence for points of interest derived from OSM ingestion.
#![forbid(unsafe_code)]

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8};
use rusqlite::{Connection, Error as SqliteError, Transaction};
use serde_json::to_string;
use std::io;
use std::path::Component;
use thiserror::Error;
use wildside_core::PointOfInterest;

/// Errors raised when persisting ingested POIs to SQLite.
#[derive(Debug, Error)]
pub enum PersistPoisError {
    /// Failed to create the parent directory for the SQLite artefact.
    #[error("failed to create parent directory {path:?}")]
    CreateDirectory {
        /// Path of the directory that could not be created.
        path: Utf8PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// Opening the SQLite database failed.
    #[error("failed to open SQLite database at {path:?}")]
    Open {
        /// Destination database path.
        path: Utf8PathBuf,
        /// Source error returned by `rusqlite`.
        #[source]
        source: SqliteError,
    },
    /// Enabling SQLite foreign keys failed.
    #[error("failed to enable SQLite foreign keys")]
    ForeignKeys {
        /// Source error returned by `rusqlite`.
        #[source]
        source: SqliteError,
    },
    /// Beginning the transaction failed.
    #[error("failed to begin POI persistence transaction")]
    BeginTransaction {
        /// Source error returned by `rusqlite`.
        #[source]
        source: SqliteError,
    },
    /// Creating the `pois` table failed.
    #[error("failed to create pois table")]
    CreateSchema {
        /// Source error returned by `rusqlite`.
        #[source]
        source: SqliteError,
    },
    /// A POI identifier could not be represented as an SQLite integer.
    #[error("POI id {poi_id} exceeds SQLite i64 range")]
    PoiIdOutOfRange {
        /// Identifier that failed the conversion.
        poi_id: u64,
    },
    /// Serializing POI tags to JSON failed.
    #[error("failed to serialize tags for POI {poi_id}")]
    SerializeTags {
        /// Identifier of the POI whose tags failed to serialize.
        poi_id: u64,
        /// Source error produced by `serde_json`.
        #[source]
        source: serde_json::Error,
    },
    /// Writing a POI row failed.
    #[error("failed to persist POI {poi_id}")]
    PersistRow {
        /// Identifier of the POI being persisted.
        poi_id: u64,
        /// Source error returned by `rusqlite`.
        #[source]
        source: SqliteError,
    },
    /// Preparing the insert statement failed.
    #[error("failed to prepare POI insert statement")]
    PrepareInsert {
        /// Source error returned by `rusqlite`.
        #[source]
        source: SqliteError,
    },
    /// Committing the transaction failed.
    #[error("failed to commit POI persistence transaction")]
    Commit {
        /// Source error returned by `rusqlite`.
        #[source]
        source: SqliteError,
    },
}

/// Persist points of interest to a SQLite database on disk.
///
/// The function is idempotent: rows are replaced when identifiers already
/// exist. Parent directories are created automatically, and the `pois` table
/// is initialized if missing. Tags are serialized to JSON strings.
pub fn persist_pois_to_sqlite(
    path: &Utf8Path,
    pois: &[PointOfInterest],
) -> Result<(), PersistPoisError> {
    ensure_parent_dir(path)?;
    let mut connection =
        Connection::open(path.as_std_path()).map_err(|source| PersistPoisError::Open {
            path: path.to_path_buf(),
            source,
        })?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(|source| PersistPoisError::ForeignKeys { source })?;

    let transaction = connection
        .transaction()
        .map_err(|source| PersistPoisError::BeginTransaction { source })?;

    create_schema(&transaction)?;
    persist_rows(&transaction, pois)?;

    transaction
        .commit()
        .map_err(|source| PersistPoisError::Commit { source })?;
    Ok(())
}

fn ensure_parent_dir(path: &Utf8Path) -> Result<(), PersistPoisError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() || parent == Utf8Path::new("/") {
        return Ok(());
    }

    let (base_dir, relative) = base_dir_and_relative(parent)?;
    if relative.as_os_str().is_empty() {
        return Ok(());
    }
    base_dir
        .create_dir_all(&relative)
        .map_err(|source| PersistPoisError::CreateDirectory {
            path: parent.to_path_buf(),
            source,
        })?;

    Ok(())
}

fn base_dir_and_relative(
    parent: &Utf8Path,
) -> Result<(fs_utf8::Dir, Utf8PathBuf), PersistPoisError> {
    let std_parent = parent.as_std_path();

    let (base, relative) = match std_parent.components().next() {
        // Windows absolute path with a drive or UNC prefix.
        Some(Component::Prefix(prefix)) => {
            let prefix_str =
                prefix
                    .as_os_str()
                    .to_str()
                    .ok_or_else(|| PersistPoisError::CreateDirectory {
                        path: parent.to_path_buf(),
                        source: io::Error::other("non-UTF-8 path prefix"),
                    })?;

            let base = Utf8PathBuf::from(format!("{}{}", prefix_str, std::path::MAIN_SEPARATOR));
            let relative = std_parent
                .strip_prefix(base.as_std_path())
                .or_else(|_| std_parent.strip_prefix(prefix.as_os_str()))
                .map_err(|_| PersistPoisError::CreateDirectory {
                    path: parent.to_path_buf(),
                    source: io::Error::other("failed to strip prefix from parent path"),
                })?
                .to_path_buf();
            (base, relative)
        }
        // Unix-style absolute path.
        Some(Component::RootDir) => {
            let base = Utf8PathBuf::from(std::path::MAIN_SEPARATOR.to_string());
            let relative = std_parent
                .strip_prefix(base.as_std_path())
                .map_err(|_| PersistPoisError::CreateDirectory {
                    path: parent.to_path_buf(),
                    source: io::Error::other("failed to strip root from absolute path"),
                })?
                .to_path_buf();
            (base, relative)
        }
        // Relative path: resolve from the current directory.
        _ => (Utf8PathBuf::from("."), std_parent.to_path_buf()),
    };

    let dir = fs_utf8::Dir::open_ambient_dir(&base, ambient_authority()).map_err(|source| {
        PersistPoisError::CreateDirectory {
            path: parent.to_path_buf(),
            source,
        }
    })?;

    let relative =
        Utf8PathBuf::from_path_buf(relative).map_err(|_| PersistPoisError::CreateDirectory {
            path: parent.to_path_buf(),
            source: io::Error::other("non-utf8 parent path"),
        })?;

    Ok((dir, relative))
}

fn create_schema(transaction: &Transaction<'_>) -> Result<(), PersistPoisError> {
    transaction
        .execute(
            "CREATE TABLE IF NOT EXISTS pois (
                id INTEGER PRIMARY KEY,
                lon REAL NOT NULL,
                lat REAL NOT NULL,
                tags TEXT NOT NULL
            )",
            [],
        )
        .map(|_| ())
        .map_err(|source| PersistPoisError::CreateSchema { source })
}

fn persist_rows(
    transaction: &Transaction<'_>,
    pois: &[PointOfInterest],
) -> Result<(), PersistPoisError> {
    if pois.is_empty() {
        return Ok(());
    }

    let mut statement = transaction
        .prepare("INSERT OR REPLACE INTO pois (id, lon, lat, tags) VALUES (?1, ?2, ?3, ?4)")
        .map_err(|source| PersistPoisError::PrepareInsert { source })?;

    for poi in pois {
        let poi_id = i64::try_from(poi.id)
            .map_err(|_| PersistPoisError::PoiIdOutOfRange { poi_id: poi.id })?;
        let tags = to_string(&poi.tags).map_err(|source| PersistPoisError::SerializeTags {
            poi_id: poi.id,
            source,
        })?;
        statement
            .execute((poi_id, poi.location.x, poi.location.y, tags))
            .map_err(|source| PersistPoisError::PersistRow {
                poi_id: poi.id,
                source,
            })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use geo::Coord;
    use rstest::{fixture, rstest};
    use rusqlite::Connection;
    use tempfile::TempDir;
    use wildside_core::Tags;

    #[fixture]
    fn poi() -> PointOfInterest {
        PointOfInterest::new(
            7,
            Coord { x: 1.0, y: 2.0 },
            Tags::from([("name".into(), "Example".into())]),
        )
    }

    #[fixture]
    fn temp_dir() -> TempDir {
        TempDir::new().expect("create temp dir")
    }

    #[rstest]
    fn persists_pois(temp_dir: TempDir, poi: PointOfInterest) {
        let db_path =
            Utf8PathBuf::from_path_buf(temp_dir.path().join("pois.db")).expect("utf-8 path");

        persist_pois_to_sqlite(&db_path, std::slice::from_ref(&poi)).expect("persist POIs");

        let conn = Connection::open(db_path.as_std_path()).expect("open database");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM pois", [], |row| row.get(0))
            .expect("count rows");
        assert_eq!(count, 1, "expected single POI row");

        let stored: (i64, f64, f64, String) = conn
            .query_row("SELECT id, lon, lat, tags FROM pois", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .expect("read row");
        assert_eq!(stored.0, 7);
        assert_eq!(stored.1, 1.0);
        assert_eq!(stored.2, 2.0);
        assert!(stored.3.contains("Example"));
    }

    #[rstest]
    fn creates_parent_directory(temp_dir: TempDir, poi: PointOfInterest) {
        let nested =
            Utf8PathBuf::from_path_buf(temp_dir.path().join("nested/pois.db")).expect("utf-8 path");

        persist_pois_to_sqlite(&nested, &[poi]).expect("persist POIs into nested path");

        assert!(nested.exists(), "database should be created at nested path");
    }

    #[rstest]
    fn rejects_out_of_range_id(temp_dir: TempDir) {
        let db_path =
            Utf8PathBuf::from_path_buf(temp_dir.path().join("pois.db")).expect("utf-8 path");
        let poi = PointOfInterest::with_empty_tags(u64::MAX, Coord { x: 0.0, y: 0.0 });

        let err =
            persist_pois_to_sqlite(&db_path, &[poi]).expect_err("should fail for out-of-range id");
        assert!(matches!(err, PersistPoisError::PoiIdOutOfRange { .. }));
    }

    fn test_absolute_path_persistence(path: Utf8PathBuf, poi: PointOfInterest, description: &str) {
        let _ = std::fs::remove_file(path.as_std_path());

        persist_pois_to_sqlite(&path, &[poi])
            .unwrap_or_else(|_| panic!("persist POIs to {description}"));

        let exists = path.exists();
        let _ = std::fs::remove_file(path.as_std_path());
        assert!(
            exists,
            "expected database file to be created at {description}"
        );
    }

    #[rstest]
    fn persists_to_absolute_path(poi: PointOfInterest) {
        let path = Utf8PathBuf::from("/tmp/wildside_pois.db");
        test_absolute_path_persistence(path, poi, "absolute path");
    }

    #[cfg(windows)]
    #[rstest]
    fn persists_to_windows_absolute_path(poi: PointOfInterest) {
        let path = Utf8PathBuf::from("C:\\temp\\wildside_pois.db");
        test_absolute_path_persistence(path, poi, "Windows absolute path");
    }

    #[cfg(unix)]
    #[rstest]
    fn persisting_under_root_reports_permission(poi: PointOfInterest) {
        let path = Utf8PathBuf::from("/pois.db");
        let outcome = persist_pois_to_sqlite(&path, &[poi]);
        match outcome {
            Err(PersistPoisError::Open { .. }) | Err(PersistPoisError::CreateDirectory { .. }) => {}
            Ok(_) => {
                // Clean up if the environment permits writing to root. Some CI
                // environments run with elevated privileges, so avoid failing
                // when permissions are relaxed.
                let _ = std::fs::remove_file(path.as_std_path());
            }
            Err(other) => panic!("unexpected error when writing to root: {other:?}"),
        }
    }
}
