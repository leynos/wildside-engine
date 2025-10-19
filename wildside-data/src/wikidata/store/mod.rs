//! Persistence layer for Wikidata claims in the `pois.db` SQLite database.
//!
//! The module is split into two focused submodules:
//! - [`schema`] materialises the SQLite structures that back the POI metadata.
//! - [`persistence`] writes extracted claims into those tables.
#![forbid(unsafe_code)]

mod persistence;
mod schema;

pub use persistence::{PersistClaimsError, persist_claims, persist_claims_to_path};
pub use schema::{ClaimsSchemaError, SCHEMA_VERSION, initialise_schema};

#[cfg(test)]
mod tests;
