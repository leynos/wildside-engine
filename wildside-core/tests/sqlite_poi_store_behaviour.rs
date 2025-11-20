//! Behavioural tests for `SqlitePoiStore` using rstest-bdd.

use std::{cell::RefCell, path::PathBuf};

use geo::{Coord, Rect};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use wildside_core::{
    PoiStore, PointOfInterest, SqlitePoiStore, SqlitePoiStoreError,
    test_support::{write_sqlite_database, write_sqlite_spatial_index},
};

/// Provides shared state for SQLite store scenarios so functions keep a small
/// and readable argument surface.
#[derive(Debug)]
struct PoiStoreWorld {
    temp_dir: TempDir,
    dataset: RefCell<Vec<PointOfInterest>>,
    store_holder: RefCell<Option<SqlitePoiStore>>,
    store_error: RefCell<Option<SqlitePoiStoreError>>,
    query_results: RefCell<Vec<PointOfInterest>>,
    paths: RefCell<Option<(PathBuf, PathBuf)>>,
}

impl PoiStoreWorld {
    fn new() -> Self {
        Self {
            temp_dir: TempDir::new().expect("create temp dir"),
            dataset: RefCell::new(Vec::new()),
            store_holder: RefCell::new(None),
            store_error: RefCell::new(None),
            query_results: RefCell::new(Vec::new()),
            paths: RefCell::new(None),
        }
    }

    fn temp_dir(&self) -> &TempDir {
        &self.temp_dir
    }

    fn dataset(&self) -> &RefCell<Vec<PointOfInterest>> {
        &self.dataset
    }

    fn store_holder(&self) -> &RefCell<Option<SqlitePoiStore>> {
        &self.store_holder
    }

    fn store_error(&self) -> &RefCell<Option<SqlitePoiStoreError>> {
        &self.store_error
    }

    fn query_results(&self) -> &RefCell<Vec<PointOfInterest>> {
        &self.query_results
    }

    fn paths(&self) -> &RefCell<Option<(PathBuf, PathBuf)>> {
        &self.paths
    }

    fn expect_paths(&self) -> (PathBuf, PathBuf) {
        self.paths()
            .borrow()
            .as_ref()
            .cloned()
            .expect("paths should be initialised before opening the store")
    }
}

#[fixture]
fn world() -> PoiStoreWorld {
    PoiStoreWorld::new()
}

fn bbox(x1: f64, y1: f64, x2: f64, y2: f64) -> Rect<f64> {
    Rect::new(Coord { x: x1, y: y1 }, Coord { x: x2, y: y2 })
}

fn persist_dataset(world: &PoiStoreWorld, pois: Vec<PointOfInterest>) {
    let db_path = world.temp_dir().path().join("pois.db");
    let index_path = world.temp_dir().path().join("pois.rstar");
    write_sqlite_database(&db_path, &pois).expect("persist database");
    write_sqlite_spatial_index(&index_path, &pois).expect("persist index");
    world.paths().replace(Some((db_path, index_path)));
    world.dataset().replace(pois);
}

#[given("a temporary directory for SQLite artefacts")]
fn given_temp_dir(world: &PoiStoreWorld) {
    let _ = world.temp_dir();
}

#[given("a SQLite POI dataset containing a point at the origin")]
fn given_dataset(world: &PoiStoreWorld) {
    let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    persist_dataset(world, vec![poi]);
}

#[given("a SQLite POI dataset containing multiple points near the origin")]
fn given_multi_poi_dataset(world: &PoiStoreWorld) {
    let pois = vec![
        PointOfInterest::with_empty_tags(1, Coord { x: -0.2, y: -0.2 }),
        PointOfInterest::with_empty_tags(2, Coord { x: 0.4, y: 0.4 }),
        PointOfInterest::with_empty_tags(3, Coord { x: 2.0, y: 2.0 }),
    ];
    persist_dataset(world, pois);
}

#[given("a SQLite dataset whose index references a missing POI")]
fn given_inconsistent_dataset(world: &PoiStoreWorld) {
    let stored = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    let ghost = PointOfInterest::with_empty_tags(2, Coord { x: 1.0, y: 1.0 });
    let db_path = world.temp_dir().path().join("pois.db");
    let index_path = world.temp_dir().path().join("pois.rstar");
    write_sqlite_database(&db_path, std::slice::from_ref(&stored)).expect("persist database");
    write_sqlite_spatial_index(&index_path, &[stored.clone(), ghost]).expect("persist index");
    world.paths().replace(Some((db_path, index_path)));
    world.dataset().replace(vec![stored]);
}

#[when("I open the SQLite POI store")]
fn open_store(world: &PoiStoreWorld) {
    let (db_path, index_path) = world.expect_paths();
    match SqlitePoiStore::open(&db_path, &index_path) {
        Ok(store) => {
            world.store_holder().replace(Some(store));
            world.store_error().replace(None);
        }
        Err(err) => {
            world.store_holder().replace(None);
            world.store_error().replace(Some(err));
        }
    }
}

fn query_bbox_helper(world: &PoiStoreWorld, coords: (f64, f64, f64, f64)) {
    assert_no_store_error(world);
    let (x1, y1, x2, y2) = coords;
    let bbox = bbox(x1, y1, x2, y2);
    let results = {
        let borrowed_store = world.store_holder().borrow();
        let store = borrowed_store
            .as_ref()
            .expect("store should be available for querying");
        store.get_pois_in_bbox(&bbox).collect()
    };
    world.query_results().replace(results);
}

fn assert_no_store_error(world: &PoiStoreWorld) {
    assert!(
        world.store_error().borrow().is_none(),
        "unexpected store error",
    );
}

#[when("I query the bbox covering the origin")]
fn query_origin(world: &PoiStoreWorld) {
    query_bbox_helper(world, (-0.5, -0.5, 0.5, 0.5));
}

#[when("I query the bbox covering multiple POIs")]
fn query_multiple(world: &PoiStoreWorld) {
    query_bbox_helper(world, (-0.3, -0.3, 0.6, 0.6));
}

#[when("I query the bbox that excludes the origin")]
fn query_outside(world: &PoiStoreWorld) {
    query_bbox_helper(world, (2.0, 2.0, 3.0, 3.0));
}

#[then("one POI is returned from the SQLite store")]
fn then_one_result(world: &PoiStoreWorld) {
    assert_no_store_error(world);
    let expected = world.dataset().borrow();
    let results = world.query_results().borrow();
    assert_eq!(results.len(), 1, "expected exactly one POI");
    assert_eq!(results[0], expected[0]);
}

#[then("no POIs are returned from the SQLite store")]
fn then_no_results(world: &PoiStoreWorld) {
    assert_no_store_error(world);
    assert!(
        world.query_results().borrow().is_empty(),
        "expected no POIs"
    );
}

#[then("exactly two POIs are returned from the SQLite store")]
fn then_two_results(world: &PoiStoreWorld) {
    assert_no_store_error(world);
    let results = world.query_results().borrow();
    assert_eq!(results.len(), 2, "expected exactly two POIs");
    let ids: Vec<_> = results.iter().map(|poi| poi.id).collect();
    assert_eq!(ids, vec![1, 2], "unexpected POI identifiers");
}

#[then("opening the SQLite store fails with a missing POI error")]
fn then_missing_poi_error(world: &PoiStoreWorld) {
    let binding = world.store_error().borrow();
    let error = binding.as_ref().expect("an error should be recorded");
    assert!(matches!(error, SqlitePoiStoreError::MissingPoi { .. }));
}

#[scenario(path = "tests/features/sqlite_poi_store.feature", index = 0)]
fn poi_returned(world: PoiStoreWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sqlite_poi_store.feature", index = 1)]
fn empty_result(world: PoiStoreWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sqlite_poi_store.feature", index = 2)]
fn missing_poi_error(world: PoiStoreWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sqlite_poi_store.feature", index = 3)]
fn multiple_results(world: PoiStoreWorld) {
    let _ = world;
}
