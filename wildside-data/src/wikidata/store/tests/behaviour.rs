//! Behavioural tests for persisting Wikidata claims using rstest-bdd.

use super::super::{PersistClaimsError, persist_claims_to_path};
use crate::wikidata::etl::EntityClaims;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use rusqlite::Connection;
use std::{cell::RefCell, path::PathBuf};
use tempfile::TempDir;

#[fixture]
pub fn temp_dir() -> TempDir {
    TempDir::new().expect("create temp dir")
}

#[fixture]
pub fn db_path() -> RefCell<Option<PathBuf>> {
    RefCell::new(None)
}

#[fixture]
pub fn claims() -> RefCell<Option<Vec<EntityClaims>>> {
    RefCell::new(None)
}

#[fixture]
pub fn persist_result() -> RefCell<Option<Result<(), PersistClaimsError>>> {
    RefCell::new(None)
}

fn create_pois_schema(path: &PathBuf) {
    let connection = Connection::open(path).expect("open SQLite database");
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

struct TestPoiData {
    id: i64,
    lon: f64,
    lat: f64,
    tags: String,
}

fn insert_poi(connection: &Connection, poi: &TestPoiData) {
    connection
        .execute(
            "INSERT INTO pois (id, lon, lat, tags) VALUES (?1, ?2, ?3, ?4)",
            (&poi.id, &poi.lon, &poi.lat, poi.tags.as_str()),
        )
        .expect("insert poi");
}

#[given("a SQLite POI database containing Berlin")]
fn sqlite_with_poi(temp_dir: &TempDir, db_path: &RefCell<Option<PathBuf>>) {
    let path = temp_dir.path().join("pois.db");
    create_pois_schema(&path);
    let connection = Connection::open(&path).expect("open SQLite database");
    insert_poi(
        &connection,
        &TestPoiData {
            id: 11,
            lon: 13.404954,
            lat: 52.520008,
            tags: "{\"wikidata\":\"Q64\"}".to_string(),
        },
    );
    *db_path.borrow_mut() = Some(path);
}

#[given("a SQLite POI database without the linked entity")]
fn sqlite_without_poi(temp_dir: &TempDir, db_path: &RefCell<Option<PathBuf>>) {
    let path = temp_dir.path().join("pois.db");
    create_pois_schema(&path);
    *db_path.borrow_mut() = Some(path);
}

#[given("extracted heritage claims for Berlin")]
fn extracted_claims(claims: &RefCell<Option<Vec<EntityClaims>>>) {
    *claims.borrow_mut() = Some(vec![EntityClaims {
        entity_id: "Q64".into(),
        linked_poi_ids: vec![11],
        heritage_designations: vec!["Q9259".into()],
    }]);
}

#[when("I persist the Wikidata claims")]
fn persist(
    db_path: &RefCell<Option<PathBuf>>,
    claims: &RefCell<Option<Vec<EntityClaims>>>,
    persist_result: &RefCell<Option<Result<(), PersistClaimsError>>>,
) {
    let path = db_path
        .borrow()
        .as_ref()
        .cloned()
        .unwrap_or_else(|| panic!("database path must be initialised"));
    let claims_vec = claims
        .borrow()
        .as_ref()
        .cloned()
        .unwrap_or_else(|| panic!("claims must be initialised"));
    let result = persist_claims_to_path(path.clone(), &claims_vec);
    *persist_result.borrow_mut() = Some(result);
}

#[then("the UNESCO heritage designation is stored for that POI")]
fn designation_persisted(
    db_path: &RefCell<Option<PathBuf>>,
    persist_result: &RefCell<Option<Result<(), PersistClaimsError>>>,
) {
    let binding = persist_result.borrow();
    let result = binding
        .as_ref()
        .unwrap_or_else(|| panic!("persistence result must be recorded"));
    if let Err(err) = result {
        panic!("expected success, got error: {err}");
    }

    let path = db_path
        .borrow()
        .as_ref()
        .cloned()
        .unwrap_or_else(|| panic!("database path must be initialised"));
    let connection = Connection::open(path).expect("open SQLite database");
    let mut statement = connection
        .prepare(
            "SELECT value_entity_id FROM poi_wikidata_claims WHERE poi_id = 11 AND property_id = 'P1435'",
        )
        .expect("prepare select");
    let designations: Vec<String> = statement
        .query_map([], |row| row.get(0))
        .expect("query designations")
        .collect::<Result<_, _>>()
        .expect("collect designations");
    assert_eq!(designations, vec!["Q9259".to_string()]);
}

#[then("persistence fails because the POI is missing")]
fn missing_poi_error(persist_result: &RefCell<Option<Result<(), PersistClaimsError>>>) {
    let binding = persist_result.borrow();
    let result = binding
        .as_ref()
        .unwrap_or_else(|| panic!("persistence result must be recorded"));
    match result {
        Ok(_) => panic!("expected an error for missing POI"),
        Err(PersistClaimsError::MissingPoi { poi_id, entity_id }) => {
            assert_eq!(*poi_id, 11);
            assert_eq!(entity_id, "Q64");
        }
        Err(other) => panic!("unexpected error: {other}"),
    }
}

#[scenario(path = "tests/features/persist_wikidata_claims.feature", index = 0)]
fn persist_claims_success(
    temp_dir: TempDir,
    db_path: RefCell<Option<PathBuf>>,
    claims: RefCell<Option<Vec<EntityClaims>>>,
    persist_result: RefCell<Option<Result<(), PersistClaimsError>>>,
) {
    let _ = (temp_dir, db_path, claims, persist_result);
}

#[scenario(path = "tests/features/persist_wikidata_claims.feature", index = 1)]
fn persist_claims_missing_poi(
    temp_dir: TempDir,
    db_path: RefCell<Option<PathBuf>>,
    claims: RefCell<Option<Vec<EntityClaims>>>,
    persist_result: RefCell<Option<Result<(), PersistClaimsError>>>,
) {
    let _ = (temp_dir, db_path, claims, persist_result);
}
