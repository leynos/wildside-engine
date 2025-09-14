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
    let result = solver.solve(&req).is_ok();
    assert_eq!(result, should_succeed);
}
