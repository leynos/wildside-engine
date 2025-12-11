//! Native Vehicle Routing Problem solver for Wildside.
//!
//! This crate provides [`VrpSolver`], the default implementation of the
//! [`Solver`](wildside_core::Solver) trait. It models the Orienteering Problem as a
//! single-vehicle VRP with optional jobs, using the `vrp-core` metaheuristics to
//! maximise total collected POI score within a time budget.
//!
//! The current implementation is intentionally small and deterministic at the API
//! boundary: it selects candidates synchronously from a [`PoiStore`], queries a
//! [`TravelTimeProvider`] for a routing matrix, then invokes `vrp-core` to search
//! for a good route. Any modelling errors are mapped to
//! [`SolveError::InvalidRequest`], pending expansion of the core error enum.

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod solver;
mod vrp;

pub use solver::{VrpSolver, VrpSolverConfig};
