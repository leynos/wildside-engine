//! Integration test validating benchmark helpers work with `VrpSolver`.
//!
//! This test ensures the full benchmark data pipeline (POI generation,
//! travel time matrix, solver invocation) works correctly end-to-end.

use rstest::rstest;
use wildside_core::Solver;
use wildside_core::test_support::{MemoryStore, TagScorer};
use wildside_solver_vrp::VrpSolver;
use wildside_solver_vrp::test_support::FixedMatrixTravelTimeProvider;

/// Include the benchmark support module from the benches directory.
#[path = "../benches/bench_support.rs"]
mod bench_support;

use bench_support::{
    BENCHMARK_SEED, build_benchmark_request, create_depot, generate_clustered_pois,
    generate_travel_time_matrix,
};

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
    // Generate candidate POIs
    let candidate_pois = generate_clustered_pois(50, BENCHMARK_SEED);

    // Build the benchmark request
    let request = build_benchmark_request(BENCHMARK_SEED);

    // Create depot at start location and combine with candidates
    let depot = create_depot(request.start);
    let mut all_pois = vec![depot];
    all_pois.extend(candidate_pois.iter().cloned());

    // Generate travel time matrix for all POIs (depot + candidates)
    let matrix_durations = generate_travel_time_matrix(&all_pois, BENCHMARK_SEED);

    // Create the travel time provider
    let provider = FixedMatrixTravelTimeProvider::new(matrix_durations);

    // Create memory store with candidate POIs (not depot)
    let store = MemoryStore::with_pois(candidate_pois);

    // Instantiate the solver
    let solver = VrpSolver::new(store, provider, TagScorer);

    // Solve and verify we get a valid result (Ok variant)
    let result = solver.solve(&request);
    assert!(
        result.is_ok(),
        "Solver should succeed with benchmark-generated data: {result:?}"
    );

    // Verify the response contains expected fields
    let response = result.expect("already checked is_ok");
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
    let candidate_pois = generate_clustered_pois(size, BENCHMARK_SEED);
    let request = build_benchmark_request(BENCHMARK_SEED);

    let depot = create_depot(request.start);
    let mut all_pois = vec![depot];
    all_pois.extend(candidate_pois.iter().cloned());

    let matrix_durations = generate_travel_time_matrix(&all_pois, BENCHMARK_SEED);
    let provider = FixedMatrixTravelTimeProvider::new(matrix_durations);
    let store = MemoryStore::with_pois(candidate_pois);
    let solver = VrpSolver::new(store, provider, TagScorer);

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
    let candidate_pois = generate_clustered_pois(20, seed);
    let request = build_benchmark_request(seed);

    let depot = create_depot(request.start);
    let mut all_pois = vec![depot];
    all_pois.extend(candidate_pois.iter().cloned());

    let matrix_durations = generate_travel_time_matrix(&all_pois, seed);
    let provider = FixedMatrixTravelTimeProvider::new(matrix_durations);
    let store = MemoryStore::with_pois(candidate_pois);
    let solver = VrpSolver::new(store, provider, TagScorer);

    let result = solver.solve(&request);
    assert!(
        result.is_ok(),
        "Solver should succeed with seed {seed}: {result:?}"
    );
}
