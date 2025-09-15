//! Tests for the `Solver` trait using a dummy implementation.

use geo::Coord;
use rstest::rstest;
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
