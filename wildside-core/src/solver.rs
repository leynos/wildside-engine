//! Solver API: request/response types, error, and trait.
//! Implementations MUST be Send + Sync and return InvalidRequest for bad inputs.
//! Use [`SolveRequest::validate`] to enforce basic invariants.
use thiserror::Error;

use crate::{InterestProfile, Route};

/// Detailed validation errors for [`SolveRequest`].
///
/// Solvers typically map invalid inputs to [`SolveError::InvalidRequest`], but
/// command-line tooling and callers may prefer more actionable diagnostics.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Error)]
pub enum SolveRequestValidationError {
    /// The request has a zero-minute time budget.
    #[error("duration_minutes must be greater than zero")]
    ZeroDuration,
    /// The start coordinate contains `NaN` or infinite values.
    #[error("start coordinate must be finite")]
    NonFiniteStart,
    /// The end coordinate contains `NaN` or infinite values.
    #[error("end coordinate must be finite")]
    NonFiniteEnd,
    /// A provided `max_nodes` hint was zero.
    #[error("max_nodes must be greater than zero when supplied")]
    ZeroMaxNodes,
}

/// Parameters for a solve request.
///
/// The request captures the starting point, the time budget in minutes, the
/// caller's interests and a random seed for deterministic results. Optionally,
/// callers can provide an end location to request point-to-point routing.
///
/// # Examples
/// ```rust
/// use geo::Coord;
/// use wildside_core::{InterestProfile, SolveRequest};
///
/// let request = SolveRequest {
///     start: Coord { x: 0.0, y: 0.0 },
///     end: None,
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
    /// Optional end location for point-to-point routes.
    ///
    /// When set, solvers should treat the tour as starting at [`SolveRequest::start`]
    /// and finishing at `end` rather than returning to the start location.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub end: Option<geo::Coord<f64>>,
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
    /// greater than zero. When set, `end` must also be finite.
    pub fn validate(&self) -> Result<(), SolveError> {
        self.validate_detailed()
            .map_err(|_| SolveError::InvalidRequest)
    }

    /// Validates invariants required by solvers while returning actionable
    /// diagnostics.
    ///
    /// This is a more detailed form of [`SolveRequest::validate`] which
    /// preserves the precise reason that validation failed.
    pub fn validate_detailed(&self) -> Result<(), SolveRequestValidationError> {
        if self.duration_minutes == 0 {
            return Err(SolveRequestValidationError::ZeroDuration);
        }
        if !is_valid_coord(&self.start) {
            return Err(SolveRequestValidationError::NonFiniteStart);
        }
        if let Some(end) = self.end
            && !is_valid_coord(&end)
        {
            return Err(SolveRequestValidationError::NonFiniteEnd);
        }
        if matches!(self.max_nodes, Some(0)) {
            return Err(SolveRequestValidationError::ZeroMaxNodes);
        }
        Ok(())
    }
}

/// Checks whether both x and y coordinates are finite.
fn is_valid_coord(coord: &geo::Coord<f64>) -> bool {
    coord.x.is_finite() && coord.y.is_finite()
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
    /// Solver implementation is not yet available.
    #[error("solver not implemented")]
    NotImplemented,
}

/// Find a route satisfying the caller's preferences and constraints.
///
/// Implementations should return [`SolveError::InvalidRequest`] for invalid
/// parameters rather than panicking. Placeholder solvers may return
/// [`SolveError::NotImplemented`] until a backend is available.
///
/// # Thread Safety
/// Implementations must avoid shared mutable state or use proper synchronisation
/// to ensure thread safety. Solvers must be `Send + Sync` to operate safely across threads.
pub trait Solver: Send + Sync {
    /// Solve a request, producing a route or an error.
    fn solve(&self, request: &SolveRequest) -> Result<SolveResponse, SolveError>;
}
