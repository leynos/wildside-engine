//! Wikidata ETL pipeline primitives.
//!
//! This module hosts the download, persistence, and metadata recording logic
//! that powers the Wikidata ingestion flow. It exposes `wikidata::etl` for
//! streaming claim extraction and `wikidata::store` for persisting the resulting
//! facts to SQLite. The binary entrypoint wires the HTTP transport and
//! filesystem paths while tests exercise the pure parsing and persistence
//! functions with fixtures.

pub mod dump;
pub mod etl;
pub mod store;
