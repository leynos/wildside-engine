//! `vrp-core` modelling helpers for `VrpSolver`.
//!
//! This module converts scored POI candidates and a travel-time matrix into a
//! `vrp-core` problem, runs the solver, and translates the resulting tour back
//! into Wildside types.

use std::sync::Arc;
use std::time::Duration;

use vrp_core::models::common::{Location, Profile};
use vrp_core::models::problem::TravelTime;
use vrp_core::models::solution::Route as VrpRoute;
use vrp_core::prelude::*;
use wildside_core::{PointOfInterest, SolveError};

use crate::solver::VrpSolverConfig;

custom_dimension!(JobScore typeof Cost);

struct ScoreObjective;

impl FeatureObjective for ScoreObjective {
    fn fitness(&self, solution: &InsertionContext) -> Cost {
        solution
            .solution
            .routes
            .iter()
            .flat_map(|route_ctx| route_ctx.route().tour.jobs())
            .map(estimate_job_cost)
            .sum()
    }

    fn estimate(&self, move_ctx: &MoveContext<'_>) -> Cost {
        match move_ctx {
            MoveContext::Route { job, .. } => estimate_job_cost(job),
            MoveContext::Activity { .. } => 0.0,
        }
    }
}

#[expect(
    clippy::float_arithmetic,
    reason = "objective cost uses floating-point POI scores"
)]
fn estimate_job_cost(job: &Job) -> Cost {
    job.dimens()
        .get_job_score()
        .copied()
        .map_or(0.0, |score| -score)
}

fn define_goal(transport: Arc<dyn TransportCost>) -> GenericResult<GoalContext> {
    let transport_feature = TransportFeatureBuilder::new("min-travel-time")
        .set_transport_cost(transport)
        .set_time_constrained(true)
        .build_minimize_distance()?;

    let score_feature = FeatureBuilder::default()
        .with_name("maximize-score")
        .with_objective(ScoreObjective)
        .build()?;

    GoalContextBuilder::with_features(&[score_feature, transport_feature])?.build()
}

struct ProblemSpec<'a> {
    candidates: &'a [PointOfInterest],
    scores: &'a [f32],
    transport: Arc<dyn TransportCost>,
    goal: GoalContext,
    budget_seconds: Duration,
    end_location: Location,
}

fn define_problem(spec: ProblemSpec<'_>) -> GenericResult<Problem> {
    let ProblemSpec {
        candidates,
        scores,
        transport,
        goal,
        budget_seconds,
        end_location,
    } = spec;

    debug_assert_eq!(
        candidates.len(),
        scores.len(),
        "VRP problem invariant violated: candidates.len() != scores.len()"
    );
    if candidates.len() != scores.len() {
        return Err("VRP problem invariant violated: candidates.len() != scores.len()".into());
    }

    let jobs = candidates
        .iter()
        .zip(scores.iter())
        .enumerate()
        .map(|(idx, (poi, score))| {
            let location = idx + 1;
            SingleBuilder::default()
                .id(format!("poi{}", poi.id).as_str())
                .dimension(|dimens| {
                    dimens.set_job_score(Cost::from(*score));
                })
                .location(location)?
                .build_as_job()
        })
        .collect::<Result<Vec<_>, _>>()?;

    let budget = budget_seconds.as_secs_f64();
    let vehicle = VehicleBuilder::default()
        .id("walker")
        .add_detail(
            VehicleDetailBuilder::default()
                .set_start_location(0)
                .set_start_time(0.0)
                .set_end_location(end_location)
                .set_end_time(budget)
                .build()?,
        )
        .build()?;

    ProblemBuilder::default()
        .add_jobs(jobs.into_iter())
        .add_vehicles(std::iter::once(vehicle))
        .with_goal(goal)
        .with_transport_cost(transport)
        .build()
}

struct TravelTimeTransportCost {
    durations: Vec<Vec<f64>>,
}

impl TravelTimeTransportCost {
    fn new(matrix: &[Vec<Duration>]) -> Self {
        let durations = matrix
            .iter()
            .map(|row| row.iter().map(Duration::as_secs_f64).collect())
            .collect();
        Self { durations }
    }

    fn duration_seconds(&self, from: Location, to: Location) -> f64 {
        let from_idx = from;
        let to_idx = to;
        let result = self
            .durations
            .get(from_idx)
            .and_then(|row| row.get(to_idx))
            .copied();
        debug_assert!(
            result.is_some(),
            "Matrix lookup failed: from={from_idx}, to={to_idx}"
        );
        result.unwrap_or(0.0)
    }
}

impl TransportCost for TravelTimeTransportCost {
    // `distance` and `duration` implement `vrp-core`'s `TransportCost` trait.
    // The trait signature includes `route` and `departure` parameters even
    // though this matrix-backed implementation does not use them. Other
    // `TransportCost` implementations may have route-dependent or
    // time-dependent costs, so these parameters are part of the shared API.
    fn distance(
        &self,
        _route: &VrpRoute,
        from: Location,
        to: Location,
        _departure: TravelTime,
    ) -> Cost {
        self.duration_seconds(from, to)
    }

    fn duration(
        &self,
        _route: &VrpRoute,
        from: Location,
        to: Location,
        _departure: TravelTime,
    ) -> f64 {
        self.duration_seconds(from, to)
    }

    fn distance_approx(&self, profile: &Profile, from: usize, to: usize) -> f64 {
        self.duration_approx(profile, from, to)
    }

    fn duration_approx(&self, _profile: &Profile, from: usize, to: usize) -> f64 {
        self.duration_seconds(from, to)
    }
}

/// Context for running a `vrp-core` solve with shared inputs.
pub(super) struct VrpSolveContext<'a> {
    config: &'a VrpSolverConfig,
}

pub(super) struct VrpInstance<'a> {
    candidates: &'a [PointOfInterest],
    scores: &'a [f32],
    matrix: &'a [Vec<Duration>],
    budget_seconds: Duration,
}

impl<'a> VrpInstance<'a> {
    pub(super) const fn new(
        candidates: &'a [PointOfInterest],
        scores: &'a [f32],
        matrix: &'a [Vec<Duration>],
        budget_seconds: Duration,
    ) -> Self {
        Self {
            candidates,
            scores,
            matrix,
            budget_seconds,
        }
    }
}

impl<'a> VrpSolveContext<'a> {
    /// Create a new solve context.
    pub(super) const fn new(config: &'a VrpSolverConfig) -> Self {
        Self { config }
    }

    /// Solve the VRP instance using the provided candidates and matrix.
    pub(super) fn solve(
        &self,
        instance: &VrpInstance<'_>,
        end_location: Location,
    ) -> Result<(Vec<PointOfInterest>, f32), SolveError> {
        let transport = Arc::new(TravelTimeTransportCost::new(instance.matrix));
        // TODO: Preserve underlying error details once `SolveError` gains richer variants.
        let goal = define_goal(transport.clone()).map_err(|_| SolveError::InvalidRequest)?;
        let problem_spec = ProblemSpec {
            candidates: instance.candidates,
            scores: instance.scores,
            transport,
            goal,
            budget_seconds: instance.budget_seconds,
            end_location,
        };
        let problem =
            Arc::new(define_problem(problem_spec).map_err(|_| SolveError::InvalidRequest)?);

        let vrp_config = VrpConfigBuilder::new(problem.clone())
            .prebuild()
            .map_err(|_| SolveError::InvalidRequest)?
            .with_max_generations(Some(self.config.max_generations))
            .build()
            .map_err(|_| SolveError::InvalidRequest)?;

        let solution = vrp_core::solver::Solver::new(problem, vrp_config)
            .solve()
            .map_err(|_| SolveError::InvalidRequest)?;

        let locations: Vec<Location> = solution.get_locations().flatten().collect();

        let mut pois = Vec::new();
        let mut chosen_scores = Vec::new();
        for loc in locations {
            let idx = loc;
            if idx == 0 {
                continue;
            }
            if let Some(poi) = instance.candidates.get(idx - 1) {
                pois.push(poi.clone());
                chosen_scores.push(instance.scores.get(idx - 1).copied().unwrap_or(0.0_f32));
            }
        }

        let total_score: f32 = chosen_scores.into_iter().sum();

        Ok((pois, total_score))
    }
}
