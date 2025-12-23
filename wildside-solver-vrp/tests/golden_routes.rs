//! Golden routes regression tests for the VRP solver.
//!
//! Each test loads a problem instance from JSON, constructs the solver with a
//! fixed travel-time matrix, and verifies the solution matches expected values
//! within defined tolerances.
//!
//! These tests guard against regressions in the solver's behaviour by asserting
//! that well-defined, small problem instances produce consistent results.

mod golden_routes_support;

use std::collections::HashSet;
use std::fs;
use std::time::Duration;

use rstest::rstest;
use wildside_core::Solver;
use wildside_core::test_support::{MemoryStore, TagScorer};
use wildside_solver_vrp::VrpSolver;
use wildside_solver_vrp::test_support::FixedMatrixTravelTimeProvider;

use golden_routes_support::{build_pois, build_request, load_golden_route};

/// Returns the list of golden route fixture names (without .json extension).
fn list_golden_route_fixtures() -> Vec<String> {
    let data_dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden_routes/data");
    fs::read_dir(&data_dir)
        .unwrap_or_else(|err| panic!("failed to read golden routes data dir: {err}"))
        .filter_map(|result| {
            let dir_entry = result.ok()?;
            let path = dir_entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                path.file_stem().and_then(|s| s.to_str()).map(String::from)
            } else {
                None
            }
        })
        .collect()
}

#[rstest]
#[case("trivial_single_poi")]
#[case("linear_three_poi")]
#[case("budget_constrained")]
#[case("point_to_point")]
#[case("max_nodes_pruning")]
#[case("empty_candidates")]
fn golden_route_regression(#[case] name: &str) {
    let golden = load_golden_route(name);
    let pois = build_pois(&golden.pois);
    let request = build_request(&golden.request);

    // Build solver with fixed travel-time matrix.
    let store = MemoryStore::with_pois(pois);
    let provider = FixedMatrixTravelTimeProvider::from_seconds(golden.travel_time_matrix_seconds);
    let solver = VrpSolver::new(store, provider, TagScorer);

    let response = solver
        .solve(&request)
        .unwrap_or_else(|e| panic!("golden route should solve successfully: {e:?}"));

    // Verify route contains expected POIs (order may vary for equivalent-cost solutions).
    let actual_ids: HashSet<u64> = response.route.pois().iter().map(|p| p.id).collect();
    let expected_ids: HashSet<u64> = golden.expected.route_poi_ids.iter().copied().collect();
    assert_eq!(
        actual_ids, expected_ids,
        "{}: route POI set mismatch (actual: {actual_ids:?}, expected: {expected_ids:?})",
        golden.name
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
            "{}: route duration {:?} exceeds budget {budget:?}",
            golden.name,
            response.route.total_duration()
        );
    }
}

/// Ensure all JSON fixtures in the data directory are covered by test cases.
#[rstest]
fn all_fixtures_are_tested() {
    let expected_fixtures: HashSet<&str> = [
        "trivial_single_poi",
        "linear_three_poi",
        "budget_constrained",
        "point_to_point",
        "max_nodes_pruning",
        "empty_candidates",
    ]
    .into_iter()
    .collect();

    let actual_fixtures: HashSet<String> = list_golden_route_fixtures().into_iter().collect();

    let missing: Vec<_> = actual_fixtures
        .iter()
        .filter(|f| !expected_fixtures.contains(f.as_str()))
        .collect();

    assert!(
        missing.is_empty(),
        "golden route fixtures exist but are not tested: {missing:?}. Add them to the #[case] list."
    );
}
