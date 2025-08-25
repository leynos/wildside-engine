use geo::{Contains, Coord, Rect};
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

use wildside_core::{PoiStore, PointOfInterest};

struct MemoryStore {
    pois: Vec<PointOfInterest>,
}

impl PoiStore for MemoryStore {
    fn get_pois_in_bbox(&self, bbox: &Rect<f64>) -> Vec<PointOfInterest> {
        self.pois
            .iter()
            .filter(|p| bbox.contains(&p.location))
            .cloned()
            .collect()
    }
}

thread_local! { static RESULT: RefCell<Option<Vec<PointOfInterest>>> = const { RefCell::new(None) }; }

#[given("a store containing a single POI at the origin")]
fn store() -> MemoryStore {
    let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    MemoryStore { pois: vec![poi] }
}

#[when("I query the bbox covering the origin")]
fn query_hit() {
    let store = store();
    let bbox = Rect::new(Coord { x: -1.0, y: -1.0 }, Coord { x: 1.0, y: 1.0 });
    let res = store.get_pois_in_bbox(&bbox);
    RESULT.with(|cell| cell.replace(Some(res)));
}

#[then("one POI is returned")]
fn one_poi() {
    RESULT.with(|cell| {
        let result = cell.borrow();
        assert_eq!(result.as_ref().unwrap().len(), 1);
    });
}

#[scenario(path = "tests/features/poi_store.feature", index = 0)]
fn poi_returned() {}

#[when("I query the bbox that excludes the origin")]
fn query_miss() {
    let store = store();
    let bbox = Rect::new(Coord { x: 2.0, y: 2.0 }, Coord { x: 3.0, y: 3.0 });
    let res = store.get_pois_in_bbox(&bbox);
    RESULT.with(|cell| cell.replace(Some(res)));
}

#[then("no POIs are returned")]
fn no_poi() {
    RESULT.with(|cell| {
        let result = cell.borrow();
        assert!(result.as_ref().unwrap().is_empty());
    });
}

#[scenario(path = "tests/features/poi_store.feature", index = 1)]
fn empty_vec_when_outside_bbox() {}
