//! Behavioural tests for the `Solver` trait using a dummy implementation.

use geo::Coord;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use std::time::Duration;
use wildside_core::{InterestProfile, Route, SolveError, SolveRequest, SolveResponse, Solver};

struct DummySolver;

impl Solver for DummySolver {
    fn solve(&self, request: &SolveRequest) -> Result<SolveResponse, SolveError> {
        if request.duration_minutes == 0 {
            Err(SolveError::InvalidRequest)
        } else {
            Ok(SolveResponse {
                route: Route::new(Vec::new(), Duration::from_secs(0)),
                score: 0.0,
            })
        }
    }
}

#[fixture]
fn solver() -> DummySolver {
    DummySolver
}

#[fixture]
fn result() -> RefCell<Result<SolveResponse, SolveError>> {
    RefCell::new(Err(SolveError::InvalidRequest))
}

#[given("a dummy solver")]
fn given_solver(
    #[from(solver)] _solver: &DummySolver,
    #[from(result)] result: &RefCell<Result<SolveResponse, SolveError>>,
) {
    *result.borrow_mut() = Err(SolveError::InvalidRequest);
}

#[when("I solve with duration 10 minutes")]
fn when_solve_ok(
    #[from(solver)] solver: &DummySolver,
    #[from(result)] result: &RefCell<Result<SolveResponse, SolveError>>,
) {
    let request = SolveRequest {
        start: Coord { x: 0.0, y: 0.0 },
        duration_minutes: 10,
        interests: InterestProfile::new(),
        seed: 1,
    };
    *result.borrow_mut() = solver.solve(&request);
}

#[when("I solve with duration 0 minutes")]
fn when_solve_err(
    #[from(solver)] solver: &DummySolver,
    #[from(result)] result: &RefCell<Result<SolveResponse, SolveError>>,
) {
    let request = SolveRequest {
        start: Coord { x: 0.0, y: 0.0 },
        duration_minutes: 0,
        interests: InterestProfile::new(),
        seed: 1,
    };
    *result.borrow_mut() = solver.solve(&request);
}

#[then("a solve response is returned")]
fn then_response(#[from(result)] result: &RefCell<Result<SolveResponse, SolveError>>) {
    assert!(result.borrow().is_ok(), "expected solve to succeed");
}

#[then("a solve error is returned")]
fn then_error(#[from(result)] result: &RefCell<Result<SolveResponse, SolveError>>) {
    assert!(result.borrow().is_err(), "expected solve to fail");
}

#[scenario(path = "tests/features/solver.feature", index = 0)]
fn solve_success(solver: DummySolver, result: RefCell<Result<SolveResponse, SolveError>>) {
    let _ = (solver, result);
}

#[scenario(path = "tests/features/solver.feature", index = 1)]
fn solve_failure(solver: DummySolver, result: RefCell<Result<SolveResponse, SolveError>>) {
    let _ = (solver, result);
}
