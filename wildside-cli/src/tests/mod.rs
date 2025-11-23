//! Shared test harness modules for the ingest CLI.
#![expect(
    clippy::panic,
    reason = "Tests assert panic branches to surface unexpected CLI outcomes"
)]

use super::*;

mod helpers;
mod pipeline;
mod pipeline_steps;
mod steps;
mod unit;
