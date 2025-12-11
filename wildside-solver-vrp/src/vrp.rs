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
use wildside_core::{InterestProfile, PointOfInterest, Scorer, SolveError};

use crate::solver::VrpSolverConfig;

custom_dimension!(JobScore typeof Cost);
custom_solution_state!(ScoreFitness typeof Cost);

struct ScoreObjective;

impl FeatureObjective for ScoreObjective {
    fn fitness(&self, solution: &InsertionContext) -> Cost {
        let solution_ctx = &solution.solution;
        solution_ctx
            .state
            .get_score_fitness()
            .copied()
            .unwrap_or_else(|| calculate_solution_fitness(solution_ctx))
    }

    fn estimate(&self, move_ctx: &MoveContext<'_>) -> Cost {
        match move_ctx {
            MoveContext::Route { job, .. } => estimate_job_cost(job),
            MoveContext::Activity { .. } => 0.0,
        }
    }
}

struct ScoreState;

impl FeatureState for ScoreState {
    fn accept_insertion(&self, solution_ctx: &mut SolutionContext, route_index: usize, _job: &Job) {
        if let Some(route_ctx) = solution_ctx.routes.get_mut(route_index) {
            self.accept_route_state(route_ctx);
        }
    }

    fn accept_route_state(&self, _route_ctx: &mut RouteContext) {}

    fn accept_solution_state(&self, solution_ctx: &mut SolutionContext) {
        let fitness = calculate_solution_fitness(solution_ctx);
        solution_ctx.state.set_score_fitness(fitness);
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

fn calculate_solution_fitness(solution_ctx: &SolutionContext) -> Cost {
    solution_ctx
        .routes
        .iter()
        .flat_map(|route_ctx| route_ctx.route().tour.jobs())
        .map(estimate_job_cost)
        .sum()
}

fn define_goal(transport: Arc<dyn TransportCost>) -> GenericResult<GoalContext> {
    let transport_feature = TransportFeatureBuilder::new("min-travel-time")
        .set_transport_cost(transport)
        .set_time_constrained(true)
        .build_minimize_distance()?;

    let score_feature = FeatureBuilder::default()
        .with_name("maximize-score")
        .with_objective(ScoreObjective)
        .with_state(ScoreState)
        .build()?;

    GoalContextBuilder::with_features(&[score_feature, transport_feature])?.build()
}

struct ProblemSpec<'a> {
    candidates: &'a [PointOfInterest],
    scores: &'a [f32],
    transport: Arc<dyn TransportCost>,
    goal: GoalContext,
    budget_seconds: Duration,
}

fn define_problem(spec: ProblemSpec<'_>) -> GenericResult<Problem> {
    let ProblemSpec {
        candidates,
        scores,
        transport,
        goal,
        budget_seconds,
    } = spec;

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
                .set_end_location(0)
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
        self.durations
            .get(from_idx)
            .and_then(|row| row.get(to_idx))
            .copied()
            .unwrap_or(0.0)
    }
}

impl TransportCost for TravelTimeTransportCost {
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

    fn distance_approx(&self, _profile: &Profile, from: usize, to: usize) -> f64 {
        self.duration_seconds(from, to)
    }

    fn duration_approx(&self, _profile: &Profile, from: usize, to: usize) -> f64 {
        self.duration_seconds(from, to)
    }
}

/// Context for running a `vrp-core` solve with shared inputs.
pub(super) struct VrpSolveContext<'a, C: Scorer> {
    scorer: &'a C,
    config: &'a VrpSolverConfig,
    interests: &'a InterestProfile,
}

impl<'a, C: Scorer> VrpSolveContext<'a, C> {
    /// Create a new solve context.
    pub(super) const fn new(
        scorer: &'a C,
        config: &'a VrpSolverConfig,
        interests: &'a InterestProfile,
    ) -> Self {
        Self {
            scorer,
            config,
            interests,
        }
    }

    /// Solve the VRP instance using the provided candidates and matrix.
    pub(super) fn solve(
        &self,
        candidates: &[PointOfInterest],
        matrix: &[Vec<Duration>],
        budget_seconds: Duration,
    ) -> Result<(Vec<PointOfInterest>, f32), SolveError> {
        let scores: Vec<f32> = candidates
            .iter()
            .map(|poi| self.scorer.score(poi, self.interests))
            .collect();

        let transport = Arc::new(TravelTimeTransportCost::new(matrix));
        let goal = define_goal(transport.clone()).map_err(|_| SolveError::InvalidRequest)?;
        let problem_spec = ProblemSpec {
            candidates,
            scores: &scores,
            transport,
            goal,
            budget_seconds,
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

        let locations: Vec<Location> = solution
            .get_locations()
            .next()
            .map(Iterator::collect)
            .unwrap_or_default();

        let mut pois = Vec::new();
        for loc in locations {
            let idx = loc;
            if idx == 0 {
                continue;
            }
            if let Some(poi) = candidates.get(idx - 1) {
                pois.push(poi.clone());
            }
        }

        let total_score = pois
            .iter()
            .map(|poi| self.scorer.score(poi, self.interests))
            .sum();

        Ok((pois, total_score))
    }
}
