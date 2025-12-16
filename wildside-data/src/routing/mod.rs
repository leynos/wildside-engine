//! HTTP-based travel time providers for routing services.
//!
//! This module provides [`HttpTravelTimeProvider`], an implementation of
//! [`wildside_core::TravelTimeProvider`] that fetches travel time matrices
//! from an OSRM routing service.
//!
//! # Architecture
//!
//! The provider makes HTTP requests to the OSRM Table API to compute pairwise
//! travel times between POIs. The synchronous [`TravelTimeProvider`] trait is
//! implemented by blocking on async HTTP calls internally, keeping the core
//! library embeddable in synchronous contexts.
//!
//! # Example
//!
//! ```no_run
//! use wildside_data::routing::{HttpTravelTimeProvider, HttpTravelTimeProviderConfig};
//! use wildside_core::{PointOfInterest, TravelTimeProvider};
//! use geo::Coord;
//! use std::time::Duration;
//!
//! // Create a provider with custom configuration
//! let config = HttpTravelTimeProviderConfig::new("http://localhost:5000")
//!     .with_timeout(Duration::from_secs(60))
//!     .with_user_agent("my-app/1.0");
//! let provider = HttpTravelTimeProvider::with_config(config);
//!
//! // Or use the simple constructor
//! let provider = HttpTravelTimeProvider::new("http://localhost:5000");
//!
//! let pois = vec![
//!     PointOfInterest::with_empty_tags(1, Coord { x: -0.1, y: 51.5 }),
//!     PointOfInterest::with_empty_tags(2, Coord { x: -0.2, y: 51.6 }),
//! ];
//!
//! let matrix = provider.get_travel_time_matrix(&pois)?;
//! println!("Travel time: {:?}", matrix[0][1]);
//! # Ok::<(), wildside_core::TravelTimeError>(())
//! ```

mod osrm;
mod provider;

#[doc(hidden)]
pub mod test_support;

pub use provider::{DEFAULT_USER_AGENT, HttpTravelTimeProvider, HttpTravelTimeProviderConfig};
