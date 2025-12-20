#![expect(
    clippy::expect_used,
    reason = "regression tests use expect for readable failures"
)]

//! Golden routes regression tests for the VRP solver.
//!
//! Each test loads a problem instance from JSON, constructs the solver with a
//! fixed travel-time matrix, and verifies the solution matches expected values
//! within defined tolerances.
//!
//! These tests guard against regressions in the solver's behaviour by asserting
//! that well-defined, small problem instances produce consistent results.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use geo::Coord;
use rstest::rstest;
use serde::Deserialize;
use wildside_core::test_support::{MemoryStore, TagScorer};
use wildside_core::{InterestProfile, PointOfInterest, Solver, Tags, Theme};
use wildside_solver_vrp::VrpSolver;
use wildside_solver_vrp::test_support::FixedMatrixTravelTimeProvider;

/// Deserialised golden route test case.
#[derive(Debug, Deserialize)]
struct GoldenRoute {
    name: String,
    #[expect(dead_code, reason = "kept for documentation in JSON files")]
    description: String,
    pois: Vec<PoiSpec>,
    travel_time_matrix_seconds: Vec<Vec<u64>>,
    request: RequestSpec,
    expected: ExpectedResult,
}

/// POI specification from JSON.
#[derive(Debug, Deserialize)]
struct PoiSpec {
    id: u64,
    x: f64,
    y: f64,
    tags: HashMap<String, String>,
}

/// Request specification from JSON.
#[derive(Debug, Deserialize)]
struct RequestSpec {
    start: CoordSpec,
    end: Option<CoordSpec>,
    duration_minutes: u16,
    interests: HashMap<String, f32>,
    seed: u64,
    max_nodes: Option<u16>,
}

/// Coordinate specification from JSON.
#[derive(Debug, Deserialize)]
struct CoordSpec {
    x: f64,
    y: f64,
}

/// Expected result from JSON.
#[derive(Debug, Deserialize)]
struct ExpectedResult {
    route_poi_ids: Vec<u64>,
    min_score: f32,
    max_score: f32,
    respects_budget: bool,
}

/// Load a golden route from the data directory.
fn load_golden_route(filename: &str) -> GoldenRoute {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden_routes/data")
        .join(filename);
    let content = fs::read_to_string(&path).expect("failed to read golden route file");
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
fn build_request(spec: &RequestSpec) -> wildside_core::SolveRequest {
    let mut interests = InterestProfile::new();
    for (theme_str, weight) in &spec.interests {
        let theme: Theme = theme_str
            .parse()
            .expect("golden route contains invalid theme");
        interests.set_weight(theme, *weight);
    }
    wildside_core::SolveRequest {
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

#[rstest]
#[case("trivial_single_poi.json")]
#[case("linear_three_poi.json")]
#[case("budget_constrained.json")]
#[case("point_to_point.json")]
#[case("max_nodes_pruning.json")]
#[case("empty_candidates.json")]
fn golden_route_regression(#[case] filename: &str) {
    let golden = load_golden_route(filename);
    let pois = build_pois(&golden.pois);
    let request = build_request(&golden.request);

    // Build solver with fixed travel-time matrix.
    let store = MemoryStore::with_pois(pois);
    let provider = FixedMatrixTravelTimeProvider::from_seconds(golden.travel_time_matrix_seconds);
    let solver = VrpSolver::new(store, provider, TagScorer);

    let response = solver
        .solve(&request)
        .expect("golden route should solve successfully");

    // Verify route contains expected POIs (order may vary for equivalent-cost solutions).
    let actual_ids: HashSet<u64> = response.route.pois().iter().map(|p| p.id).collect();
    let expected_ids: HashSet<u64> = golden.expected.route_poi_ids.iter().copied().collect();
    assert_eq!(
        actual_ids, expected_ids,
        "{}: route POI set mismatch (actual: {:?}, expected: {:?})",
        golden.name, actual_ids, expected_ids
    );

    // Verify score within expected range.
    assert!(
        response.score >= golden.expected.min_score && response.score <= golden.expected.max_score,
        "{}: score {} outside expected range [{}, {}]",
        golden.name,
        response.score,
        golden.expected.min_score,
        golden.expected.max_score
    );

    // Verify budget is respected when expected.
    if golden.expected.respects_budget {
        let budget = Duration::from_secs(u64::from(request.duration_minutes) * 60);
        assert!(
            response.route.total_duration() <= budget,
            "{}: route duration {:?} exceeds budget {:?}",
            golden.name,
            response.route.total_duration(),
            budget
        );
    }
}
