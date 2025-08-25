use geo::{Coord, Rect};
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

use wildside_core::{PoiStore, PointOfInterest, test_support::MemoryStore};

thread_local! {
    static RESULT: RefCell<Vec<PointOfInterest>> = const { RefCell::new(Vec::new()) };
}

#[given("a store containing a single POI at the origin")]
fn store() -> MemoryStore {
    RESULT.with(|cell| cell.borrow_mut().clear());
    let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    MemoryStore::with_poi(poi)
}

#[when("I query the bbox covering the origin")]
fn query_hit() {
    let store = store();
    let bbox = Rect::new(Coord { x: -1.0, y: -1.0 }, Coord { x: 1.0, y: 1.0 });
    let res: Vec<_> = store.get_pois_in_bbox(&bbox).collect();
    RESULT.with(|cell| *cell.borrow_mut() = res);
}

#[then("one POI is returned")]
fn one_poi() {
    RESULT.with(|cell| assert_eq!(cell.borrow().len(), 1));
}

#[scenario(path = "tests/features/poi_store.feature", index = 0)]
fn poi_returned() {}

#[when("I query the bbox that excludes the origin")]
fn query_miss() {
    let store = store();
    let bbox = Rect::new(Coord { x: 2.0, y: 2.0 }, Coord { x: 3.0, y: 3.0 });
    let res: Vec<_> = store.get_pois_in_bbox(&bbox).collect();
    RESULT.with(|cell| *cell.borrow_mut() = res);
}

#[then("no POIs are returned")]
fn no_poi() {
    RESULT.with(|cell| assert!(cell.borrow().is_empty()));
}

#[scenario(path = "tests/features/poi_store.feature", index = 1)]
fn empty_vec_when_outside_bbox() {}
