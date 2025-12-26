//! Property-based tests for the VRP solver.
//!
//! These tests use `proptest` to assert invariants that must hold for all valid
//! solver inputs, complementing the golden route regression tests and BDD
//! behavioural tests.
//!
//! # Invariants tested
//!
//! - **Budget compliance:** Route duration never exceeds the time budget.
//! - **No duplicates:** Each POI ID appears at most once in the route.
//! - **Score validity:** Scores are non-negative, finite, and bounded by POI count.
//! - **Constraint adherence:** `max_nodes` limits are respected (with and without pruning).
//! - **POI validity:** All route POIs exist in the candidate set.
//! - **Point-to-point validity:** Routes with distinct end locations maintain all
//!   core invariants and have correctly set start/end coordinates.
//! - **Empty candidates:** When no candidates match, an empty route with zero score
//!   is returned.

#![expect(
    clippy::cast_precision_loss,
    reason = "POI counts are small in tests; precision loss is negligible"
)]

mod proptest_support;

use std::collections::HashSet;
use std::time::Duration;

use geo::Coord;
use proptest::prelude::*;
use wildside_core::test_support::{MemoryStore, TagScorer, UnitTravelTimeProvider};
use wildside_core::{InterestProfile, Scorer, Solver, Theme};
use wildside_solver_vrp::VrpSolver;

use proptest_support::{
    assert_no_duplicate_poi_ids, euclidean_distance, generate_pois_near_origin, poi_set_strategy,
};

/// Build a standard solve request for property tests.
fn build_request(
    duration_minutes: u16,
    seed: u64,
    max_nodes: Option<u16>,
    end: Option<Coord<f64>>,
) -> wildside_core::SolveRequest {
    let interests = InterestProfile::new()
        .with_weight(Theme::Art, 0.8)
        .with_weight(Theme::History, 0.5)
        .with_weight(Theme::Nature, 0.3)
        .with_weight(Theme::Culture, 0.2);
    wildside_core::SolveRequest {
        start: Coord { x: 0.0, y: 0.0 },
        end,
        duration_minutes,
        interests,
        seed,
        max_nodes,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property: Route total duration never exceeds the requested time budget.
    ///
    /// This is a fundamental invariant of the orienteering problem: the solver
    /// must respect the maximum allowed travel time (Tmax).
    ///
    /// Uses `UnitTravelTimeProvider` which generates matrices dynamically based
    /// on the actual number of candidates after filtering.
    #[test]
    fn route_duration_respects_budget(
        seed in any::<u64>(),
        duration_minutes in 5_u16..=60_u16,
    ) {
        let pois = generate_pois_near_origin(5);

        let store = MemoryStore::with_pois(pois);
        let solver = VrpSolver::new(store, UnitTravelTimeProvider, TagScorer);

        let request = build_request(duration_minutes, seed, None, None);
        let response = solver.solve(&request).expect("solve should succeed");

        let budget = Duration::from_secs(u64::from(duration_minutes) * 60);
        prop_assert!(
            response.route.total_duration() <= budget,
            "Route duration {:?} exceeds budget {:?}",
            response.route.total_duration(),
            budget
        );
    }

    /// Property: No POI appears more than once in the route.
    ///
    /// The orienteering problem visits each location at most once. Duplicate
    /// visits would violate the problem constraints and inflate scores.
    ///
    /// Uses a variable-size POI set (3-15 POIs) with randomised locations to
    /// exercise more configurations.
    #[test]
    fn route_has_no_duplicate_pois(
        seed in any::<u64>(),
        pois in poi_set_strategy(3, 15),
    ) {
        let store = MemoryStore::with_pois(pois);
        let solver = VrpSolver::new(store, UnitTravelTimeProvider, TagScorer);

        let request = build_request(60, seed, None, None);
        let response = solver.solve(&request).expect("solve should succeed");

        assert_no_duplicate_poi_ids(response.route.pois())?;
    }

    /// Property: Score always respects contract bounds and is finite.
    ///
    /// The scoring contract requires all POI scores to be in `[0.0, 1.0]`. The
    /// total route score is a sum of individual POI scores, so it must be
    /// non-negative and must not exceed the number of POIs in the route.
    /// Additionally, scores must never be NaN or infinite.
    #[test]
    fn score_respects_bounds_and_is_finite(seed in any::<u64>()) {
        let pois = generate_pois_near_origin(3);

        let store = MemoryStore::with_pois(pois);
        let solver = VrpSolver::new(store, UnitTravelTimeProvider, TagScorer);

        let request = build_request(30, seed, None, None);
        let response = solver.solve(&request).expect("solve should succeed");

        let poi_count = response.route.pois().len();

        prop_assert!(
            response.score >= 0.0,
            "Score {} is negative",
            response.score
        );
        // Since each POI score is in [0.0, 1.0], the total score cannot
        // exceed the number of POIs. We compare using integer bounds to
        // avoid floating-point arithmetic lint issues.
        prop_assert!(
            response.score <= poi_count as f32,
            "Score {} exceeds upper bound {} for {} POIs",
            response.score,
            poi_count,
            poi_count
        );
        prop_assert!(
            response.score.is_finite(),
            "Score {} is not finite",
            response.score
        );

        // Verify that the scorer produces valid per-POI scores within [0.0, 1.0].
        // This confirms the Scorer contract is upheld for each POI independently.
        let scorer = TagScorer;
        for poi in response.route.pois() {
            let poi_score = scorer.score(poi, &request.interests);
            prop_assert!(
                (0.0..=1.0).contains(&poi_score),
                "POI {} has score {} outside [0.0, 1.0]",
                poi.id,
                poi_score
            );
        }
    }

    /// Property: When `max_nodes` is set, the route never contains more POIs.
    ///
    /// The `max_nodes` parameter is a pruning hint that limits the number of
    /// candidate POIs considered by the solver. The resulting route should
    /// respect this constraint.
    ///
    /// Uses `UnitTravelTimeProvider`, which generates matrices dynamically based
    /// on the actual number of candidates after filtering.
    #[test]
    fn max_nodes_constraint_is_respected(
        seed in any::<u64>(),
        max_nodes in 1_u16..=5_u16,
    ) {
        // Create more POIs than max_nodes to test pruning.
        let pois = generate_pois_near_origin(10);

        let store = MemoryStore::with_pois(pois);
        let solver = VrpSolver::new(store, UnitTravelTimeProvider, TagScorer);

        let request = build_request(120, seed, Some(max_nodes), None);
        let response = solver.solve(&request).expect("solve should succeed");

        prop_assert!(
            response.route.pois().len() <= usize::from(max_nodes),
            "Route has {} POIs but max_nodes is {}",
            response.route.pois().len(),
            max_nodes
        );
    }

    /// Property: When `max_nodes` >= candidate count, no pruning occurs but
    /// constraint is still respected.
    ///
    /// This covers the boundary where `max_nodes` is at least the number of
    /// candidate POIs, so pruning should not reduce the candidate set. The
    /// resulting route must still not contain more POIs than allowed.
    ///
    /// Uses `UnitTravelTimeProvider`, which generates matrices dynamically based
    /// on the actual number of candidates after filtering.
    #[test]
    fn max_nodes_constraint_respected_without_pruning(
        seed in any::<u64>(),
        // Ensure `max_nodes` is always >= candidate_count (10) to exercise
        // the no-pruning path.
        max_nodes in 10_u16..=20_u16,
    ) {
        // Same candidate count as the pruning test, but now `max_nodes` >=
        // candidate_count.
        let pois = generate_pois_near_origin(10);

        let store = MemoryStore::with_pois(pois);
        let solver = VrpSolver::new(store, UnitTravelTimeProvider, TagScorer);

        let request = build_request(120, seed, Some(max_nodes), None);
        let response = solver.solve(&request).expect("solve should succeed");

        // Even in the no-pruning case, the route must never violate
        // `max_nodes`.
        prop_assert!(
            response.route.pois().len() <= usize::from(max_nodes),
            "Route has {} POIs but max_nodes is {} in no-pruning scenario",
            response.route.pois().len(),
            max_nodes
        );
    }

    /// Property: All POIs in the route exist in the original candidate set.
    ///
    /// The solver should only return POIs that were present in the store and
    /// matched the query criteria. No spurious POI IDs should appear.
    ///
    /// Uses a variable-size POI set (2-12 POIs) with randomised locations to
    /// exercise more configurations.
    #[test]
    fn route_pois_exist_in_candidates(
        seed in any::<u64>(),
        pois in poi_set_strategy(2, 12),
    ) {
        let candidate_ids: HashSet<u64> = pois.iter().map(|p| p.id).collect();

        let store = MemoryStore::with_pois(pois);
        let solver = VrpSolver::new(store, UnitTravelTimeProvider, TagScorer);

        let request = build_request(60, seed, None, None);
        let response = solver.solve(&request).expect("solve should succeed");

        for poi in response.route.pois() {
            prop_assert!(
                candidate_ids.contains(&poi.id),
                "Route contains POI {} which is not in the candidate set {:?}",
                poi.id,
                candidate_ids
            );
        }
    }

    /// Property: Point-to-point routes return valid responses.
    ///
    /// When an end coordinate is specified, the solver should still produce
    /// valid responses with the same invariants as round-trip routes.
    /// Additionally, the route's start and end coordinates must match the
    /// request's start and end values.
    ///
    /// Uses a variable-size POI set (3-10 POIs) with randomised locations to
    /// exercise more configurations.
    #[test]
    fn point_to_point_routes_are_valid(
        seed in any::<u64>(),
        pois in poi_set_strategy(3, 10),
    ) {
        let store = MemoryStore::with_pois(pois);
        let solver = VrpSolver::new(store, UnitTravelTimeProvider, TagScorer);

        let start = Coord { x: 0.0, y: 0.0 };
        let end = Coord { x: 0.01, y: 0.01 };
        let request = build_request(30, seed, None, Some(end));
        let response = solver.solve(&request).expect("solve should succeed");

        // Verify core invariants hold for point-to-point routes.
        let budget = Duration::from_secs(30 * 60);
        prop_assert!(
            response.route.total_duration() <= budget,
            "Point-to-point route duration {:?} exceeds budget {:?}",
            response.route.total_duration(),
            budget
        );
        prop_assert!(response.score >= 0.0, "Score is negative");
        prop_assert!(response.score.is_finite(), "Score is not finite");
        assert_no_duplicate_poi_ids(response.route.pois())?;

        // Verify that the route's start and end coordinates match the request.
        // Using a small tolerance for floating-point comparison.
        let tolerance = 0.0001;
        let route_start = response.route.start();
        let route_end = response.route.end();

        let start_distance = euclidean_distance(&route_start, &start);
        prop_assert!(
            start_distance <= tolerance,
            "Route start {:?} is too far from requested start {:?} (distance: {:.6})",
            route_start,
            start,
            start_distance
        );

        let end_distance = euclidean_distance(&route_end, &end);
        prop_assert!(
            end_distance <= tolerance,
            "Route end {:?} is too far from requested end {:?} (distance: {:.6})",
            route_end,
            end,
            end_distance
        );

        // Verify all POIs are within reasonable distance of both start and end,
        // confirming they're in the candidate search area and reachable from
        // both endpoints.
        //
        // POIs are distributed ±0.01 around origin, start is at origin, end is
        // at (0.01, 0.01). Maximum distance from start is 0.02 (diagonal).
        // Maximum distance from end is ~0.03 (POI at (-0.01, -0.01) to end at
        // (0.01, 0.01)).
        let max_distance_from_start = 0.02;
        let max_distance_from_end = 0.03;
        for poi in response.route.pois() {
            let dist_from_start = euclidean_distance(&poi.location, &start);
            prop_assert!(
                dist_from_start <= max_distance_from_start,
                "POI {} at {:?} is too far from start (distance: {:.6})",
                poi.id,
                poi.location,
                dist_from_start
            );

            let dist_from_end = euclidean_distance(&poi.location, &end);
            prop_assert!(
                dist_from_end <= max_distance_from_end,
                "POI {} at {:?} is too far from end (distance: {:.6})",
                poi.id,
                poi.location,
                dist_from_end
            );
        }
    }

    /// Property: Empty candidate sets produce empty routes with zero score.
    ///
    /// When no POIs match the query, the solver should return an empty route
    /// rather than failing. Uses an empty store to guarantee no candidates,
    /// making this test independent of the solver's search radius heuristic.
    #[test]
    fn empty_candidates_produce_empty_route(seed in any::<u64>()) {
        // Use an empty store to guarantee no candidates are found.
        // This approach is independent of the solver's search radius heuristic.
        let store = MemoryStore::default();
        let solver = VrpSolver::new(store, UnitTravelTimeProvider, TagScorer);

        let request = build_request(10, seed, None, None);
        let response = solver.solve(&request).expect("solve should succeed");

        prop_assert!(
            response.route.pois().is_empty(),
            "Expected empty route but got {} POIs",
            response.route.pois().len()
        );
        prop_assert!(
            response.score.abs() < f32::EPSILON,
            "Expected zero score but got {}",
            response.score
        );
    }
}
