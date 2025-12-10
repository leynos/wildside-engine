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
///     max_nodes: Some(50),
/// };
/// assert_eq!(request.duration_minutes, 30);
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// Optional upper bound on candidate POIs considered by the solver.
    ///
    /// This is a hint to prune expensive searches. A value of zero is
    /// rejected by [`SolveRequest::validate`]; `None` leaves the solver free
    /// to choose its own limits.
    pub max_nodes: Option<u16>,
}

impl SolveRequest {
    /// Validates invariants required by solvers.
    ///
    /// Returns [`SolveError::InvalidRequest`] when the time budget is zero or the
    /// start coordinates are non-finite. A provided `max_nodes` hint must be
    /// greater than zero.
    pub fn validate(&self) -> Result<(), SolveError> {
        if self.duration_minutes == 0 {
            return Err(SolveError::InvalidRequest);
        }
        if !(self.start.x.is_finite() && self.start.y.is_finite()) {
            return Err(SolveError::InvalidRequest);
        }
        if matches!(self.max_nodes, Some(0)) {
            return Err(SolveError::InvalidRequest);
        }
        Ok(())
    }
}

/// Telemetry from a solve operation.
///
/// Contains metrics describing solver execution, useful for performance
/// monitoring and debugging.
///
/// # Examples
/// ```rust
/// use std::time::Duration;
/// use wildside_core::Diagnostics;
///
/// let diagnostics = Diagnostics {
///     solve_time: Duration::from_millis(42),
///     candidates_evaluated: 150,
/// };
/// assert_eq!(diagnostics.candidates_evaluated, 150);
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diagnostics {
    /// Time taken to produce the solution.
    pub solve_time: std::time::Duration,
    /// Number of candidate POIs evaluated by the solver.
    pub candidates_evaluated: u64,
}

/// Response from a successful solve.
///
/// Contains the chosen [`Route`], its aggregate score, and [`Diagnostics`]
/// describing solver execution.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct SolveResponse {
    /// The ordered route for the visitor.
    pub route: Route,
    /// Total score accumulated along the route.
    pub score: f32,
    /// Telemetry from the solve operation.
    pub diagnostics: Diagnostics,
}

/// Errors returned by [`Solver::solve`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, Error)]
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
