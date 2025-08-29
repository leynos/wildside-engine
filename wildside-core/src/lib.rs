//! Core domain types for the Wildside engine.

pub mod poi;
pub mod profile;
pub mod route;
pub mod store;
pub mod theme;
pub mod travel_time;

pub use poi::PointOfInterest;
pub use profile::InterestProfile;
pub use route::Route;
pub use store::PoiStore;
pub use theme::Theme;
pub use travel_time::{TravelTimeError, TravelTimeMatrix, TravelTimeProvider};

#[cfg(any(test, feature = "test-utils"))]
pub mod test_support;
