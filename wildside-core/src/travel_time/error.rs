//! Travel time errors returned by `TravelTimeProvider` implementations.

use thiserror::Error;

/// Errors from [`crate::travel_time::TravelTimeProvider::get_travel_time_matrix`].
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TravelTimeError {
    /// No points of interest were provided.
    ///
    /// The provider requires at least one POI to compute a matrix. Callers
    /// should pre-filter input to avoid this condition.
    #[error("at least one point of interest is required")]
    EmptyInput,

    /// HTTP request failed with an error status.
    ///
    /// The routing service returned an HTTP error response (4xx or 5xx).
    #[error("routing request to {url} failed with status {status}: {message}")]
    HttpError {
        /// The URL that was requested.
        url: String,
        /// The HTTP status code.
        status: u16,
        /// A human-readable error message.
        message: String,
    },

    /// Network error during routing request.
    ///
    /// A connection or transport-level error occurred when contacting the
    /// routing service.
    #[error("network error contacting routing service at {url}: {message}")]
    NetworkError {
        /// The URL that was requested.
        url: String,
        /// A human-readable error message.
        message: String,
    },

    /// Request timed out.
    ///
    /// The routing service did not respond within the configured timeout.
    #[error("routing request to {url} timed out after {timeout_secs} seconds")]
    Timeout {
        /// The URL that was requested.
        url: String,
        /// The timeout duration in seconds.
        timeout_secs: u64,
    },

    /// Failed to parse the routing service response.
    ///
    /// The routing service returned a response that could not be deserialised.
    #[error("failed to parse routing response: {message}")]
    ParseError {
        /// A human-readable error message.
        message: String,
    },

    /// The routing service returned an error in its response body.
    ///
    /// The routing service responded with a valid HTTP response but indicated
    /// an error condition in the response payload (e.g., invalid coordinates).
    #[error("routing service error: {code} - {message}")]
    ServiceError {
        /// The error code from the routing service.
        code: String,
        /// A human-readable error message.
        message: String,
    },
}
