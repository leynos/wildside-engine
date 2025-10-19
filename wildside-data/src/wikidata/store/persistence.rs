#![forbid(unsafe_code)]

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, Error as SqliteError, OptionalExtension};
use thiserror::Error;

use crate::wikidata::etl::{EntityClaims, HERITAGE_PROPERTY};

use super::schema::{ClaimsSchemaError, initialise_schema};

/// Persist the supplied claims into an initialised SQLite connection.
///
/// The function ensures the schema is present, validates that every referenced
/// POI id exists in the `pois` table, and performs idempotent inserts for both
/// entity metadata and claim values.
///
/// # Examples
/// ```
/// use rusqlite::Connection;
/// use wildside_data::wikidata::etl::EntityClaims;
/// use wildside_data::wikidata::store::persist_claims;
///
/// let mut conn = Connection::open_in_memory().expect("create in-memory database");
/// conn.execute(
///     "CREATE TABLE pois (
///         id INTEGER PRIMARY KEY,
///         lon REAL NOT NULL,
///         lat REAL NOT NULL,
///         tags TEXT NOT NULL
///     )",
///     [],
/// )
/// .expect("create pois table");
/// conn.execute(
///     "INSERT INTO pois (id, lon, lat, tags) VALUES (?1, ?2, ?3, ?4)",
///     (7, 13.4, 52.5, "{\"wikidata\":\"Q64\"}"),
/// )
/// .expect("insert POI row");
/// let claims = vec![EntityClaims {
///     entity_id: "Q64".into(),
///     linked_poi_ids: vec![7],
///     heritage_designations: vec!["Q9259".into()],
/// }];
///
/// persist_claims(&mut conn, &claims).expect("persist claims");
/// let count: i64 = conn
///     .query_row(
///         "SELECT COUNT(*) FROM poi_wikidata_claims WHERE poi_id = 7",
///         [],
///         |row| row.get(0),
///     )
///     .expect("query persisted claims");
/// assert_eq!(count, 1);
/// ```
pub fn persist_claims(
    connection: &mut Connection,
    claims: &[EntityClaims],
) -> Result<(), PersistClaimsError> {
    initialise_schema(connection)?;
    if claims.is_empty() {
        return Ok(());
    }

    let transaction = connection
        .transaction()
        .map_err(|source| PersistClaimsError::Sqlite {
            operation: "begin persistence transaction",
            source,
        })?;

    {
        let mut insert_entity = transaction
            .prepare_cached("INSERT OR IGNORE INTO wikidata_entities (entity_id) VALUES (?1)")
            .map_err(|source| PersistClaimsError::Sqlite {
                operation: "prepare insert entity",
                source,
            })?;
        let mut insert_link = transaction
            .prepare_cached(
                "INSERT OR IGNORE INTO poi_wikidata_links (poi_id, entity_id) VALUES (?1, ?2)",
            )
            .map_err(|source| PersistClaimsError::Sqlite {
                operation: "prepare link POI",
                source,
            })?;
        let mut insert_claim = transaction
            .prepare_cached(
                "INSERT OR IGNORE INTO wikidata_entity_claims (
                    entity_id,
                    property_id,
                    value_entity_id
                ) VALUES (?1, ?2, ?3)",
            )
            .map_err(|source| PersistClaimsError::Sqlite {
                operation: "prepare insert claim",
                source,
            })?;
        let mut check_poi = transaction
            .prepare_cached("SELECT 1 FROM pois WHERE id = ?1 LIMIT 1")
            .map_err(|source| PersistClaimsError::Sqlite {
                operation: "prepare POI lookup",
                source,
            })?;

        let mut known_pois = HashSet::new();

        for claim in claims {
            insert_entity
                .execute([claim.entity_id.as_str()])
                .map_err(|source| PersistClaimsError::Sqlite {
                    operation: "insert entity",
                    source,
                })?;

            for designation in &claim.heritage_designations {
                insert_entity
                    .execute([designation.as_str()])
                    .map_err(|source| PersistClaimsError::Sqlite {
                        operation: "insert designation entity",
                        source,
                    })?;
                insert_claim
                    .execute((
                        claim.entity_id.as_str(),
                        HERITAGE_PROPERTY,
                        designation.as_str(),
                    ))
                    .map_err(|source| PersistClaimsError::Sqlite {
                        operation: "insert heritage claim",
                        source,
                    })?;
            }

            for poi_id in &claim.linked_poi_ids {
                let poi_id_i64 = i64::try_from(*poi_id)
                    .map_err(|_| PersistClaimsError::PoiIdOutOfRange { poi_id: *poi_id })?;
                if !known_pois.contains(poi_id) {
                    let exists = check_poi
                        .query_row([poi_id_i64], |_| Ok(()))
                        .optional()
                        .map_err(|source| PersistClaimsError::Sqlite {
                            operation: "verify POI presence",
                            source,
                        })?
                        .is_some();
                    if !exists {
                        return Err(PersistClaimsError::MissingPoi {
                            poi_id: *poi_id,
                            entity_id: claim.entity_id.clone(),
                        });
                    }
                    known_pois.insert(*poi_id);
                }
                insert_link
                    .execute((poi_id_i64, claim.entity_id.as_str()))
                    .map_err(|source| PersistClaimsError::Sqlite {
                        operation: "link POI to entity",
                        source,
                    })?;
            }
        }
    }

    transaction
        .commit()
        .map_err(|source| PersistClaimsError::Sqlite {
            operation: "commit persistence transaction",
            source,
        })?;

    Ok(())
}

/// Convenience helper to persist claims to a database file on disk.
///
/// # Examples
/// ```
/// use rusqlite::Connection;
/// use tempfile::NamedTempFile;
/// use wildside_data::wikidata::etl::EntityClaims;
/// use wildside_data::wikidata::store::persist_claims_to_path;
///
/// let temp = NamedTempFile::new().expect("create temp file");
/// let conn = Connection::open(temp.path()).expect("open database");
/// conn.execute(
///     "CREATE TABLE pois (
///         id INTEGER PRIMARY KEY,
///         lon REAL NOT NULL,
///         lat REAL NOT NULL,
///         tags TEXT NOT NULL
///     )",
///     [],
/// )
/// .expect("create pois table");
/// conn.execute(
///     "INSERT INTO pois (id, lon, lat, tags) VALUES (?1, ?2, ?3, ?4)",
///     (11, 0.0, 0.0, "{\"wikidata\":\"Q42\"}"),
/// )
/// .expect("insert POI row");
/// drop(conn);
///
/// let claims = vec![EntityClaims {
///     entity_id: "Q42".into(),
///     linked_poi_ids: vec![11],
///     heritage_designations: vec!["Q9259".into()],
/// }];
///
/// persist_claims_to_path(temp.path(), &claims).expect("persist claims to disk");
/// let conn = Connection::open(temp.path()).expect("reopen database");
/// let exists: i64 = conn
///     .query_row(
///         "SELECT COUNT(*) FROM poi_wikidata_claims WHERE poi_id = 11",
///         [],
///         |row| row.get(0),
///     )
///     .expect("read persisted claims");
/// assert_eq!(exists, 1);
/// ```
pub fn persist_claims_to_path<P: AsRef<Path>>(
    path: P,
    claims: &[EntityClaims],
) -> Result<(), PersistClaimsError> {
    let mut connection =
        Connection::open(path.as_ref()).map_err(|source| PersistClaimsError::Open {
            path: path.as_ref().to_path_buf(),
            source,
        })?;
    persist_claims(&mut connection, claims)
}

/// Errors raised when persisting Wikidata claims.
#[derive(Debug, Error)]
pub enum PersistClaimsError {
    #[error("failed to open SQLite database at {path:?}")]
    Open {
        path: PathBuf,
        #[source]
        source: SqliteError,
    },
    #[error(transparent)]
    Schema(#[from] ClaimsSchemaError),
    #[error("POI id {poi_id} exceeds SQLite i64 range")]
    PoiIdOutOfRange { poi_id: u64 },
    #[error("POI id {poi_id} referenced by entity {entity_id} is missing from the pois table")]
    MissingPoi { poi_id: u64, entity_id: String },
    #[error("failed to persist {operation}")]
    Sqlite {
        operation: &'static str,
        #[source]
        source: SqliteError,
    },
}
