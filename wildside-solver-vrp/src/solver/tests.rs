//! Tests for the `VrpSolver`.

use super::*;
use geo::Coord;
use rstest::rstest;
use wildside_core::test_support::{MemoryStore, TagScorer, UnitTravelTimeProvider};
use wildside_core::{InterestProfile, Theme};

use crate::test_support::poi;

#[rstest]
fn candidate_selection_respects_max_nodes() {
    let pois = vec![
        poi(1, 0.0, 0.0, "art"),
        poi(2, 0.001, 0.0, "history"),
        poi(3, 0.002, 0.0, "nature"),
    ];
    let store = MemoryStore::with_pois(pois);
    let solver = VrpSolver::new(store, UnitTravelTimeProvider, TagScorer);
    let interests = InterestProfile::new()
        .with_weight(Theme::Art, 0.9)
        .with_weight(Theme::History, 0.4)
        .with_weight(Theme::Nature, 0.1);

    let request = SolveRequest {
        start: Coord { x: 0.0, y: 0.0 },
        end: None,
        duration_minutes: 10,
        interests,
        seed: 1,
        max_nodes: Some(2),
    };

    let candidates = solver.select_candidates(&request);
    assert_eq!(candidates.len(), 2);
    let first = candidates
        .first()
        .map(|(poi, _)| poi)
        .expect("expected first candidate");
    assert_eq!(first.id, 1);
    let second = candidates
        .get(1)
        .map(|(poi, _)| poi)
        .expect("expected second candidate");
    assert_eq!(second.id, 2);
}

#[rstest]
fn solve_returns_route_with_positive_score() {
    let pois = vec![poi(1, 0.0, 0.0, "art"), poi(2, 0.001, 0.0, "history")];
    let store = MemoryStore::with_pois(pois);
    let solver = VrpSolver::new(store, UnitTravelTimeProvider, TagScorer);
    let interests = InterestProfile::new()
        .with_weight(Theme::Art, 0.8)
        .with_weight(Theme::History, 0.5);
    let request = SolveRequest {
        start: Coord { x: 0.0, y: 0.0 },
        end: None,
        duration_minutes: 10,
        interests,
        seed: 1,
        max_nodes: None,
    };

    let response = solver.solve(&request).expect("solve should succeed");
    assert!(!response.route.pois().is_empty());
    assert!(response.score > 0.0);
    assert!(response.route.total_duration() <= Duration::from_secs(600));
}

#[rstest]
fn invalid_request_is_rejected() {
    let store = MemoryStore::default();
    let solver = VrpSolver::new(store, UnitTravelTimeProvider, TagScorer);
    let request = SolveRequest {
        start: Coord { x: 0.0, y: 0.0 },
        end: None,
        duration_minutes: 0,
        interests: InterestProfile::new(),
        seed: 1,
        max_nodes: None,
    };

    let err = solver
        .solve(&request)
        .expect_err("expected invalid request error");
    assert!(matches!(err, SolveError::InvalidRequest));
}

#[rstest]
fn route_duration_adds_final_leg_to_end_location() {
    let start = PointOfInterest::with_empty_tags(0, Coord { x: 0.0, y: 0.0 });
    let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    let end = PointOfInterest::with_empty_tags(u64::MAX, Coord { x: 1.0, y: 1.0 });
    let all_pois = vec![start, poi.clone(), end];
    let matrix = vec![
        vec![
            Duration::ZERO,
            Duration::from_secs(5),
            Duration::from_secs(3),
        ],
        vec![
            Duration::from_secs(11),
            Duration::ZERO,
            Duration::from_secs(7),
        ],
        vec![
            Duration::from_secs(13),
            Duration::from_secs(17),
            Duration::ZERO,
        ],
    ];

    let duration = route_duration(&[poi], &all_pois, &matrix, 2);
    assert_eq!(duration, Duration::from_secs(12));
}

#[rstest]
fn route_duration_returns_to_start_when_end_is_depot() {
    let start = PointOfInterest::with_empty_tags(0, Coord { x: 0.0, y: 0.0 });
    let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    let all_pois = vec![start, poi.clone()];
    let matrix = vec![
        vec![Duration::ZERO, Duration::from_secs(5)],
        vec![Duration::from_secs(11), Duration::ZERO],
    ];

    let duration = route_duration(&[poi], &all_pois, &matrix, 0);
    assert_eq!(duration, Duration::from_secs(16));
}
