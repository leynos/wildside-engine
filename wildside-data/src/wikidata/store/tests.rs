//! Unit tests for the Wikidata claims persistence layer.

mod behaviour;

use super::{
    ClaimsSchemaError, PersistClaimsError, SCHEMA_VERSION, initialise_schema, persist_claims,
};
use crate::wikidata::etl::EntityClaims;
use rstest::{fixture, rstest};
use rusqlite::Connection;

fn create_pois_table(connection: &Connection) {
    connection
        .execute(
            "CREATE TABLE pois (
                id INTEGER PRIMARY KEY,
                lon REAL NOT NULL,
                lat REAL NOT NULL,
                tags TEXT NOT NULL
            )",
            [],
        )
        .expect("create pois table");
}

fn insert_poi(connection: &Connection, id: i64) {
    connection
        .execute(
            "INSERT INTO pois (id, lon, lat, tags) VALUES (?1, 0.0, 0.0, '{}')",
            [id],
        )
        .expect("insert poi");
}

#[fixture]
fn connection() -> Connection {
    Connection::open_in_memory().expect("open in-memory database")
}

#[rstest]
fn initialises_schema_records_version(mut connection: Connection) -> Result<(), ClaimsSchemaError> {
    create_pois_table(&connection);

    initialise_schema(&mut connection)?;

    let version: i64 = connection
        .query_row(
            "SELECT version FROM wikidata_schema_version LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("schema version present");
    assert_eq!(version, SCHEMA_VERSION);

    let table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN (
                'wikidata_entities',
                'poi_wikidata_links',
                'wikidata_entity_claims'
            )",
            [],
            |row| row.get(0),
        )
        .expect("query tables");
    assert_eq!(
        table_count, 3,
        "expected three Wikidata tables to be created"
    );
    Ok(())
}

#[rstest]
fn persists_claims_for_linked_poi(mut connection: Connection) -> Result<(), PersistClaimsError> {
    create_pois_table(&connection);
    insert_poi(&connection, 7);

    let claims = vec![EntityClaims {
        entity_id: "Q64".into(),
        linked_poi_ids: vec![7],
        heritage_designations: vec!["Q9259".into()],
    }];

    persist_claims(&mut connection, &claims)?;

    let mut statement = connection
        .prepare("SELECT poi_id, property_id, value_entity_id FROM poi_wikidata_claims")
        .expect("prepare select");
    let rows: Vec<(i64, String, String)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("map rows")
        .collect::<Result<_, _>>()
        .expect("collect rows");
    assert_eq!(rows, vec![(7, "P1435".to_string(), "Q9259".to_string())]);

    Ok(())
}

#[rstest]
fn rejects_missing_poi(mut connection: Connection) {
    create_pois_table(&connection);

    let claims = vec![EntityClaims {
        entity_id: "Q64".into(),
        linked_poi_ids: vec![42],
        heritage_designations: vec!["Q9259".into()],
    }];

    let err = persist_claims(&mut connection, &claims).expect_err("missing POI should error");
    match err {
        PersistClaimsError::MissingPoi { poi_id, entity_id } => {
            assert_eq!(poi_id, 42);
            assert_eq!(entity_id, "Q64");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[rstest]
fn inserts_idempotently(mut connection: Connection) -> Result<(), PersistClaimsError> {
    create_pois_table(&connection);
    insert_poi(&connection, 11);

    let claims = vec![EntityClaims {
        entity_id: "Q42".into(),
        linked_poi_ids: vec![11],
        heritage_designations: vec!["Q9259".into()],
    }];

    persist_claims(&mut connection, &claims)?;
    persist_claims(&mut connection, &claims)?;

    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM poi_wikidata_claims", [], |row| {
            row.get(0)
        })
        .expect("count rows");
    assert_eq!(
        count, 1,
        "duplicate persistence should not create extra rows"
    );
    Ok(())
}
