//! Behavioural tests for building and querying the spatial index.

use geo::Coord;
use rstar::{AABB, RTree};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use wildside_core::{PointOfInterest, build_spatial_index};

fn point(id: u64, x: f64, y: f64) -> PointOfInterest {
    PointOfInterest::with_empty_tags(id, Coord { x, y })
}

fn bbox(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> AABB<[f64; 2]> {
    AABB::from_corners([min_x, min_y], [max_x, max_y])
}

#[fixture]
fn pois() -> RefCell<Vec<PointOfInterest>> {
    RefCell::new(Vec::new())
}

#[fixture]
fn tree() -> RefCell<Option<RTree<PointOfInterest>>> {
    RefCell::new(None)
}

#[fixture]
fn results() -> RefCell<Vec<PointOfInterest>> {
    RefCell::new(Vec::new())
}

#[given("a collection of POIs including the city centre and riverside landmarks")]
fn given_sample_pois(#[from(pois)] pois: &RefCell<Vec<PointOfInterest>>) {
    *pois.borrow_mut() = vec![point(1, 0.0, 0.0), point(2, 5.0, 1.0)];
}

#[given("an empty collection of POIs")]
fn given_empty_pois(#[from(pois)] pois: &RefCell<Vec<PointOfInterest>>) {
    pois.borrow_mut().clear();
}

#[when("I build the spatial index")]
fn when_build_index(
    #[from(pois)] pois: &RefCell<Vec<PointOfInterest>>,
    #[from(tree)] tree: &RefCell<Option<RTree<PointOfInterest>>>,
) {
    let built = build_spatial_index(&pois.borrow());
    *tree.borrow_mut() = Some(built);
}

#[when("I query the bbox that covers the city centre landmark")]
fn when_query_hits(
    #[from(tree)] tree: &RefCell<Option<RTree<PointOfInterest>>>,
    #[from(results)] results: &RefCell<Vec<PointOfInterest>>,
) {
    let bbox = bbox(-0.5, -0.5, 0.5, 0.5);
    let matches = tree
        .borrow()
        .as_ref()
        .expect("tree built")
        .locate_in_envelope_intersecting(&bbox)
        .cloned()
        .collect();
    *results.borrow_mut() = matches;
}

#[when("I query the bbox that excludes all landmarks")]
fn when_query_misses(
    #[from(tree)] tree: &RefCell<Option<RTree<PointOfInterest>>>,
    #[from(results)] results: &RefCell<Vec<PointOfInterest>>,
) {
    let bbox = bbox(10.0, 10.0, 11.0, 11.0);
    let matches = tree
        .borrow()
        .as_ref()
        .expect("tree built")
        .locate_in_envelope_intersecting(&bbox)
        .cloned()
        .collect();
    *results.borrow_mut() = matches;
}

#[then("exactly one POI with id 1 is returned")]
fn then_single_result(#[from(results)] results: &RefCell<Vec<PointOfInterest>>) {
    let results = results.borrow();
    assert_eq!(results.len(), 1, "expected a single matching POI");
    let poi = results.first().expect("one result present");
    assert_eq!(poi.id, 1, "expected the city centre POI to be returned");
}

#[then("no POIs are returned")]
fn then_no_results(#[from(results)] results: &RefCell<Vec<PointOfInterest>>) {
    assert!(results.borrow().is_empty(), "expected no results");
}

#[then("the spatial index is empty")]
fn then_tree_empty(#[from(tree)] tree: &RefCell<Option<RTree<PointOfInterest>>>) {
    let size = tree.borrow().as_ref().expect("tree built").size();
    assert_eq!(size, 0, "expected an empty spatial index");
}

#[scenario(path = "tests/features/spatial_index.feature", index = 0)]
fn scenario_query_hit(
    pois: RefCell<Vec<PointOfInterest>>,
    tree: RefCell<Option<RTree<PointOfInterest>>>,
    results: RefCell<Vec<PointOfInterest>>,
) {
    let _ = (pois, tree, results);
}

#[scenario(path = "tests/features/spatial_index.feature", index = 1)]
fn scenario_query_miss(
    pois: RefCell<Vec<PointOfInterest>>,
    tree: RefCell<Option<RTree<PointOfInterest>>>,
    results: RefCell<Vec<PointOfInterest>>,
) {
    let _ = (pois, tree, results);
}

#[scenario(path = "tests/features/spatial_index.feature", index = 2)]
fn scenario_empty_tree(
    pois: RefCell<Vec<PointOfInterest>>,
    tree: RefCell<Option<RTree<PointOfInterest>>>,
    results: RefCell<Vec<PointOfInterest>>,
) {
    let _ = (pois, tree, results);
}
