//! Compute travel times between points of interest.
//!
//! The `TravelTimeProvider` trait abstracts the retrieval of pairwise travel
//! times between [`PointOfInterest`](crate::PointOfInterest) values. Callers
//! supply a slice of POIs and receive an adjacency matrix of
//! [`Duration`](std::time::Duration) values.
//!
//! Errors are returned when inputs are invalid, e.g. an empty slice.

mod error;
mod provider;

pub use error::TravelTimeError;
pub use provider::{TravelTimeMatrix, TravelTimeProvider};
