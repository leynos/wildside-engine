//! Travel time errors returned by `TravelTimeProvider` implementations.

use thiserror::Error;

/// Errors from [`crate::travel_time::TravelTimeProvider::get_travel_time_matrix`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TravelTimeError {
    /// No points of interest were provided.
    ///
    /// The provider requires at least one POI to compute a matrix. Callers
    /// should pre-filter input to avoid this condition.
    #[error("at least one point of interest is required")]
    EmptyInput,
}
