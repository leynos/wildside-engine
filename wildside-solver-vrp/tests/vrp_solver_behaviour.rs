//! Behavioural tests for `VrpSolver` using rstest-bdd.

use std::cell::RefCell;

use geo::Coord;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use wildside_core::test_support::{MemoryStore, TagScorer, UnitTravelTimeProvider};
use wildside_core::{
    InterestProfile, PointOfInterest, SolveError, SolveRequest, SolveResponse, Solver, Theme,
    TravelTimeError, TravelTimeMatrix, TravelTimeProvider,
};
use wildside_solver_vrp::VrpSolver;
use wildside_solver_vrp::test_support::poi;

#[derive(Debug, Clone)]
enum ProviderChoice {
    Unit(UnitTravelTimeProvider),
    Failing,
}

impl Default for ProviderChoice {
    fn default() -> Self {
        Self::Unit(UnitTravelTimeProvider)
    }
}

impl TravelTimeProvider for ProviderChoice {
    fn get_travel_time_matrix(
        &self,
        pois: &[PointOfInterest],
    ) -> Result<TravelTimeMatrix, TravelTimeError> {
        match self {
            Self::Unit(provider) => provider.get_travel_time_matrix(pois),
            Self::Failing => Err(TravelTimeError::EmptyInput),
        }
    }
}

#[derive(Debug)]
struct VrpWorld {
    dataset: RefCell<Vec<PointOfInterest>>,
    provider: RefCell<ProviderChoice>,
    request: RefCell<SolveRequest>,
    outcome: RefCell<Option<Result<SolveResponse, SolveError>>>,
}

impl VrpWorld {
    fn new() -> Self {
        Self {
            dataset: RefCell::new(Vec::new()),
            provider: RefCell::new(ProviderChoice::default()),
            request: RefCell::new(SolveRequest {
                start: Coord { x: 0.0, y: 0.0 },
                end: None,
                duration_minutes: 10,
                interests: InterestProfile::new(),
                seed: 1,
                max_nodes: None,
            }),
            outcome: RefCell::new(None),
        }
    }

    #[expect(
        clippy::expect_used,
        reason = "behaviour tests use expect for readable failures"
    )]
    fn expect_outcome(&self) -> Result<SolveResponse, SolveError> {
        self.outcome
            .borrow()
            .as_ref()
            .cloned()
            .expect("outcome should be recorded before assertions")
    }
}

#[fixture]
fn world() -> VrpWorld {
    VrpWorld::new()
}

#[given("a memory POI store with points near the origin")]
fn given_store_with_points(world: &VrpWorld) {
    let pois = vec![poi(1, 0.0, 0.0, "art"), poi(2, 0.001, 0.0, "history")];
    world.dataset.replace(pois);
}

#[given("a memory POI store with no points near the origin")]
fn given_store_without_points(world: &VrpWorld) {
    world.dataset.replace(vec![poi(10, 5.0, 5.0, "art")]);
}

#[given("a unit travel time provider")]
fn given_unit_provider(world: &VrpWorld) {
    world
        .provider
        .replace(ProviderChoice::Unit(UnitTravelTimeProvider));
}

#[given("a failing travel time provider")]
fn given_failing_provider(world: &VrpWorld) {
    world.provider.replace(ProviderChoice::Failing);
}

#[given("a tag scorer")]
fn given_tag_scorer(world: &VrpWorld) {
    // TagScorer has no state to initialize.
    let _ = world;
}

#[given("a valid solve request with interests")]
fn given_valid_request(world: &VrpWorld) {
    let interests = InterestProfile::new()
        .with_weight(Theme::Art, 0.8)
        .with_weight(Theme::History, 0.5);
    world.request.replace(SolveRequest {
        start: Coord { x: 0.0, y: 0.0 },
        end: None,
        duration_minutes: 10,
        interests,
        seed: 1,
        max_nodes: None,
    });
}

#[when("the VRP solver runs")]
fn when_solver_runs(world: &VrpWorld) {
    let store = MemoryStore::with_pois(world.dataset.borrow().clone());
    let provider = world.provider.borrow().clone();
    let solver = VrpSolver::new(store, provider, TagScorer);
    let request = world.request.borrow().clone();
    let outcome = solver.solve(&request);
    world.outcome.replace(Some(outcome));
}

#[then("a route is returned containing in-bbox POIs")]
#[expect(
    clippy::expect_used,
    reason = "behaviour tests use expect for readable failures"
)]
fn then_route_returned(world: &VrpWorld) {
    let response = world.expect_outcome().expect("expected solve success");
    assert!(!response.route.pois().is_empty());
}

#[then("the route score is positive")]
#[expect(
    clippy::expect_used,
    reason = "behaviour tests use expect for readable failures"
)]
fn then_score_positive(world: &VrpWorld) {
    let response = world.expect_outcome().expect("expected solve success");
    assert!(response.score > 0.0);
}

#[then("the solve fails with InvalidRequest")]
#[expect(
    clippy::expect_used,
    reason = "behaviour tests use expect for readable failures"
)]
fn then_invalid_request(world: &VrpWorld) {
    let err = world
        .expect_outcome()
        .expect_err("expected InvalidRequest error");
    assert!(matches!(err, SolveError::InvalidRequest));
}

#[then("an empty route is returned")]
#[expect(
    clippy::expect_used,
    clippy::float_cmp,
    reason = "behaviour tests use expect and strict float checks for clarity"
)]
fn then_empty_route(world: &VrpWorld) {
    let response = world.expect_outcome().expect("expected solve success");
    assert!(response.route.pois().is_empty());
    assert_eq!(response.score, 0.0);
}

#[scenario(path = "tests/features/vrp_solver.feature", index = 0)]
fn valid_solve(world: VrpWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/vrp_solver.feature", index = 1)]
fn failing_travel_time(world: VrpWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/vrp_solver.feature", index = 2)]
fn empty_candidates(world: VrpWorld) {
    let _ = world;
}
