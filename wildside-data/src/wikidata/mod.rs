//! Wikidata ETL pipeline primitives.
//!
//! This module hosts the download and metadata recording logic that powers the
//! Wikidata ingestion flow. The binary entrypoint wires the HTTP transport and
//! filesystem paths while tests exercise the pure parsing functions with
//! fixtures.

pub mod dump;
pub mod etl;
