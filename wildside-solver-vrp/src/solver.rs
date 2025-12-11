//! `VrpSolver` implementation backed by `vrp-core`.

use std::time::{Duration, Instant};

use geo::{Coord, Rect};
use wildside_core::{
    Diagnostics, PoiStore, PointOfInterest, Route, Scorer, SolveError, SolveRequest, SolveResponse,
    Solver, TravelTimeProvider,
};

use crate::vrp::VrpSolveContext;

/// Configuration for [`VrpSolver`].
#[derive(Debug, Clone)]
pub struct VrpSolverConfig {
    /// Average walking speed used to derive a candidate search radius.
    pub average_speed_kmh: f64,
    /// Upper bound on `vrp-core` generations.
    pub max_generations: usize,
}

impl Default for VrpSolverConfig {
    fn default() -> Self {
        Self {
            average_speed_kmh: 5.0,
            max_generations: 50,
        }
    }
}

/// Native solver using `vrp-core` to search for high-score routes.
///
/// The solver is generic over the engine boundaries: a read-only POI store,
/// a travel-time provider, and a relevance scorer.
pub struct VrpSolver<S, T, C>
where
    S: PoiStore,
    T: TravelTimeProvider,
    C: Scorer,
{
    store: S,
    travel_time_provider: T,
    scorer: C,
    config: VrpSolverConfig,
}

impl<S, T, C> VrpSolver<S, T, C>
where
    S: PoiStore,
    T: TravelTimeProvider,
    C: Scorer,
{
    /// Construct a solver using default configuration.
    pub fn new(store: S, travel_time_provider: T, scorer: C) -> Self {
        Self::with_config(
            store,
            travel_time_provider,
            scorer,
            VrpSolverConfig::default(),
        )
    }

    /// Construct a solver with explicit configuration.
    pub const fn with_config(
        store: S,
        travel_time_provider: T,
        scorer: C,
        config: VrpSolverConfig,
    ) -> Self {
        Self {
            store,
            travel_time_provider,
            scorer,
            config,
        }
    }
}

impl<S, T, C> Solver for VrpSolver<S, T, C>
where
    S: PoiStore + Send + Sync,
    T: TravelTimeProvider + Send + Sync,
    C: Scorer + Send + Sync,
{
    fn solve(&self, request: &SolveRequest) -> Result<SolveResponse, SolveError> {
        request.validate()?;
        let started_at = Instant::now();

        let candidates = self.select_candidates(request);
        if candidates.is_empty() {
            return Ok(SolveResponse {
                route: Route::empty(),
                score: 0.0,
                diagnostics: Diagnostics {
                    solve_time: started_at.elapsed(),
                    candidates_evaluated: 0,
                },
            });
        }

        let depot = PointOfInterest::with_empty_tags(0, request.start);
        let mut all_pois = Vec::with_capacity(candidates.len() + 1);
        all_pois.push(depot.clone());
        all_pois.extend(candidates.iter().cloned());

        let matrix = self
            .travel_time_provider
            .get_travel_time_matrix(&all_pois)
            .map_err(|_| SolveError::InvalidRequest)?;

        let budget_seconds = Duration::from_secs(u64::from(request.duration_minutes) * 60);
        let context = VrpSolveContext::new(&self.scorer, &self.config, &request.interests);
        let (route_pois, total_score) = context.solve(&candidates, &matrix, budget_seconds)?;

        let total_duration = route_duration(&route_pois, &all_pois, &matrix);
        let diagnostics = Diagnostics {
            solve_time: started_at.elapsed(),
            candidates_evaluated: candidates.len() as u64,
        };

        Ok(SolveResponse {
            route: Route::new(route_pois, total_duration),
            score: total_score,
            diagnostics,
        })
    }
}

#[expect(
    clippy::float_arithmetic,
    reason = "candidate selection uses floating-point score and distance heuristics"
)]
fn bounding_box(start: Coord<f64>, duration_minutes: u16, speed_kmh: f64) -> Rect<f64> {
    let duration_hours = f64::from(duration_minutes) / 60.0;
    let distance_km = duration_hours * speed_kmh;
    let radius_deg = distance_km / 111.0;
    Rect::new(
        Coord {
            x: start.x - radius_deg,
            y: start.y - radius_deg,
        },
        Coord {
            x: start.x + radius_deg,
            y: start.y + radius_deg,
        },
    )
}

impl<S, T, C> VrpSolver<S, T, C>
where
    S: PoiStore,
    T: TravelTimeProvider,
    C: Scorer,
{
    fn select_candidates(&self, request: &SolveRequest) -> Vec<PointOfInterest> {
        let bbox = bounding_box(
            request.start,
            request.duration_minutes,
            self.config.average_speed_kmh,
        );

        let mut scored: Vec<(PointOfInterest, f32)> = self
            .store
            .get_pois_in_bbox(&bbox)
            .map(|poi| {
                let score = self.scorer.score(&poi, &request.interests);
                (poi, score)
            })
            .collect();

        scored.sort_unstable_by(|(lhs_poi, lhs_score), (rhs_poi, rhs_score)| {
            rhs_score
                .partial_cmp(lhs_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| lhs_poi.id.cmp(&rhs_poi.id))
        });

        if let Some(max_nodes) = request.max_nodes {
            let max = usize::from(max_nodes);
            scored.truncate(max);
        }

        scored.into_iter().map(|(poi, _)| poi).collect()
    }
}

fn route_duration(
    route_pois: &[PointOfInterest],
    all_pois: &[PointOfInterest],
    matrix: &[Vec<Duration>],
) -> Duration {
    let mut duration = Duration::ZERO;
    let mut prev_index = 0_usize;
    for poi in route_pois {
        let next_index = all_pois
            .iter()
            .position(|candidate| candidate.id == poi.id)
            .unwrap_or(prev_index);
        if let Some(row) = matrix.get(prev_index)
            && let Some(edge) = row.get(next_index)
        {
            duration += *edge;
        }
        prev_index = next_index;
    }
    duration
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::Coord;
    use rstest::rstest;
    use wildside_core::test_support::{MemoryStore, TagScorer, UnitTravelTimeProvider};
    use wildside_core::{InterestProfile, Tags, Theme};

    fn poi(id: u64, x: f64, y: f64, theme: &str) -> PointOfInterest {
        PointOfInterest::new(
            id,
            Coord { x, y },
            Tags::from([(theme.to_owned(), String::new())]),
        )
    }

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
            duration_minutes: 10,
            interests,
            seed: 1,
            max_nodes: Some(2),
        };

        let candidates = solver.select_candidates(&request);
        assert_eq!(candidates.len(), 2);
        let first = candidates.first().expect("expected first candidate");
        assert_eq!(first.id, 1);
        let second = candidates.get(1).expect("expected second candidate");
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
}
