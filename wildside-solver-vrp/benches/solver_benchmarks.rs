//! Criterion benchmarks for the VRP solver.
//!
//! Measures solve time across problem sizes (50, 100, 200 candidates) to track
//! performance and detect regressions. Results include statistical analysis
//! with percentile distributions.
//!
//! Run benchmarks with:
//! ```bash
//! cargo bench --package wildside-solver-vrp
//! ```

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput};
use wildside_core::Solver;
use wildside_core::test_support::{MemoryStore, TagScorer};
use wildside_solver_vrp::VrpSolver;
use wildside_solver_vrp::test_support::FixedMatrixTravelTimeProvider;

mod bench_support;

use bench_support::{
    BENCHMARK_SEED, build_benchmark_request, create_depot, generate_clustered_pois,
    generate_travel_time_matrix,
};

/// Problem sizes to benchmark: 50, 100, 200 candidate POIs.
const PROBLEM_SIZES: &[usize] = &[50, 100, 200];

/// Benchmark solve times for various problem sizes.
///
/// For each problem size (50, 100, 200 candidates), this benchmark:
/// 1. Generates a deterministic set of clustered POIs
/// 2. Computes a distance-based travel time matrix
/// 3. Measures the time to solve the orienteering problem
///
/// The benchmark uses 10 samples and 3-second measurement windows for
/// CI-friendly runtimes while maintaining statistical validity.
fn bench_solve_times(c: &mut Criterion) {
    let mut group = c.benchmark_group("solve_time");

    // Configure for CI-friendly runtimes while maintaining statistical validity.
    // 10 samples is the criterion minimum; 3 seconds keeps total under 5 minutes.
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    for &size in PROBLEM_SIZES {
        // Pre-generate inputs outside the benchmark loop.
        let candidate_pois = generate_clustered_pois(size, BENCHMARK_SEED);
        let request = build_benchmark_request(BENCHMARK_SEED);

        // Include depot in the POI set for the travel time matrix.
        let depot = create_depot(request.start);
        let mut all_pois = vec![depot];
        all_pois.extend(candidate_pois.iter().cloned());

        let matrix_durations = generate_travel_time_matrix(&all_pois, BENCHMARK_SEED);
        let provider = FixedMatrixTravelTimeProvider::new(matrix_durations);

        // Store contains only the candidate POIs (not the depot).
        let store = MemoryStore::with_pois(candidate_pois);
        let solver = VrpSolver::new(store, provider, TagScorer);

        #[expect(
            clippy::as_conversions,
            reason = "Safe conversion for small problem sizes"
        )]
        let throughput_size = size as u64;
        group.throughput(Throughput::Elements(throughput_size));
        group.bench_with_input(BenchmarkId::new("candidates", size), &size, |b, _| {
            b.iter(|| {
                #[expect(
                    clippy::let_underscore_must_use,
                    reason = "Benchmarking solve performance, result is intentionally discarded"
                )]
                let _ = solver.solve(&request);
            });
        });
    }

    group.finish();
}

// Private module containing the criterion_group! macro invocation.
mod group {
    use super::bench_solve_times;
    use criterion::criterion_group;

    criterion_group!(benches, bench_solve_times);

    // Re-export the generated function for use by criterion_main!
    pub use self::benches as run;
}

// The criterion_main! macro must be invoked at crate level to generate main().
// It receives the benchmark group function from the module above.
use criterion::criterion_main;
criterion_main!(group::run);
