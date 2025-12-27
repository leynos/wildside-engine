//! Unit tests for benchmark setup functions.
//!
//! Tests the `build_benchmark_request` and `create_depot` functions from
//! the benchmark module.

// Allow float comparisons in tests since we're checking exact literal values.
#![expect(clippy::float_cmp, reason = "Tests compare exact literal values")]

use geo::Coord;
use rstest::rstest;
use wildside_core::{InterestProfile, PointOfInterest, SolveRequest, Theme};

/// Build a standard benchmark solve request.
///
/// Mirrors the function in `solver_benchmarks.rs` for testing.
fn build_benchmark_request(seed: u64) -> SolveRequest {
    const DURATION_MINUTES: u16 = 60;

    SolveRequest {
        start: Coord { x: 0.05, y: 0.05 },
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
///
/// Mirrors the function in `solver_benchmarks.rs` for testing.
fn create_depot(start: Coord<f64>) -> PointOfInterest {
    PointOfInterest::with_empty_tags(0, start)
}

// =============================================================================
// Unit tests for build_benchmark_request
// =============================================================================

#[rstest]
fn build_benchmark_request_uses_correct_duration() {
    let request = build_benchmark_request(42);
    assert_eq!(request.duration_minutes, 60);
}

#[rstest]
fn build_benchmark_request_uses_seed() {
    let request1 = build_benchmark_request(42);
    let request2 = build_benchmark_request(100);
    assert_eq!(request1.seed, 42);
    assert_eq!(request2.seed, 100);
}

#[rstest]
fn build_benchmark_request_has_correct_start_position() {
    let request = build_benchmark_request(42);
    assert_eq!(request.start.x, 0.05);
    assert_eq!(request.start.y, 0.05);
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
fn build_benchmark_request_includes_art_interest() {
    let request = build_benchmark_request(42);
    let weight = request.interests.weight(&Theme::Art);
    assert!(
        weight.is_some_and(|w| w > 0.0),
        "Art interest should have positive weight"
    );
}

#[rstest]
fn build_benchmark_request_includes_history_interest() {
    let request = build_benchmark_request(42);
    let weight = request.interests.weight(&Theme::History);
    assert!(
        weight.is_some_and(|w| w > 0.0),
        "History interest should have positive weight"
    );
}

#[rstest]
fn build_benchmark_request_includes_nature_interest() {
    let request = build_benchmark_request(42);
    let weight = request.interests.weight(&Theme::Nature);
    assert!(
        weight.is_some_and(|w| w > 0.0),
        "Nature interest should have positive weight"
    );
}

#[rstest]
fn build_benchmark_request_includes_culture_interest() {
    let request = build_benchmark_request(42);
    let weight = request.interests.weight(&Theme::Culture);
    assert!(
        weight.is_some_and(|w| w > 0.0),
        "Culture interest should have positive weight"
    );
}

// =============================================================================
// Unit tests for create_depot
// =============================================================================

#[rstest]
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
    assert_eq!(depot.location.x, x);
    assert_eq!(depot.location.y, y);
}
