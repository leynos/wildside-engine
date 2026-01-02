//! Shared test harness modules for the Wildside CLI.
#![expect(
    clippy::panic,
    reason = "Tests assert panic branches to surface unexpected CLI outcomes"
)]

use super::*;

mod feature_flag_steps;
mod feature_flags;
mod helpers;
mod pipeline;
mod pipeline_steps;
mod solve_steps;
mod solve_unit;
mod steps;
mod unit;
