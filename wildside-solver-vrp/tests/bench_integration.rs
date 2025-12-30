//! Integration test validating benchmark helpers work with `VrpSolver`.
//!
//! This test ensures the full benchmark data pipeline (POI generation,
//! travel time matrix, solver invocation) works correctly end-to-end.

use rstest::rstest;
use wildside_core::test_support::{MemoryStore, TagScorer};
use wildside_core::{SolveRequest, Solver};
use wildside_solver_vrp::VrpSolver;
use wildside_solver_vrp::test_support::FixedMatrixTravelTimeProvider;

/// Include the benchmark support module from the benches directory.
#[path = "../benches/bench_support.rs"]
mod bench_support;

use bench_support::{
    BENCHMARK_SEED, build_benchmark_request, create_depot, generate_clustered_pois,
    generate_travel_time_matrix,
};

/// Build a solver and request from the given parameters.
///
/// This helper encapsulates the common setup pattern used across integration tests:
/// 1. Generate candidate POIs
/// 2. Build the benchmark request
/// 3. Create depot and combine with candidates
/// 4. Generate travel time matrix
/// 5. Create provider, store, and solver
fn build_solver_and_request(
    size: usize,
    seed: u64,
) -> (
    VrpSolver<MemoryStore, FixedMatrixTravelTimeProvider, TagScorer>,
    SolveRequest,
) {
    let candidate_pois = generate_clustered_pois(size, seed);
    let request = build_benchmark_request(seed);

    let depot = create_depot(request.start);
    let mut all_pois = vec![depot];
    all_pois.extend(candidate_pois.iter().cloned());

    let matrix_durations = generate_travel_time_matrix(&all_pois, seed);
    let provider = FixedMatrixTravelTimeProvider::new(matrix_durations);
    let store = MemoryStore::with_pois(candidate_pois);
    let solver = VrpSolver::new(store, provider, TagScorer);

    (solver, request)
}

/// Validates that benchmark data generation produces solver-compatible inputs.
///
/// This integration test exercises the full pipeline:
/// 1. Generate clustered POIs using `generate_clustered_pois`
/// 2. Build a travel time matrix using `generate_travel_time_matrix`
/// 3. Construct a `FixedMatrixTravelTimeProvider` from the matrix
/// 4. Instantiate `VrpSolver` with the generated data
/// 5. Call `solve()` and verify it returns a valid result
#[rstest]
fn benchmark_helpers_produce_solver_compatible_data() {
    let (solver, request) = build_solver_and_request(50, BENCHMARK_SEED);

    // Solve and verify we get a valid result (Ok variant)
    let response = solver
        .solve(&request)
        .expect("Solver should succeed with benchmark-generated data");

    // Verify the response contains expected fields
    assert!(
        response.diagnostics.solve_time.as_nanos() > 0,
        "Solve time should be recorded"
    );
}

/// Validates that the solver handles various problem sizes from benchmark helpers.
#[rstest]
#[case(10)]
#[case(50)]
#[case(100)]
fn benchmark_helpers_work_with_various_problem_sizes(#[case] size: usize) {
    let (solver, request) = build_solver_and_request(size, BENCHMARK_SEED);

    let result = solver.solve(&request);
    assert!(
        result.is_ok(),
        "Solver should succeed with {size} POIs: {result:?}"
    );
}

/// Validates that different seeds produce different but valid solver inputs.
#[rstest]
#[case(42)]
#[case(100)]
#[case(999)]
fn different_seeds_produce_valid_solver_inputs(#[case] seed: u64) {
    let (solver, request) = build_solver_and_request(20, seed);

    let result = solver.solve(&request);
    assert!(
        result.is_ok(),
        "Solver should succeed with seed {seed}: {result:?}"
    );
}
