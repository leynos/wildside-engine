//! Persist Wikidata entities, POI links, and heritage claims into SQLite using
//! a single transaction with idempotent statement execution. The helpers in
//! this module encapsulate the cached statement lifecycle so callers can load
//! batches of claims without duplicating insert guards or foreign key checks.
#![forbid(unsafe_code)]

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use rusqlite::{CachedStatement, Connection, Error as SqliteError, OptionalExtension, Transaction};
use thiserror::Error;

use crate::wikidata::etl::{EntityClaims, HERITAGE_PROPERTY};

use super::schema::{ClaimsSchemaError, initialise_schema};

struct PreparedStatements<'conn> {
    insert_entity: CachedStatement<'conn>,
    insert_link: CachedStatement<'conn>,
    insert_claim: CachedStatement<'conn>,
    check_poi: CachedStatement<'conn>,
}

impl<'conn> PreparedStatements<'conn> {
    fn prepare(transaction: &'conn Transaction<'conn>) -> Result<Self, PersistClaimsError> {
        let insert_entity = transaction
            .prepare_cached(concat!(
                "INSERT INTO wikidata_entities (entity_id) VALUES (?1) ",
                "ON CONFLICT(entity_id) DO NOTHING",
            ))
            .map_err(|source| PersistClaimsError::Sqlite {
                operation: "prepare insert entity",
                source,
            })?;
        let insert_link = transaction
            .prepare_cached(concat!(
                "INSERT INTO poi_wikidata_links (poi_id, entity_id) VALUES (?1, ?2) ",
                "ON CONFLICT(poi_id, entity_id) DO NOTHING",
            ))
            .map_err(|source| PersistClaimsError::Sqlite {
                operation: "prepare link POI",
                source,
            })?;
        let insert_claim = transaction
            .prepare_cached(concat!(
                "INSERT INTO wikidata_entity_claims (\n",
                "    entity_id,\n",
                "    property_id,\n",
                "    value_entity_id\n",
                ") VALUES (?1, ?2, ?3)\n",
                "ON CONFLICT(entity_id, property_id, value_entity_id) DO NOTHING",
            ))
            .map_err(|source| PersistClaimsError::Sqlite {
                operation: "prepare insert claim",
                source,
            })?;
        let check_poi = transaction
            .prepare_cached("SELECT 1 FROM pois WHERE id = ?1 LIMIT 1")
            .map_err(|source| PersistClaimsError::Sqlite {
                operation: "prepare POI lookup",
                source,
            })?;

        Ok(Self {
            insert_entity,
            insert_link,
            insert_claim,
            check_poi,
        })
    }
}

fn persist_entity(
    statement: &mut CachedStatement<'_>,
    entity_id: &str,
    operation: &'static str,
) -> Result<(), PersistClaimsError> {
    statement
        .execute([entity_id])
        .map(|_| ())
        .map_err(|source| PersistClaimsError::Sqlite { operation, source })
}

fn persist_heritage_designations(
    statements: &mut PreparedStatements<'_>,
    entity_id: &str,
    designations: &[String],
) -> Result<(), PersistClaimsError> {
    for designation in designations {
        persist_entity(
            &mut statements.insert_entity,
            designation.as_str(),
            "insert designation entity",
        )?;
        statements
            .insert_claim
            .execute((entity_id, HERITAGE_PROPERTY, designation.as_str()))
            .map_err(|source| PersistClaimsError::Sqlite {
                operation: "insert heritage claim",
                source,
            })?;
    }
    Ok(())
}

fn persist_poi_links(
    statements: &mut PreparedStatements<'_>,
    entity_id: &str,
    poi_ids: &[u64],
    known_pois: &mut HashSet<u64>,
) -> Result<(), PersistClaimsError> {
    for poi_id in poi_ids {
        let poi_id_i64 = i64::try_from(*poi_id)
            .map_err(|_| PersistClaimsError::PoiIdOutOfRange { poi_id: *poi_id })?;
        if !known_pois.contains(poi_id) {
            let exists = statements
                .check_poi
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
                    entity_id: entity_id.to_owned(),
                });
            }
            known_pois.insert(*poi_id);
        }
        statements
            .insert_link
            .execute((poi_id_i64, entity_id))
            .map_err(|source| PersistClaimsError::Sqlite {
                operation: "link POI to entity",
                source,
            })?;
    }
    Ok(())
}

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
        let mut statements = PreparedStatements::prepare(&transaction)?;
        let mut known_pois = HashSet::new();

        for claim in claims {
            persist_entity(
                &mut statements.insert_entity,
                claim.entity_id.as_str(),
                "insert entity",
            )?;
            persist_heritage_designations(
                &mut statements,
                claim.entity_id.as_str(),
                &claim.heritage_designations,
            )?;
            persist_poi_links(
                &mut statements,
                claim.entity_id.as_str(),
                &claim.linked_poi_ids,
                &mut known_pois,
            )?;
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
