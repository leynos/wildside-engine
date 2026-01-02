//! Unit tests for benchmark setup functions.
//!
//! Tests the `build_benchmark_request` and `create_depot` functions from
//! the benchmark support module.

use geo::Coord;
use rstest::rstest;
use wildside_core::Theme;

/// Include the benchmark support module from the benches directory.
/// The `dead_code` suppression is narrowly scoped to this module declaration
/// because bench_support exports items used by other test files but not all
/// items are used in this specific file.
#[expect(dead_code, reason = "bench_support exports items not all used here")]
#[path = "../benches/bench_support.rs"]
mod bench_support;

use bench_support::{BENCHMARK_START, DURATION_MINUTES, build_benchmark_request, create_depot};

// =============================================================================
// Unit tests for build_benchmark_request
// =============================================================================

#[rstest]
fn build_benchmark_request_uses_correct_duration() {
    let request = build_benchmark_request(42);
    assert_eq!(request.duration_minutes, DURATION_MINUTES);
}

#[rstest]
fn build_benchmark_request_uses_seed() {
    let request1 = build_benchmark_request(42);
    let request2 = build_benchmark_request(100);
    assert_eq!(request1.seed, 42);
    assert_eq!(request2.seed, 100);
}

#[rstest]
#[expect(clippy::float_cmp, reason = "Test compares exact literal values")]
fn build_benchmark_request_has_correct_start_position() {
    let request = build_benchmark_request(42);
    assert_eq!(request.start.x, BENCHMARK_START.x);
    assert_eq!(request.start.y, BENCHMARK_START.y);
}

#[rstest]
fn build_benchmark_request_has_no_end_position() {
    let request = build_benchmark_request(42);
    assert!(request.end.is_none());
}

#[rstest]
fn build_benchmark_request_has_no_max_nodes() {
    let request = build_benchmark_request(42);
    assert!(request.max_nodes.is_none());
}

#[rstest]
#[case(Theme::Art)]
#[case(Theme::History)]
#[case(Theme::Nature)]
#[case(Theme::Culture)]
fn build_benchmark_request_includes_theme_interest(#[case] theme: Theme) {
    let request = build_benchmark_request(42);
    let weight = request.interests.weight(&theme);
    assert!(
        weight.is_some_and(|w| w > 0.0),
        "{theme:?} interest should have positive weight"
    );
}

// =============================================================================
// Unit tests for create_depot
// =============================================================================

#[rstest]
#[expect(clippy::float_cmp, reason = "Test compares exact literal values")]
fn create_depot_uses_correct_location() {
    let start = Coord { x: 0.05, y: 0.05 };
    let depot = create_depot(start);
    assert_eq!(depot.location.x, 0.05);
    assert_eq!(depot.location.y, 0.05);
}

#[rstest]
fn create_depot_uses_id_zero() {
    let start = Coord { x: 0.0, y: 0.0 };
    let depot = create_depot(start);
    assert_eq!(depot.id, 0);
}

#[rstest]
fn create_depot_has_empty_tags() {
    let start = Coord { x: 0.0, y: 0.0 };
    let depot = create_depot(start);
    assert!(depot.tags.is_empty());
}

#[rstest]
#[case(0.0, 0.0)]
#[case(1.0, 2.0)]
#[case(-0.5, 0.5)]
fn create_depot_handles_various_coordinates(#[case] x: f64, #[case] y: f64) {
    let start = Coord { x, y };
    let depot = create_depot(start);
    #[expect(clippy::float_cmp, reason = "Test compares exact literal values")]
    {
        assert_eq!(depot.location.x, x);
        assert_eq!(depot.location.y, y);
    }
}
