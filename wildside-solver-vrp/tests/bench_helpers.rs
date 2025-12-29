//! Unit and behavioural tests for benchmark helper functions.
//!
//! These tests verify the `generate_clustered_pois` and `generate_travel_time_matrix`
//! functions produce correct, deterministic outputs.

use std::time::Duration;

use geo::Coord;
use rstest::rstest;
use wildside_core::PointOfInterest;

/// Include the benchmark support module from the benches directory.
/// Note: This path works because Cargo compiles tests with the package root as base.
/// The `dead_code` suppression is narrowly scoped to this module declaration
/// because bench_support exports items used by other test files but not all
/// items are used in this specific file.
#[expect(dead_code, reason = "bench_support exports items not all used here")]
#[path = "../benches/bench_support.rs"]
mod bench_support;

use bench_support::{BENCHMARK_SEED, generate_clustered_pois, generate_travel_time_matrix};

// =============================================================================
// Unit tests for generate_clustered_pois
// =============================================================================

#[rstest]
fn generate_clustered_pois_returns_correct_count() {
    let pois = generate_clustered_pois(50, BENCHMARK_SEED);
    assert_eq!(pois.len(), 50);
}

#[rstest]
#[case(0)]
#[case(1)]
#[case(10)]
#[case(100)]
fn generate_clustered_pois_handles_various_sizes(#[case] count: usize) {
    let pois = generate_clustered_pois(count, BENCHMARK_SEED);
    assert_eq!(pois.len(), count);
}

#[rstest]
#[expect(
    clippy::float_cmp,
    reason = "Determinism test requires exact float comparison"
)]
fn generate_clustered_pois_is_deterministic() {
    let pois1 = generate_clustered_pois(20, BENCHMARK_SEED);
    let pois2 = generate_clustered_pois(20, BENCHMARK_SEED);

    for (poi1, poi2) in pois1.iter().zip(pois2.iter()) {
        assert_eq!(poi1.id, poi2.id);
        assert_eq!(poi1.location.x, poi2.location.x);
        assert_eq!(poi1.location.y, poi2.location.y);
    }
}

#[rstest]
fn generate_clustered_pois_assigns_sequential_ids() {
    let pois = generate_clustered_pois(10, BENCHMARK_SEED);
    for (i, poi) in pois.iter().enumerate() {
        #[expect(
            clippy::as_conversions,
            reason = "Safe conversion for small test indices"
        )]
        let expected_id = (i + 1) as u64;
        assert_eq!(poi.id, expected_id);
    }
}

#[rstest]
#[expect(clippy::indexing_slicing, reason = "Test uses known fixed indices")]
fn generate_clustered_pois_assigns_themes_cyclically() {
    let pois = generate_clustered_pois(8, BENCHMARK_SEED);
    let themes: Vec<_> = pois.iter().map(|p| p.tags.iter().next()).collect();

    // First 4 should be unique (history, art, nature, culture in some order)
    // Next 4 should repeat the same pattern
    assert_eq!(themes[0], themes[4]);
    assert_eq!(themes[1], themes[5]);
    assert_eq!(themes[2], themes[6]);
    assert_eq!(themes[3], themes[7]);
}

#[rstest]
#[expect(
    clippy::float_cmp,
    reason = "Test checks float inequality for different seeds"
)]
fn generate_clustered_pois_different_seeds_produce_different_results() {
    let pois1 = generate_clustered_pois(10, 42);
    let pois2 = generate_clustered_pois(10, 43);

    // At least one POI should have different coordinates
    let any_different = pois1
        .iter()
        .zip(pois2.iter())
        .any(|(p1, p2)| p1.location.x != p2.location.x || p1.location.y != p2.location.y);

    assert!(
        any_different,
        "Different seeds should produce different POI distributions"
    );
}

// =============================================================================
// Unit tests for generate_travel_time_matrix
// =============================================================================

#[rstest]
fn generate_travel_time_matrix_returns_square_matrix() {
    let pois = generate_clustered_pois(5, BENCHMARK_SEED);
    let matrix = generate_travel_time_matrix(&pois, BENCHMARK_SEED);

    assert_eq!(matrix.len(), 5);
    for row in &matrix {
        assert_eq!(row.len(), 5);
    }
}

#[rstest]
#[expect(
    clippy::indexing_slicing,
    clippy::needless_range_loop,
    reason = "Test uses loop index for matrix diagonal access"
)]
fn generate_travel_time_matrix_diagonal_is_zero() {
    let pois = generate_clustered_pois(10, BENCHMARK_SEED);
    let matrix = generate_travel_time_matrix(&pois, BENCHMARK_SEED);

    for i in 0..pois.len() {
        assert_eq!(
            matrix[i][i],
            Duration::ZERO,
            "Diagonal element [{i}][{i}] should be zero"
        );
    }
}

#[rstest]
#[expect(
    clippy::indexing_slicing,
    clippy::needless_range_loop,
    reason = "Test uses loop indices for matrix access"
)]
fn generate_travel_time_matrix_has_positive_off_diagonal() {
    let pois = generate_clustered_pois(5, BENCHMARK_SEED);
    let matrix = generate_travel_time_matrix(&pois, BENCHMARK_SEED);

    for i in 0..pois.len() {
        for j in 0..pois.len() {
            if i != j {
                assert!(
                    matrix[i][j] > Duration::ZERO,
                    "Off-diagonal element [{i}][{j}] should be positive"
                );
            }
        }
    }
}

#[rstest]
#[expect(
    clippy::indexing_slicing,
    reason = "Test uses loop indices for matrix comparison"
)]
fn generate_travel_time_matrix_is_deterministic() {
    let pois = generate_clustered_pois(5, BENCHMARK_SEED);
    let matrix1 = generate_travel_time_matrix(&pois, BENCHMARK_SEED);
    let matrix2 = generate_travel_time_matrix(&pois, BENCHMARK_SEED);

    for i in 0..pois.len() {
        for j in 0..pois.len() {
            assert_eq!(
                matrix1[i][j], matrix2[i][j],
                "Matrix should be deterministic at [{i}][{j}]"
            );
        }
    }
}

#[rstest]
fn generate_travel_time_matrix_handles_empty_input() {
    let pois: Vec<PointOfInterest> = vec![];
    let matrix = generate_travel_time_matrix(&pois, BENCHMARK_SEED);
    assert!(matrix.is_empty());
}

#[rstest]
#[expect(clippy::indexing_slicing, reason = "Test uses known fixed index [0]")]
fn generate_travel_time_matrix_handles_single_poi() {
    let pois = vec![PointOfInterest::with_empty_tags(
        1,
        Coord { x: 0.0, y: 0.0 },
    )];
    let matrix = generate_travel_time_matrix(&pois, BENCHMARK_SEED);

    assert_eq!(matrix.len(), 1);
    assert_eq!(matrix[0].len(), 1);
    assert_eq!(matrix[0][0], Duration::ZERO);
}

#[rstest]
#[expect(clippy::indexing_slicing, reason = "Test uses known fixed indices")]
fn generate_travel_time_matrix_reflects_distance() {
    // Create POIs at known locations with varying distances from origin
    let close = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    let medium = PointOfInterest::with_empty_tags(2, Coord { x: 0.01, y: 0.0 });
    let far = PointOfInterest::with_empty_tags(3, Coord { x: 0.02, y: 0.0 });
    let pois = vec![close, medium, far];

    let matrix = generate_travel_time_matrix(&pois, BENCHMARK_SEED);

    // Travel time from origin to far should be greater than to medium
    // (accounting for noise, we check the general trend holds on average)
    // The noise is ±20%, so 0->far should still be roughly 2x 0->medium
    let time_to_medium = matrix[0][1].as_secs_f64();
    let time_to_far = matrix[0][2].as_secs_f64();

    assert!(
        time_to_far > time_to_medium,
        "Farther POI should have longer travel time (medium: {time_to_medium}s, far: {time_to_far}s)"
    );
}
