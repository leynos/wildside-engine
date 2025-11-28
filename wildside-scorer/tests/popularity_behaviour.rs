//! Behavioural coverage for computing global popularity scores.

use std::cell::RefCell;

use camino::Utf8PathBuf;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use rusqlite::Connection;
use tempfile::TempDir;
use wildside_scorer::{
    PopularityError, PopularityScores, PopularityWeights, compute_popularity_scores,
};

/// Temporary directory for each scenario.
#[fixture]
pub fn temp_dir() -> TempDir {
    match TempDir::new() {
        Ok(dir) => dir,
        Err(err) => panic!("create temporary directory: {err}"),
    }
}

/// Shared location for the `SQLite` database under test.
#[fixture]
pub fn db_path() -> RefCell<Option<Utf8PathBuf>> {
    RefCell::new(None)
}

/// Default popularity weights used by scenarios.
#[fixture]
pub fn weights() -> PopularityWeights {
    PopularityWeights::default()
}

/// Captures the outcome of popularity computation for assertions.
#[fixture]
pub fn compute_result() -> RefCell<Option<Result<PopularityScores, PopularityError>>> {
    RefCell::new(None)
}

#[given("a SQLite POI database with sitelink counts")]
#[expect(
    clippy::expect_used,
    reason = "fixture initialisation should fail fast when database operations fail"
)]
fn sqlite_with_sitelinks(temp_dir: &TempDir, db_path: &RefCell<Option<Utf8PathBuf>>) {
    let path = Utf8PathBuf::from_path_buf(temp_dir.path().join("pois.db")).expect("utf8 path");
    let connection = Connection::open(path.as_std_path()).expect("open sqlite database");
    create_schema(&connection);
    insert_poi(&connection, 1, "Q1", "{\"wikidata\":\"Q1\"}");
    insert_poi(&connection, 2, "Q2", "{\"wikidata\":\"Q2\"}");
    insert_poi(&connection, 3, "Q3", "{}");
    link_entity(&connection, 1, "Q1");
    link_entity(&connection, 2, "Q2");
    insert_heritage_claim(&connection, "Q1");
    connection
        .execute(
            "CREATE TABLE wikidata_entity_sitelinks (
                entity_id TEXT PRIMARY KEY,
                sitelink_count INTEGER NOT NULL
            )",
            [],
        )
        .expect("create sitelink table");
    connection
        .execute(
            "INSERT INTO wikidata_entity_sitelinks (entity_id, sitelink_count) VALUES ('Q1', 50)",
            [],
        )
        .expect("insert sitelinks for Q1");
    connection
        .execute(
            "INSERT INTO wikidata_entity_sitelinks (entity_id, sitelink_count) VALUES ('Q2', 10)",
            [],
        )
        .expect("insert sitelinks for Q2");
    *db_path.borrow_mut() = Some(path);
}

#[given("a SQLite POI database with malformed sitelinks")]
#[expect(
    clippy::expect_used,
    reason = "fixture initialisation should fail fast when database operations fail"
)]
fn sqlite_with_invalid_sitelinks(temp_dir: &TempDir, db_path: &RefCell<Option<Utf8PathBuf>>) {
    let path = Utf8PathBuf::from_path_buf(temp_dir.path().join("pois.db")).expect("utf8 path");
    let connection = Connection::open(path.as_std_path()).expect("open sqlite database");
    create_schema(&connection);
    insert_poi(
        &connection,
        7,
        "Q7",
        "{\"wikidata\":\"Q7\",\"sitelinks\":\"many\"}",
    );
    link_entity(&connection, 7, "Q7");
    *db_path.borrow_mut() = Some(path);
}

#[when("I compute popularity scores")]
fn compute_scores(
    db_path: &RefCell<Option<Utf8PathBuf>>,
    weights: PopularityWeights,
    compute_result: &RefCell<Option<Result<PopularityScores, PopularityError>>>,
) {
    let path = db_path
        .borrow()
        .as_ref()
        .cloned()
        .unwrap_or_else(|| panic!("database path must be initialised"));
    let result = compute_popularity_scores(&path, weights);
    *compute_result.borrow_mut() = Some(result);
}

#[then("the heritage POI has the highest normalised score")]
#[expect(
    clippy::float_arithmetic,
    reason = "assertions compare floating-point scores"
)]
fn heritage_scores_highest(
    compute_result: &RefCell<Option<Result<PopularityScores, PopularityError>>>,
) {
    let binding = compute_result.borrow();
    let result = binding
        .as_ref()
        .unwrap_or_else(|| panic!("computation result must be recorded"));
    match result {
        Ok(scores) => {
            let Some(q1) = scores.get(1) else {
                panic!("score for heritage poi")
            };
            let Some(q2) = scores.get(2) else {
                panic!("score for non-heritage poi")
            };
            assert!(
                (q1 - 1.0_f32).abs() < 0.000_1_f32,
                "heritage POI should normalise to 1.0 (got {q1})"
            );
            assert!(
                q2 < 1.0_f32,
                "non-heritage POI should have a lower score (got {q2})"
            );
        }
        Err(err) => panic!("popularity computation should succeed, got {err}"),
    }
}

#[then("popularity computation fails because sitelinks are invalid")]
fn computation_fails(compute_result: &RefCell<Option<Result<PopularityScores, PopularityError>>>) {
    let binding = compute_result.borrow();
    let result = binding
        .as_ref()
        .unwrap_or_else(|| panic!("computation result must be recorded"));
    match result {
        Ok(_) => panic!("expected popularity computation to fail"),
        Err(PopularityError::InvalidSitelinkCountJson { poi_id, .. }) => {
            assert_eq!(*poi_id, 7_u64);
        }
        Err(other) => panic!("unexpected error: {other}"),
    }
}

#[then("the unlinked POI has a zero normalised score")]
#[expect(
    clippy::float_arithmetic,
    reason = "assertions compare floating-point scores"
)]
fn unlinked_poi_scores_zero(
    compute_result: &RefCell<Option<Result<PopularityScores, PopularityError>>>,
) {
    let binding = compute_result.borrow();
    let result = binding
        .as_ref()
        .unwrap_or_else(|| panic!("computation result must be recorded"));
    match result {
        Ok(scores) => {
            let Some(q3) = scores.get(3) else {
                panic!("score for unlinked poi")
            };
            assert!(
                (q3 - 0.0_f32).abs() < 0.000_1_f32,
                "unlinked POI should normalise to 0.0 (got {q3})"
            );
        }
        Err(err) => panic!("popularity computation should succeed, got {err}"),
    }
}

#[expect(
    clippy::expect_used,
    reason = "schema setup should panic when the database is unavailable"
)]
fn create_schema(connection: &Connection) {
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
    connection
        .execute(
            "CREATE TABLE poi_wikidata_links (
                poi_id INTEGER NOT NULL,
                entity_id TEXT NOT NULL,
                PRIMARY KEY (poi_id, entity_id)
            )",
            [],
        )
        .expect("create links table");
    connection
        .execute(
            "CREATE TABLE wikidata_entity_claims (
                entity_id TEXT NOT NULL,
                property_id TEXT NOT NULL,
                value_entity_id TEXT NOT NULL
            )",
            [],
        )
        .expect("create claims table");
}

fn insert_poi(connection: &Connection, id: i64, entity: &str, tags: &str) {
    connection
        .execute(
            "INSERT INTO pois (id, lon, lat, tags) VALUES (?1, 0.0, 0.0, ?2)",
            (&id, tags),
        )
        .unwrap_or_else(|err| panic!("insert poi {id} for entity {entity}: {err}"));
}

fn link_entity(connection: &Connection, poi_id: i64, entity: &str) {
    connection
        .execute(
            "INSERT INTO poi_wikidata_links (poi_id, entity_id) VALUES (?1, ?2)",
            (poi_id, entity),
        )
        .unwrap_or_else(|err| panic!("link poi {poi_id} to {entity}: {err}"));
}

fn insert_heritage_claim(connection: &Connection, entity: &str) {
    connection
        .execute(
            "INSERT INTO wikidata_entity_claims (entity_id, property_id, value_entity_id) VALUES (?1, 'P1435', 'Q9259')",
            [entity],
        )
        .unwrap_or_else(|err| panic!("insert heritage claim for {entity}: {err}"));
}

#[scenario(path = "tests/features/popularity.feature", index = 0)]
fn heritage_scores_highest_when_sitelinks_present(
    temp_dir: TempDir,
    db_path: RefCell<Option<Utf8PathBuf>>,
    weights: PopularityWeights,
    compute_result: RefCell<Option<Result<PopularityScores, PopularityError>>>,
) {
    let _ = (temp_dir, db_path, weights, compute_result);
}

#[scenario(path = "tests/features/popularity.feature", index = 1)]
fn invalid_sitelinks_fail(
    temp_dir: TempDir,
    db_path: RefCell<Option<Utf8PathBuf>>,
    weights: PopularityWeights,
    compute_result: RefCell<Option<Result<PopularityScores, PopularityError>>>,
) {
    let _ = (temp_dir, db_path, weights, compute_result);
}

#[scenario(path = "tests/features/popularity.feature", index = 2)]
fn unlinked_poi_scores_zero_scenario(
    temp_dir: TempDir,
    db_path: RefCell<Option<Utf8PathBuf>>,
    weights: PopularityWeights,
    compute_result: RefCell<Option<Result<PopularityScores, PopularityError>>>,
) {
    let _ = (temp_dir, db_path, weights, compute_result);
}
