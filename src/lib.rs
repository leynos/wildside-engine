//! Facade crate for the Wildside recommendation engine.
//!
//! This crate re-exports the core domain types and exposes optional solver and
//! store implementations behind feature flags.

#![forbid(unsafe_code)]

pub use wildside_core::{
    Diagnostics, InterestProfile, PoiStore, PointOfInterest, Route, SolveError, SolveRequest,
    SolveResponse, Solver, Theme, TravelTimeError, TravelTimeMatrix, TravelTimeProvider,
};

#[cfg(feature = "store-sqlite")]
pub use wildside_core::{SqlitePoiStore, SqlitePoiStoreError};

#[cfg(feature = "solver-vrp")]
pub use wildside_solver_vrp::VrpSolver;

#[cfg(feature = "solver-ortools")]
pub use wildside_solver_ortools::OrtoolsSolver;
