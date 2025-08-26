//! Behavioural tests for `PoiStore` bounding-box queries.

use geo::{Coord, Rect};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use wildside_core::{PoiStore, PointOfInterest, test_support::MemoryStore};

fn bbox(x1: f64, y1: f64, x2: f64, y2: f64) -> Rect<f64> {
    Rect::new(Coord { x: x1, y: y1 }, Coord { x: x2, y: y2 })
}

#[fixture]
fn store() -> MemoryStore {
    let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    MemoryStore::with_poi(poi)
}

#[fixture]
fn results() -> RefCell<Vec<PointOfInterest>> {
    RefCell::new(Vec::new())
}

#[given("a store containing a single POI at the origin")]
fn given_store(
    #[from(store)] _store: &MemoryStore,
    #[from(results)] results: &RefCell<Vec<PointOfInterest>>,
) {
    results.borrow_mut().clear();
}

#[when("I query the bbox covering the origin")]
fn query_hit(
    #[from(store)] store: &MemoryStore,
    #[from(results)] results: &RefCell<Vec<PointOfInterest>>,
) {
    let bbox = bbox(-1.0, -1.0, 1.0, 1.0);
    *results.borrow_mut() = store.get_pois_in_bbox(&bbox).collect();
}

#[when("I query the bbox that excludes the origin")]
fn query_miss(
    #[from(store)] store: &MemoryStore,
    #[from(results)] results: &RefCell<Vec<PointOfInterest>>,
) {
    let bbox = bbox(2.0, 2.0, 3.0, 3.0);
    *results.borrow_mut() = store.get_pois_in_bbox(&bbox).collect();
}

#[when("I query the bbox whose edge passes through the origin")]
fn query_boundary_hit(
    #[from(store)] store: &MemoryStore,
    #[from(results)] results: &RefCell<Vec<PointOfInterest>>,
) {
    let bbox = bbox(0.0, -1.0, 1.0, 1.0);
    *results.borrow_mut() = store.get_pois_in_bbox(&bbox).collect();
}

#[when("I query the bbox defined with reversed corners but covering the origin")]
fn query_hit_reversed(
    #[from(store)] store: &MemoryStore,
    #[from(results)] results: &RefCell<Vec<PointOfInterest>>,
) {
    let bbox = bbox(1.0, 1.0, -1.0, -1.0);
    *results.borrow_mut() = store.get_pois_in_bbox(&bbox).collect();
}

#[then("one POI is returned")]
fn one_poi(#[from(results)] results: &RefCell<Vec<PointOfInterest>>) {
    let results = results.borrow();
    assert_eq!(results.len(), 1, "expected exactly one POI within the bbox");
    // Also verify identity/content
    let poi = results.first().expect("one result present");
    assert_eq!(
        poi.location,
        Coord { x: 0.0, y: 0.0 },
        "expected the origin POI to be returned",
    );
}

#[then("no POIs are returned")]
fn no_poi(#[from(results)] results: &RefCell<Vec<PointOfInterest>>) {
    assert!(
        results.borrow().is_empty(),
        "expected no POIs within the bbox"
    );
}

#[scenario(path = "tests/features/poi_store.feature", index = 0)]
fn poi_returned(store: MemoryStore, results: RefCell<Vec<PointOfInterest>>) {
    let _ = (store, results);
}

#[scenario(path = "tests/features/poi_store.feature", index = 1)]
fn empty_vec_when_outside_bbox(store: MemoryStore, results: RefCell<Vec<PointOfInterest>>) {
    let _ = (store, results);
}

#[scenario(path = "tests/features/poi_store.feature", index = 2)]
fn boundary_inclusive(store: MemoryStore, results: RefCell<Vec<PointOfInterest>>) {
    let _ = (store, results);
}

#[scenario(path = "tests/features/poi_store.feature", index = 3)]
fn reversed_corners(store: MemoryStore, results: RefCell<Vec<PointOfInterest>>) {
    let _ = (store, results);
}
