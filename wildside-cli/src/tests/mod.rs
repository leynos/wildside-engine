//! Shared test harness modules for the Wildside CLI.
#![expect(
    clippy::panic,
    reason = "Tests assert panic branches to surface unexpected CLI outcomes"
)]

use super::*;

mod helpers;
mod pipeline;
mod pipeline_steps;
mod solve_steps;
mod solve_unit;
mod steps;
mod unit;
