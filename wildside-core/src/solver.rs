//! Solver API: request/response types, error, and trait.
//! Implementations MUST be Send + Sync and return InvalidRequest for bad inputs.
//! Use [`SolveRequest::validate`] to enforce basic invariants.
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
    pub start: geo::Coord<f64>,
    /// Time budget in minutes.
    pub duration_minutes: u16,
    /// Visitor interest profile guiding POI selection.
    pub interests: InterestProfile,
    /// Seed for reproducible stochastic components.
    pub seed: u64,
}

impl SolveRequest {
    /// Validates invariants required by solvers.
    ///
    /// Returns [`SolveError::InvalidRequest`] when the time budget is zero or the
    /// start coordinates are non-finite.
    pub fn validate(&self) -> Result<(), SolveError> {
        if self.duration_minutes == 0 {
            return Err(SolveError::InvalidRequest);
        }
        if !(self.start.x.is_finite() && self.start.y.is_finite()) {
            return Err(SolveError::InvalidRequest);
        }
        Ok(())
    }
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
    /// Request parameters were invalid, e.g. zero duration or non-finite coordinates.
    #[error("invalid request")]
    InvalidRequest,
}

/// Find a route satisfying the caller's preferences and constraints.
///
/// Implementations should return [`SolveError::InvalidRequest`] for invalid
/// parameters rather than panicking.
///
/// # Thread Safety
/// Implementations must avoid shared mutable state or use proper synchronisation
/// to ensure thread safety. Solvers must be `Send + Sync` to operate safely across threads.
pub trait Solver: Send + Sync {
    /// Solve a request, producing a route or an error.
    fn solve(&self, request: &SolveRequest) -> Result<SolveResponse, SolveError>;
}
