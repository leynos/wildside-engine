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

// Criterion macros generate code that triggers missing_docs warnings.
#![allow(missing_docs, reason = "Criterion macros generate undocumented code")]

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use geo::Coord;
use wildside_core::test_support::{MemoryStore, TagScorer};
use wildside_core::{InterestProfile, PointOfInterest, Solver, Theme};
use wildside_solver_vrp::VrpSolver;
use wildside_solver_vrp::test_support::FixedMatrixTravelTimeProvider;

mod bench_support;

use bench_support::{BENCHMARK_SEED, generate_clustered_pois, generate_travel_time_matrix};

/// Problem sizes to benchmark: 50, 100, 200 candidate POIs.
const PROBLEM_SIZES: &[usize] = &[50, 100, 200];

/// Time budget for benchmark solve requests (minutes).
const DURATION_MINUTES: u16 = 60;

/// Build a standard benchmark solve request.
///
/// Uses a consistent interest profile and deterministic seed for reproducibility.
fn build_benchmark_request(seed: u64) -> wildside_core::SolveRequest {
    wildside_core::SolveRequest {
        start: Coord { x: 0.05, y: 0.05 }, // Centre of the POI area
        end: None,
        duration_minutes: DURATION_MINUTES,
        interests: InterestProfile::new()
            .with_weight(Theme::Art, 0.8)
            .with_weight(Theme::History, 0.5)
            .with_weight(Theme::Nature, 0.3)
            .with_weight(Theme::Culture, 0.2),
        seed,
        max_nodes: None,
    }
}

/// Create a depot POI at the start location for the travel time matrix.
fn create_depot(start: Coord<f64>) -> PointOfInterest {
    PointOfInterest::with_empty_tags(0, start)
}

/// Benchmark solve times for various problem sizes.
///
/// For each problem size (50, 100, 200 candidates), this benchmark:
/// 1. Generates a deterministic set of clustered POIs
/// 2. Computes a distance-based travel time matrix
/// 3. Measures the time to solve the orienteering problem
///
/// The benchmark uses 100 samples and 10-second measurement windows for
/// reliable P95/P99 estimation.
fn bench_solve_times(c: &mut Criterion) {
    let mut group = c.benchmark_group("solve_time");

    // Configure for reliable percentile estimation.
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));

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

criterion_group!(benches, bench_solve_times);
criterion_main!(benches);
