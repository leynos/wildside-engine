#![cfg_attr(docsrs, feature(doc_cfg))]

//! Core domain types for the Wildside engine.

pub mod poi;
pub mod profile;
pub mod route;
pub mod scorer;
pub mod solver;
pub mod store;
pub mod theme;
pub mod travel_time;

pub use poi::{PointOfInterest, SpatialIndex, Tags, build_spatial_index};
pub use profile::InterestProfile;
pub use route::Route;
pub use scorer::Scorer;
pub use solver::{SolveError, SolveRequest, SolveResponse, Solver};
pub use store::PoiStore;
pub use theme::Theme;
pub use travel_time::{TravelTimeError, TravelTimeMatrix, TravelTimeProvider};

#[cfg(any(test, feature = "test-support"))]
#[cfg_attr(all(not(test), docsrs), doc(cfg(feature = "test-support")))]
pub mod test_support;

#[cfg(any(test, feature = "test-support"))]
#[cfg_attr(all(not(test), docsrs), doc(cfg(feature = "test-support")))]
pub use crate::test_support::TagScorer;
