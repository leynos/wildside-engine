use geo::Coord;
use thiserror::Error;

use crate::{InterestProfile, Route};

/// Parameters for a solve request.
///
/// The request captures the starting point, the time budget in minutes, the
/// caller's interests and a random seed for deterministic results.
///
/// # Examples
/// ```rust
/// use geo::Coord;
/// use wildside_core::{InterestProfile, SolveRequest};
///
/// let request = SolveRequest {
///     start: Coord { x: 0.0, y: 0.0 },
///     duration_minutes: 30,
///     interests: InterestProfile::new(),
///     seed: 1,
/// };
/// assert_eq!(request.duration_minutes, 30);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SolveRequest {
    /// Start location for the tour.
    pub start: Coord<f64>,
    /// Time budget in minutes.
    pub duration_minutes: u16,
    /// Visitor interest profile guiding POI selection.
    pub interests: InterestProfile,
    /// Seed for reproducible stochastic components.
    pub seed: u64,
}

/// Response from a successful solve.
///
/// Contains the chosen [`Route`] and its aggregate score.
#[derive(Debug, Clone, PartialEq)]
pub struct SolveResponse {
    /// The ordered route for the visitor.
    pub route: Route,
    /// Total score accumulated along the route.
    pub score: f32,
}

/// Errors returned by [`Solver::solve`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SolveError {
    /// Request parameters were invalid, e.g. zero duration.
    #[error("invalid request")]
    InvalidRequest,
}

/// Alias for the solver error type.
pub type Error = SolveError;

/// Find a route satisfying the caller's preferences and constraints.
///
/// Implementations should return [`Error::InvalidRequest`] for invalid
/// parameters rather than panicking.
/// Solvers must be `Send + Sync` to operate safely across threads.
pub trait Solver: Send + Sync {
    /// Solve a request, producing a route or an error.
    fn solve(&self, request: &SolveRequest) -> Result<SolveResponse, Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::Coord;
    use rstest::rstest;
    use std::time::Duration;

    struct DummySolver;

    impl Solver for DummySolver {
        fn solve(&self, request: &SolveRequest) -> Result<SolveResponse, Error> {
            if request.duration_minutes == 0 {
                Err(Error::InvalidRequest)
            } else {
                Ok(SolveResponse {
                    route: Route::new(Vec::new(), Duration::from_secs(0)),
                    score: 0.0,
                })
            }
        }
    }

    #[rstest]
    fn returns_response_on_valid_request() {
        let solver = DummySolver;
        let request = SolveRequest {
            start: Coord { x: 0.0, y: 0.0 },
            duration_minutes: 1,
            interests: InterestProfile::new(),
            seed: 0,
        };
        let response = solver.solve(&request).expect("valid request");
        assert!(response.route.pois().is_empty());
    }

    #[rstest]
    fn returns_error_on_zero_duration() {
        let solver = DummySolver;
        let request = SolveRequest {
            start: Coord { x: 0.0, y: 0.0 },
            duration_minutes: 0,
            interests: InterestProfile::new(),
            seed: 0,
        };
        let err = solver.solve(&request).expect_err("zero duration");
        assert_eq!(err, Error::InvalidRequest);
    }
}
