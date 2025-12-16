//! `VrpSolver` implementation backed by `vrp-core`.
//!
//! Supports point-to-point routing when `SolveRequest::end` is set.

use std::time::{Duration, Instant};

use geo::{Coord, Rect};
use wildside_core::{
    Diagnostics, PoiStore, PointOfInterest, Route, Scorer, SolveError, SolveRequest, SolveResponse,
    Solver, TravelTimeProvider,
};

use crate::vrp::VrpInstance;
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

        let scored_candidates = self.select_candidates(request);
        if scored_candidates.is_empty() {
            if let Some(end_coord) = request.end {
                let start = PointOfInterest::with_empty_tags(0, request.start);
                let end_poi = PointOfInterest::with_empty_tags(u64::MAX, end_coord);
                let all_pois = vec![start, end_poi];
                let matrix = self
                    .travel_time_provider
                    .get_travel_time_matrix(&all_pois)
                    .map_err(|_| SolveError::InvalidRequest)?;
                let total_duration = final_leg_duration(0, 1, &matrix);
                return Ok(SolveResponse {
                    route: Route::new(Vec::new(), total_duration),
                    score: 0.0,
                    diagnostics: Diagnostics {
                        solve_time: started_at.elapsed(),
                        candidates_evaluated: 0,
                    },
                });
            }
            return Ok(SolveResponse {
                route: Route::empty(),
                score: 0.0,
                diagnostics: Diagnostics {
                    solve_time: started_at.elapsed(),
                    candidates_evaluated: 0,
                },
            });
        }

        let (candidates, scores): (Vec<PointOfInterest>, Vec<f32>) =
            scored_candidates.into_iter().unzip();
        let depot = PointOfInterest::with_empty_tags(0, request.start);
        let end_poi = request
            .end
            .map(|end_coord| PointOfInterest::with_empty_tags(u64::MAX, end_coord));
        let mut all_pois =
            Vec::with_capacity(candidates.len() + 1 + usize::from(end_poi.is_some()));
        all_pois.push(depot);
        all_pois.extend(candidates.iter().cloned());
        if let Some(end_poi_value) = end_poi.clone() {
            all_pois.push(end_poi_value);
        }

        let matrix = self
            .travel_time_provider
            .get_travel_time_matrix(&all_pois)
            .map_err(|_| SolveError::InvalidRequest)?;

        let end_location = end_poi.as_ref().map_or(0, |_| all_pois.len() - 1);
        let budget_seconds = Duration::from_secs(u64::from(request.duration_minutes) * 60);
        let context = VrpSolveContext::new(&self.config);
        let instance = VrpInstance::new(&candidates, &scores, &matrix, budget_seconds);
        let (route_pois, total_score) = context.solve(&instance, end_location)?;

        let total_duration = route_duration(&route_pois, &all_pois, &matrix, end_location);
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
fn bounding_box(
    start: Coord<f64>,
    end_coord: Option<Coord<f64>>,
    duration_minutes: u16,
    speed_kmh: f64,
) -> Rect<f64> {
    let duration_hours = f64::from(duration_minutes) / 60.0;
    let distance_km = duration_hours * speed_kmh;
    let radius_deg = distance_km / 111.0;
    let min_x = end_coord.map_or(start.x, |end| start.x.min(end.x));
    let max_x = end_coord.map_or(start.x, |end| start.x.max(end.x));
    let min_y = end_coord.map_or(start.y, |end| start.y.min(end.y));
    let max_y = end_coord.map_or(start.y, |end| start.y.max(end.y));
    Rect::new(
        Coord {
            x: min_x - radius_deg,
            y: min_y - radius_deg,
        },
        Coord {
            x: max_x + radius_deg,
            y: max_y + radius_deg,
        },
    )
}

impl<S, T, C> VrpSolver<S, T, C>
where
    S: PoiStore,
    T: TravelTimeProvider,
    C: Scorer,
{
    fn select_candidates(&self, request: &SolveRequest) -> Vec<(PointOfInterest, f32)> {
        let bbox = bounding_box(
            request.start,
            request.end,
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

        scored
    }
}

fn build_poi_index(all_pois: &[PointOfInterest]) -> std::collections::HashMap<u64, usize> {
    all_pois
        .iter()
        .enumerate()
        .map(|(idx, poi)| (poi.id, idx))
        .collect()
}

fn final_leg_duration(from_index: usize, end_index: usize, matrix: &[Vec<Duration>]) -> Duration {
    if from_index == end_index {
        return Duration::ZERO;
    }

    matrix
        .get(from_index)
        .and_then(|row| row.get(end_index))
        .copied()
        .map_or_else(
            || {
                log::warn!(
                    "Matrix access failed for final leg from index {from_index} to index {end_index}; falling back to zero duration"
                );
                debug_assert!(
                    false,
                    "Matrix access failed for final leg from index {from_index} to index {end_index}"
                );
                Duration::ZERO
            },
            |duration| duration,
        )
}

fn route_duration(
    route_pois: &[PointOfInterest],
    all_pois: &[PointOfInterest],
    matrix: &[Vec<Duration>],
    end_index: usize,
) -> Duration {
    let mut duration = Duration::ZERO;
    let mut prev_index = 0_usize;
    let poi_index = build_poi_index(all_pois);
    for poi in route_pois {
        let poi_id = poi.id;
        let next_index = poi_index.get(&poi_id).copied().unwrap_or_else(|| {
            log::warn!(
                "POI {poi_id} not found in POI index; falling back to previous index {prev_index}"
            );
            debug_assert!(false, "POI {poi_id} not found in index");
            prev_index
        });
        if let Some(row) = matrix.get(prev_index)
            && let Some(edge) = row.get(next_index)
        {
            duration += *edge;
        }
        prev_index = next_index;
    }
    duration + final_leg_duration(prev_index, end_index, matrix)
}

#[cfg(test)]
mod tests;
