#![forbid(unsafe_code)]

use rusqlite::{Connection, Error as SqliteError, OptionalExtension};
use thiserror::Error;

pub const SCHEMA_VERSION: i64 = 1;

/// Initialise the Wikidata claims schema inside an existing SQLite database.
///
/// The function enables foreign keys, creates the supporting tables, indexes
/// and views, and records the schema version. Existing installations must
/// already match the expected version; mismatches are rejected so migrations
/// can be applied explicitly.
///
/// # Examples
/// ```
/// use rusqlite::Connection;
/// use wildside_data::wikidata::store::initialise_schema;
///
/// let mut conn = Connection::open_in_memory().expect("create in-memory database");
/// conn.execute(
///     "CREATE TABLE pois (id INTEGER PRIMARY KEY, lon REAL NOT NULL, lat REAL NOT NULL, tags TEXT NOT NULL)",
///     [],
/// )
/// .expect("seed POI table");
/// initialise_schema(&mut conn).expect("create Wikidata schema");
///
/// let version: i64 = conn
///     .query_row(
///         "SELECT version FROM wikidata_schema_version LIMIT 1",
///         [],
///         |row| row.get(0),
///     )
///     .expect("read schema version");
/// assert_eq!(version, 1);
/// ```
pub fn initialise_schema(connection: &mut Connection) -> Result<(), ClaimsSchemaError> {
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(|source| ClaimsSchemaError::ForeignKeys { source })?;

    let transaction = connection
        .transaction()
        .map_err(|source| ClaimsSchemaError::Migration {
            step: "begin schema transaction",
            source,
        })?;

    create_core_tables(&transaction)?;
    create_indexes(&transaction)?;
    create_views(&transaction)?;
    ensure_schema_version(&transaction)?;

    transaction
        .commit()
        .map_err(|source| ClaimsSchemaError::Migration {
            step: "commit schema transaction",
            source,
        })?;

    Ok(())
}

fn create_core_tables(transaction: &rusqlite::Transaction<'_>) -> Result<(), ClaimsSchemaError> {
    run_migration_step(
        transaction,
        "create wikidata_entities",
        "CREATE TABLE IF NOT EXISTS wikidata_entities (
            entity_id TEXT PRIMARY KEY CHECK (length(trim(entity_id)) > 0)
        ) WITHOUT ROWID",
    )?;
    run_migration_step(
        transaction,
        "create poi_wikidata_links",
        "CREATE TABLE IF NOT EXISTS poi_wikidata_links (
            poi_id INTEGER NOT NULL,
            entity_id TEXT NOT NULL,
            PRIMARY KEY (poi_id, entity_id),
            FOREIGN KEY (poi_id) REFERENCES pois(id) ON DELETE CASCADE,
            FOREIGN KEY (entity_id) REFERENCES wikidata_entities(entity_id) ON DELETE CASCADE
        ) WITHOUT ROWID",
    )?;
    run_migration_step(
        transaction,
        "create wikidata_entity_claims",
        "CREATE TABLE IF NOT EXISTS wikidata_entity_claims (
            entity_id TEXT NOT NULL,
            property_id TEXT NOT NULL,
            value_entity_id TEXT NOT NULL,
            PRIMARY KEY (entity_id, property_id, value_entity_id),
            FOREIGN KEY (entity_id) REFERENCES wikidata_entities(entity_id) ON DELETE CASCADE,
            FOREIGN KEY (value_entity_id) REFERENCES wikidata_entities(entity_id) ON DELETE CASCADE
        ) WITHOUT ROWID",
    )
}

fn create_indexes(transaction: &rusqlite::Transaction<'_>) -> Result<(), ClaimsSchemaError> {
    run_migration_step(
        transaction,
        "index wikidata_entity_claims",
        "CREATE INDEX IF NOT EXISTS idx_wikidata_entity_claims_property
            ON wikidata_entity_claims(property_id, value_entity_id, entity_id)",
    )?;
    run_migration_step(
        transaction,
        "index poi_wikidata_links",
        "CREATE INDEX IF NOT EXISTS idx_poi_wikidata_links_entity
            ON poi_wikidata_links(entity_id, poi_id)",
    )
}

fn create_views(transaction: &rusqlite::Transaction<'_>) -> Result<(), ClaimsSchemaError> {
    run_migration_step(
        transaction,
        "create poi_wikidata_claims view",
        "CREATE VIEW IF NOT EXISTS poi_wikidata_claims AS
            SELECT
                links.poi_id AS poi_id,
                claims.entity_id AS entity_id,
                claims.property_id AS property_id,
                claims.value_entity_id AS value_entity_id
            FROM poi_wikidata_links AS links
            JOIN wikidata_entity_claims AS claims
                ON claims.entity_id = links.entity_id",
    )
}

fn ensure_schema_version(transaction: &rusqlite::Transaction<'_>) -> Result<(), ClaimsSchemaError> {
    run_migration_step(
        transaction,
        "create schema version table",
        "CREATE TABLE IF NOT EXISTS wikidata_schema_version (
            version INTEGER PRIMARY KEY CHECK (version > 0),
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
        ) WITHOUT ROWID",
    )?;

    let existing_version: Option<i64> = transaction
        .query_row(
            "SELECT version FROM wikidata_schema_version LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|source| ClaimsSchemaError::Migration {
            step: "read schema version",
            source,
        })?;

    match existing_version {
        Some(version) if version == SCHEMA_VERSION => {}
        Some(found) => {
            return Err(ClaimsSchemaError::VersionMismatch {
                expected: SCHEMA_VERSION,
                found,
            });
        }
        None => {
            transaction
                .execute(
                    "INSERT INTO wikidata_schema_version (version) VALUES (?1)",
                    [SCHEMA_VERSION],
                )
                .map_err(|source| ClaimsSchemaError::Migration {
                    step: "record schema version",
                    source,
                })?;
        }
    }

    Ok(())
}

fn run_migration_step(
    transaction: &rusqlite::Transaction<'_>,
    step: &'static str,
    sql: &str,
) -> Result<(), ClaimsSchemaError> {
    transaction
        .execute(sql, [])
        .map(|_| ())
        .map_err(|source| ClaimsSchemaError::Migration { step, source })
}

/// Errors raised when initialising the Wikidata claims schema.
#[derive(Debug, Error)]
pub enum ClaimsSchemaError {
    #[error("failed to enable SQLite foreign keys")]
    ForeignKeys {
        #[source]
        source: SqliteError,
    },
    #[error("failed to execute migration step '{step}'")]
    Migration {
        step: &'static str,
        #[source]
        source: SqliteError,
    },
    #[error(
        "expected Wikidata schema version {expected} but found {found}; apply migrations before retrying"
    )]
    VersionMismatch { expected: i64, found: i64 },
}
