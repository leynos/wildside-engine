//! Optional OR-Tools-based solver implementation.
//!
//! This crate currently provides a stub solver that compiles behind the
//! `solver-ortools` feature flag. It reserves the API surface for a future
//! CP-SAT implementation without pulling native OR-Tools dependencies yet.

#![forbid(unsafe_code)]

use wildside_core::TravelTimeProvider;
use wildside_core::{PoiStore, Scorer, SolveError, SolveRequest, SolveResponse, Solver};

/// Placeholder solver for the optional OR-Tools backend.
#[derive(Debug)]
pub struct OrtoolsSolver<S, T, C>
where
    S: PoiStore,
    T: TravelTimeProvider,
    C: Scorer,
{
    _store: S,
    _travel_time_provider: T,
    _scorer: C,
}

impl<S, T, C> OrtoolsSolver<S, T, C>
where
    S: PoiStore,
    T: TravelTimeProvider,
    C: Scorer,
{
    /// Construct a placeholder OR-Tools solver.
    pub const fn new(store: S, travel_time_provider: T, scorer: C) -> Self {
        Self {
            _store: store,
            _travel_time_provider: travel_time_provider,
            _scorer: scorer,
        }
    }
}

impl<S, T, C> Solver for OrtoolsSolver<S, T, C>
where
    S: PoiStore + Send + Sync,
    T: TravelTimeProvider + Send + Sync,
    C: Scorer + Send + Sync,
{
    fn solve(&self, _request: &SolveRequest) -> Result<SolveResponse, SolveError> {
        Err(SolveError::NotImplemented)
    }
}
