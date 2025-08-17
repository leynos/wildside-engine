//! Data access and ingestion logic for the Wildside engine.
//!
//! Responsibilities:
//! - Define repository and source traits for ingestion and query.
//! - Provide adapters for files, HTTP and databases.
//! - Encapsulate serialisation formats and schema evolution.
//!
//! Boundaries:
//! - Do not encode domain rules (live in `wildside-core`).
//! - Keep blocking I/O off async executors; prefer async-capable clients.
//!
//! Invariants:
//! - Thread-safe by default where feasible.
//! - No global mutable state.
