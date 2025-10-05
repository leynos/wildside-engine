//! Behavioural tests for `SqlitePoiStore` using rstest-bdd.

use std::cell::RefCell;

use geo::{Coord, Rect};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use wildside_core::{
    PoiStore, PointOfInterest, SqlitePoiStore, SqlitePoiStoreError,
    test_support::{write_sqlite_database, write_sqlite_spatial_index},
};

fn bbox(x1: f64, y1: f64, x2: f64, y2: f64) -> Rect<f64> {
    Rect::new(Coord { x: x1, y: y1 }, Coord { x: x2, y: y2 })
}

#[fixture]
fn temp_dir() -> TempDir {
    TempDir::new().expect("create temp dir")
}

#[fixture]
fn dataset() -> RefCell<Vec<PointOfInterest>> {
    RefCell::new(Vec::new())
}

#[fixture]
fn store_holder() -> RefCell<Option<SqlitePoiStore>> {
    RefCell::new(None)
}

#[fixture]
fn store_error() -> RefCell<Option<SqlitePoiStoreError>> {
    RefCell::new(None)
}

#[fixture]
fn query_results() -> RefCell<Vec<PointOfInterest>> {
    RefCell::new(Vec::new())
}

#[fixture]
fn paths() -> RefCell<Option<(std::path::PathBuf, std::path::PathBuf)>> {
    RefCell::new(None)
}

#[given("a temporary directory for SQLite artefacts")]
fn given_temp_dir(temp_dir: &TempDir) {
    let _ = temp_dir;
}

#[given("a SQLite POI dataset containing a point at the origin")]
fn given_dataset(
    temp_dir: &TempDir,
    paths: &RefCell<Option<(std::path::PathBuf, std::path::PathBuf)>>,
    dataset: &RefCell<Vec<PointOfInterest>>,
) {
    let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    let db_path = temp_dir.path().join("pois.db");
    let index_path = temp_dir.path().join("pois.rstar");
    write_sqlite_database(&db_path, std::slice::from_ref(&poi)).expect("persist database");
    write_sqlite_spatial_index(&index_path, std::slice::from_ref(&poi)).expect("persist index");
    paths.replace(Some((db_path, index_path)));
    dataset.replace(vec![poi]);
}

#[given("a SQLite dataset whose index references a missing POI")]
fn given_inconsistent_dataset(
    temp_dir: &TempDir,
    paths: &RefCell<Option<(std::path::PathBuf, std::path::PathBuf)>>,
    dataset: &RefCell<Vec<PointOfInterest>>,
) {
    let stored = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    let ghost = PointOfInterest::with_empty_tags(2, Coord { x: 1.0, y: 1.0 });
    let db_path = temp_dir.path().join("pois.db");
    let index_path = temp_dir.path().join("pois.rstar");
    write_sqlite_database(&db_path, std::slice::from_ref(&stored)).expect("persist database");
    write_sqlite_spatial_index(&index_path, &[stored.clone(), ghost]).expect("persist index");
    paths.replace(Some((db_path, index_path)));
    dataset.replace(vec![stored]);
}

#[when("I open the SQLite POI store")]
fn open_store(
    paths: &RefCell<Option<(std::path::PathBuf, std::path::PathBuf)>>,
    store_holder: &RefCell<Option<SqlitePoiStore>>,
    store_error: &RefCell<Option<SqlitePoiStoreError>>,
) {
    let (db_path, index_path) = paths
        .borrow()
        .as_ref()
        .cloned()
        .expect("paths should be initialised before opening the store");
    match SqlitePoiStore::open(&db_path, &index_path) {
        Ok(store) => {
            store_holder.replace(Some(store));
            store_error.replace(None);
        }
        Err(err) => {
            store_holder.replace(None);
            store_error.replace(Some(err));
        }
    }
}

fn query_bbox_helper(
    store_holder: &RefCell<Option<SqlitePoiStore>>,
    query_results: &RefCell<Vec<PointOfInterest>>,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
) {
    let binding = store_holder.borrow();
    let store = binding
        .as_ref()
        .expect("store should be available for querying");
    let bbox = bbox(x1, y1, x2, y2);
    query_results.replace(store.get_pois_in_bbox(&bbox).collect());
}

#[when("I query the bbox covering the origin")]
fn query_origin(
    store_holder: &RefCell<Option<SqlitePoiStore>>,
    query_results: &RefCell<Vec<PointOfInterest>>,
) {
    query_bbox_helper(store_holder, query_results, -0.5, -0.5, 0.5, 0.5);
}

#[when("I query the bbox that excludes the origin")]
fn query_outside(
    store_holder: &RefCell<Option<SqlitePoiStore>>,
    query_results: &RefCell<Vec<PointOfInterest>>,
) {
    query_bbox_helper(store_holder, query_results, 2.0, 2.0, 3.0, 3.0);
}

#[then("one POI is returned from the SQLite store")]
fn then_one_result(
    dataset: &RefCell<Vec<PointOfInterest>>,
    query_results: &RefCell<Vec<PointOfInterest>>,
    store_error: &RefCell<Option<SqlitePoiStoreError>>,
) {
    assert!(store_error.borrow().is_none(), "unexpected store error");
    let expected = dataset.borrow();
    let results = query_results.borrow();
    assert_eq!(results.len(), 1, "expected exactly one POI");
    assert_eq!(results[0], expected[0]);
}

#[then("no POIs are returned from the SQLite store")]
fn then_no_results(
    query_results: &RefCell<Vec<PointOfInterest>>,
    store_error: &RefCell<Option<SqlitePoiStoreError>>,
) {
    assert!(store_error.borrow().is_none(), "unexpected store error");
    assert!(query_results.borrow().is_empty(), "expected no POIs");
}

#[then("opening the SQLite store fails with a missing POI error")]
fn then_missing_poi_error(store_error: &RefCell<Option<SqlitePoiStoreError>>) {
    let binding = store_error.borrow();
    let error = binding.as_ref().expect("an error should be recorded");
    assert!(matches!(error, SqlitePoiStoreError::MissingPoi { .. }));
}

#[scenario(path = "tests/features/sqlite_poi_store.feature", index = 0)]
fn poi_returned(
    temp_dir: TempDir,
    dataset: RefCell<Vec<PointOfInterest>>,
    store_holder: RefCell<Option<SqlitePoiStore>>,
    store_error: RefCell<Option<SqlitePoiStoreError>>,
    query_results: RefCell<Vec<PointOfInterest>>,
    paths: RefCell<Option<(std::path::PathBuf, std::path::PathBuf)>>,
) {
    let _ = (
        temp_dir,
        dataset,
        store_holder,
        store_error,
        query_results,
        paths,
    );
}

#[scenario(path = "tests/features/sqlite_poi_store.feature", index = 1)]
fn empty_result(
    temp_dir: TempDir,
    dataset: RefCell<Vec<PointOfInterest>>,
    store_holder: RefCell<Option<SqlitePoiStore>>,
    store_error: RefCell<Option<SqlitePoiStoreError>>,
    query_results: RefCell<Vec<PointOfInterest>>,
    paths: RefCell<Option<(std::path::PathBuf, std::path::PathBuf)>>,
) {
    let _ = (
        temp_dir,
        dataset,
        store_holder,
        store_error,
        query_results,
        paths,
    );
}

#[scenario(path = "tests/features/sqlite_poi_store.feature", index = 2)]
fn missing_poi_error(
    temp_dir: TempDir,
    dataset: RefCell<Vec<PointOfInterest>>,
    store_holder: RefCell<Option<SqlitePoiStore>>,
    store_error: RefCell<Option<SqlitePoiStoreError>>,
    query_results: RefCell<Vec<PointOfInterest>>,
    paths: RefCell<Option<(std::path::PathBuf, std::path::PathBuf)>>,
) {
    let _ = (
        temp_dir,
        dataset,
        store_holder,
        store_error,
        query_results,
        paths,
    );
}
