#![expect(
    clippy::expect_used,
    reason = "behaviour tests use expect for readable failures"
)]

//! Behavioural tests for golden routes using rstest-bdd.
//!
//! These scenarios exercise the VRP solver with well-defined problem instances
//! loaded from JSON files, verifying consistent behaviour across code changes.

mod golden_routes_support;

use std::cell::RefCell;
use std::collections::HashSet;
use std::time::Duration;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use wildside_core::test_support::{MemoryStore, TagScorer};
use wildside_core::{SolveResponse, Solver};
use wildside_solver_vrp::VrpSolver;
use wildside_solver_vrp::test_support::FixedMatrixTravelTimeProvider;

use golden_routes_support::{GoldenRoute, build_pois, build_request, load_golden_route};

/// World state for golden route BDD scenarios.
#[derive(Debug, Default)]
struct GoldenRouteWorld {
    golden: RefCell<Option<GoldenRoute>>,
    response: RefCell<Option<SolveResponse>>,
}

#[fixture]
fn world() -> GoldenRouteWorld {
    GoldenRouteWorld::default()
}

#[given("a golden route {name:word}")]
fn given_golden_route(world: &GoldenRouteWorld, name: String) {
    // Strip surrounding quotes that rstest-bdd may include from Gherkin syntax.
    let clean_name = name.trim_matches('"');
    let loaded = load_golden_route(clean_name);
    world.golden.replace(Some(loaded));
}

#[when("the VRP solver solves the golden route")]
fn when_solver_runs(world: &GoldenRouteWorld) {
    let borrowed_golden = world.golden.borrow();
    let golden_ref = borrowed_golden
        .as_ref()
        .expect("golden route should be loaded");

    let pois = build_pois(&golden_ref.pois);
    let request = build_request(&golden_ref.request);
    let store = MemoryStore::with_pois(pois);
    let provider =
        FixedMatrixTravelTimeProvider::from_seconds(golden_ref.travel_time_matrix_seconds.clone());
    let solver = VrpSolver::new(store, provider, TagScorer);

    let result = solver.solve(&request).expect("solve should succeed");
    world.response.replace(Some(result));
}

#[then("the route contains the expected POIs")]
fn then_route_matches(world: &GoldenRouteWorld) {
    let borrowed_golden = world.golden.borrow();
    let golden_ref = borrowed_golden
        .as_ref()
        .expect("golden route should be loaded");
    let borrowed_response = world.response.borrow();
    let response_ref = borrowed_response
        .as_ref()
        .expect("response should be recorded");

    // Compare as sets since VRP may produce equivalent routes in different orders.
    let actual_ids: HashSet<u64> = response_ref.route.pois().iter().map(|p| p.id).collect();
    let expected_ids: HashSet<u64> = golden_ref.expected.route_poi_ids.iter().copied().collect();
    assert_eq!(
        actual_ids, expected_ids,
        "route POI set mismatch (actual: {:?}, expected: {:?})",
        actual_ids, expected_ids
    );
}

#[then("the score is within expected range")]
fn then_score_in_range(world: &GoldenRouteWorld) {
    let borrowed_golden = world.golden.borrow();
    let golden_ref = borrowed_golden
        .as_ref()
        .expect("golden route should be loaded");
    let borrowed_response = world.response.borrow();
    let response_ref = borrowed_response
        .as_ref()
        .expect("response should be recorded");

    assert!(
        response_ref.score >= golden_ref.expected.min_score
            && response_ref.score <= golden_ref.expected.max_score,
        "score {} outside expected range [{}, {}]",
        response_ref.score,
        golden_ref.expected.min_score,
        golden_ref.expected.max_score
    );
}

#[then("the route respects the time budget")]
fn then_respects_budget(world: &GoldenRouteWorld) {
    let borrowed_golden = world.golden.borrow();
    let golden_ref = borrowed_golden
        .as_ref()
        .expect("golden route should be loaded");

    // Only check budget if the fixture expects it to be respected.
    if !golden_ref.expected.respects_budget {
        return;
    }

    let borrowed_response = world.response.borrow();
    let response_ref = borrowed_response
        .as_ref()
        .expect("response should be recorded");

    let budget = Duration::from_secs(u64::from(golden_ref.request.duration_minutes) * 60);
    assert!(
        response_ref.route.total_duration() <= budget,
        "route duration {:?} exceeds budget {:?}",
        response_ref.route.total_duration(),
        budget
    );
}

#[then("an empty route with zero score is returned")]
#[expect(clippy::float_cmp, reason = "testing exact zero score")]
fn then_empty_route(world: &GoldenRouteWorld) {
    let borrowed_response = world.response.borrow();
    let response_ref = borrowed_response
        .as_ref()
        .expect("response should be recorded");

    assert!(response_ref.route.pois().is_empty(), "expected empty route");
    assert_eq!(response_ref.score, 0.0, "expected zero score");
}

#[scenario(path = "tests/features/golden_routes.feature", index = 0)]
fn single_poi_visited(world: GoldenRouteWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/golden_routes.feature", index = 1)]
fn budget_constraint(world: GoldenRouteWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/golden_routes.feature", index = 2)]
fn empty_candidates(world: GoldenRouteWorld) {
    let _ = world;
}
