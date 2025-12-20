#![expect(
    clippy::expect_used,
    reason = "behaviour tests use expect for readable failures"
)]

//! Behavioural tests for golden routes using rstest-bdd.
//!
//! These scenarios exercise the VRP solver with well-defined problem instances
//! loaded from JSON files, verifying consistent behaviour across code changes.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use geo::Coord;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde::Deserialize;
use wildside_core::test_support::{MemoryStore, TagScorer};
use wildside_core::{
    InterestProfile, PointOfInterest, SolveRequest, SolveResponse, Solver, Tags, Theme,
};
use wildside_solver_vrp::VrpSolver;
use wildside_solver_vrp::test_support::FixedMatrixTravelTimeProvider;

/// Deserialised golden route test case.
#[derive(Debug, Deserialize, Clone)]
struct GoldenRoute {
    #[expect(dead_code, reason = "kept for debugging test failures")]
    name: String,
    #[expect(dead_code, reason = "kept for documentation in JSON files")]
    description: String,
    pois: Vec<PoiSpec>,
    travel_time_matrix_seconds: Vec<Vec<u64>>,
    request: RequestSpec,
    expected: ExpectedResult,
}

/// POI specification from JSON.
#[derive(Debug, Deserialize, Clone)]
struct PoiSpec {
    id: u64,
    x: f64,
    y: f64,
    tags: HashMap<String, String>,
}

/// Request specification from JSON.
#[derive(Debug, Deserialize, Clone)]
struct RequestSpec {
    start: CoordSpec,
    end: Option<CoordSpec>,
    duration_minutes: u16,
    interests: HashMap<String, f32>,
    seed: u64,
    max_nodes: Option<u16>,
}

/// Coordinate specification from JSON.
#[derive(Debug, Deserialize, Clone)]
struct CoordSpec {
    x: f64,
    y: f64,
}

/// Expected result from JSON.
#[derive(Debug, Deserialize, Clone)]
struct ExpectedResult {
    route_poi_ids: Vec<u64>,
    min_score: f32,
    max_score: f32,
    #[expect(dead_code, reason = "reserved for future budget validation tests")]
    respects_budget: bool,
}

/// Load a golden route from the data directory.
fn load_golden_route(name: &str) -> GoldenRoute {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden_routes/data")
        .join(format!("{name}.json"));
    let content = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "failed to read golden route file at {}: {}",
            path.display(),
            e
        )
    });
    serde_json::from_str(&content).expect("failed to parse golden route JSON")
}

/// Convert POI specs to domain POIs.
fn build_pois(specs: &[PoiSpec]) -> Vec<PointOfInterest> {
    specs
        .iter()
        .map(|s| {
            let tags: Tags = s.tags.clone().into_iter().collect();
            PointOfInterest::new(s.id, Coord { x: s.x, y: s.y }, tags)
        })
        .collect()
}

/// Convert request spec to domain request.
fn build_request(spec: &RequestSpec) -> SolveRequest {
    let mut interests = InterestProfile::new();
    for (theme_str, weight) in &spec.interests {
        let theme: Theme = theme_str
            .parse()
            .expect("golden route contains invalid theme");
        interests.set_weight(theme, *weight);
    }
    SolveRequest {
        start: Coord {
            x: spec.start.x,
            y: spec.start.y,
        },
        end: spec.end.as_ref().map(|e| Coord { x: e.x, y: e.y }),
        duration_minutes: spec.duration_minutes,
        interests,
        seed: spec.seed,
        max_nodes: spec.max_nodes,
    }
}

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
