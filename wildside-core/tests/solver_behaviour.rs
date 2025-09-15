//! Tests for the `Solver` trait using a dummy implementation.

use geo::Coord;
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use std::time::Duration;
use wildside_core::{InterestProfile, Route, SolveError, SolveRequest, SolveResponse, Solver};

struct DummySolver;

impl Solver for DummySolver {
    fn solve(&self, request: &SolveRequest) -> Result<SolveResponse, SolveError> {
        // `interests` and `seed` are ignored by this stub.
        let _ = (&request.interests, request.seed);
        request.validate()?;
        Ok(SolveResponse {
            route: Route::new(Vec::new(), Duration::from_secs(0)),
            score: 0.0,
        })
    }
}

#[rstest]
#[case(10, true)]
#[case(0, false)]
fn solver_returns_expected(#[case] duration: u16, #[case] should_succeed: bool) {
    let solver = DummySolver;
    let req = SolveRequest {
        start: Coord { x: 0.0, y: 0.0 },
        duration_minutes: duration,
        interests: InterestProfile::new(),
        seed: 1,
    };
    let validation = req.validate();
    let result = solver.solve(&req);

    if should_succeed {
        validation.expect("expected valid request");
        let response = result.expect("expected solve success");
        assert!(response.route.pois().is_empty());
        assert_eq!(response.score, 0.0);
    } else {
        let err = validation.expect_err("expected InvalidRequest");
        assert!(matches!(err, SolveError::InvalidRequest));

        let err = result.expect_err("expected InvalidRequest");
        assert!(matches!(err, SolveError::InvalidRequest));
    }
}

#[rstest]
fn zero_duration_returns_invalid_request() {
    let solver = DummySolver;
    let req = SolveRequest {
        start: Coord { x: 0.0, y: 0.0 },
        duration_minutes: 0,
        interests: InterestProfile::new(),
        seed: 1,
    };

    let err = req.validate().expect_err("expected InvalidRequest");
    assert!(matches!(err, SolveError::InvalidRequest));

    let err = solver.solve(&req).expect_err("expected InvalidRequest");
    assert!(matches!(err, SolveError::InvalidRequest));
}

#[rstest]
#[case(Coord { x: f64::NAN, y: 0.0 })]
#[case(Coord { x: f64::INFINITY, y: 0.0 })]
#[case(Coord { x: 0.0, y: f64::NAN })]
#[case(Coord { x: 0.0, y: f64::NEG_INFINITY })]
fn non_finite_start_is_invalid(#[case] start: Coord<f64>) {
    let solver = DummySolver;
    let req = SolveRequest {
        start,
        duration_minutes: 10,
        interests: InterestProfile::new(),
        seed: 1,
    };

    let err = req.validate().expect_err("expected InvalidRequest");
    assert!(matches!(err, SolveError::InvalidRequest));

    let err = solver.solve(&req).expect_err("expected InvalidRequest");
    assert!(matches!(err, SolveError::InvalidRequest));
}

#[fixture]
fn solver() -> DummySolver {
    DummySolver
}

#[fixture]
fn request() -> RefCell<SolveRequest> {
    RefCell::new(SolveRequest {
        start: Coord { x: 0.0, y: 0.0 },
        duration_minutes: 10,
        interests: InterestProfile::new(),
        seed: 1,
    })
}

#[fixture]
fn outcome() -> RefCell<Result<SolveResponse, SolveError>> {
    RefCell::new(Ok(SolveResponse {
        route: Route::new(Vec::new(), Duration::from_secs(0)),
        score: 0.0,
    }))
}

#[given("a dummy solver")]
fn given_solver(#[from(solver)] _solver: &DummySolver) {
    // Solver has no shared state to initialise.
}

#[given("a valid solve request")]
fn given_valid_request(#[from(request)] request: &RefCell<SolveRequest>) {
    *request.borrow_mut() = SolveRequest {
        start: Coord { x: 0.0, y: 0.0 },
        duration_minutes: 10,
        interests: InterestProfile::new(),
        seed: 1,
    };
}

#[given("a solve request with zero duration")]
fn given_zero_duration_request(#[from(request)] request: &RefCell<SolveRequest>) {
    *request.borrow_mut() = SolveRequest {
        start: Coord { x: 0.0, y: 0.0 },
        duration_minutes: 0,
        interests: InterestProfile::new(),
        seed: 1,
    };
}

#[given("a solve request with a non-finite start coordinate")]
fn given_non_finite_request(#[from(request)] request: &RefCell<SolveRequest>) {
    *request.borrow_mut() = SolveRequest {
        start: Coord {
            x: f64::NAN,
            y: 0.0,
        },
        duration_minutes: 10,
        interests: InterestProfile::new(),
        seed: 1,
    };
}

#[when("I run the solver")]
fn when_run_solver(
    #[from(solver)] solver: &DummySolver,
    #[from(request)] request: &RefCell<SolveRequest>,
    #[from(outcome)] outcome: &RefCell<Result<SolveResponse, SolveError>>,
) {
    let request = request.borrow();
    *outcome.borrow_mut() = solver.solve(&request);
}

#[then("a successful response is produced")]
fn then_successful_response(#[from(outcome)] outcome: &RefCell<Result<SolveResponse, SolveError>>) {
    let borrow = outcome.borrow();
    let response = borrow
        .as_ref()
        .expect("expected solver to succeed with a valid request");
    assert!(response.route.pois().is_empty());
    assert_eq!(response.score, 0.0);
}

#[then("an invalid request error is returned")]
fn then_invalid_request(#[from(outcome)] outcome: &RefCell<Result<SolveResponse, SolveError>>) {
    let borrow = outcome.borrow();
    assert!(
        matches!(&*borrow, Err(SolveError::InvalidRequest)),
        "expected InvalidRequest error, got {borrow:?}"
    );
}

#[scenario(path = "tests/features/solver.feature", index = 0)]
fn valid_request_is_solved(
    solver: DummySolver,
    request: RefCell<SolveRequest>,
    outcome: RefCell<Result<SolveResponse, SolveError>>,
) {
    let _ = (solver, request, outcome);
}

#[scenario(path = "tests/features/solver.feature", index = 1)]
fn zero_duration_request_fails(
    solver: DummySolver,
    request: RefCell<SolveRequest>,
    outcome: RefCell<Result<SolveResponse, SolveError>>,
) {
    let _ = (solver, request, outcome);
}

#[scenario(path = "tests/features/solver.feature", index = 2)]
fn non_finite_request_fails(
    solver: DummySolver,
    request: RefCell<SolveRequest>,
    outcome: RefCell<Result<SolveResponse, SolveError>>,
) {
    let _ = (solver, request, outcome);
}
