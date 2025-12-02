//! Scoring utilities for Wildside points of interest.
//!
//! The crate provides two complementary capabilities:
//! - **Offline popularity computation** walks a `pois.db` `SQLite` database,
//!   extracts popularity signals, normalizes them into the `0.0..=1.0` range,
//!   and optionally serializes the resulting scores to `popularity.bin` via
//!   `bincode`. Popularity is derived from two signals: Wikidata sitelink
//!   counts per linked entity, and UNESCO World Heritage designation
//!   (`P1435=Q9259`).
//! - **Request-time user relevance scoring** combines per-theme interests from
//!   an [`InterestProfile`](wildside_core::InterestProfile) with fast, indexed
//!   lookups against `pois.db` and the pre-computed popularity scores. It
//!   implements the [`Scorer`](wildside_core::Scorer) trait so callers can
//!   plug the scorer into route solvers.
//!
//! # Examples
//!
//! ```no_run
//! use camino::Utf8Path;
//! use wildside_scorer::{PopularityWeights, write_popularity_file};
//!
//! let db_path = Utf8Path::new("artifacts/pois.db");
//! let output = Utf8Path::new("artifacts/popularity.bin");
//! let weights = PopularityWeights::default();
//! write_popularity_file(db_path, output, weights).expect("persist popularity scores");
//! ```

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;

use bincode::Options;
use camino::Utf8Path;
use rusqlite::Connection;
use wildside_fs::ensure_parent_dir;

mod error;
pub(crate) mod resolver;
mod types;
mod user;

pub use error::PopularityError;
pub use types::{PopularityScores, PopularityWeights};
pub use user::{
    ClaimSelector, ScoreWeights, ThemeClaimMapping, UserRelevanceError, UserRelevanceScorer,
};

use resolver::SitelinkResolver;

pub(crate) const HERITAGE_PROPERTY: &str = "P1435";
pub(crate) const SITELINK_TABLE: &str = "wikidata_entity_sitelinks";
const UNESCO_WORLD_HERITAGE: &str = "Q9259";

/// Bincode options used for serializing and deserializing popularity scores.
pub(crate) fn bincode_options() -> impl bincode::Options {
    bincode::DefaultOptions::new()
}

/// Public helper exposing the bincode configuration used for popularity files.
#[must_use]
pub fn popularity_bincode_options() -> impl bincode::Options {
    bincode_options()
}

/// Compute normalized popularity scores for all POIs in a `pois.db` database.
///
/// # Errors
/// Returns [`PopularityError`] when the `SQLite` database cannot be opened,
/// queried, or when tag payloads contain invalid sitelink values.
pub fn compute_popularity_scores(
    db_path: &Utf8Path,
    weights: PopularityWeights,
) -> Result<PopularityScores, PopularityError> {
    let mut connection = Connection::open(db_path.as_std_path()).map_err(|source| {
        PopularityError::OpenDatabase {
            path: db_path.to_path_buf(),
            source,
        }
    })?;

    let raw = read_raw_scores(&mut connection, weights)?;
    let normalised = normalise_scores(&raw);
    Ok(PopularityScores::new(normalised))
}

/// Compute popularity scores and persist them to `popularity.bin`.
///
/// The parent directory is created when missing. The function returns the
/// in-memory scores as well as writing them to disk.
///
/// # Errors
/// Propagates errors from [`compute_popularity_scores`] and from filesystem
/// interactions when creating the output file or serializing the scores.
pub fn write_popularity_file(
    db_path: &Utf8Path,
    output_path: &Utf8Path,
    weights: PopularityWeights,
) -> Result<PopularityScores, PopularityError> {
    let scores = compute_popularity_scores(db_path, weights)?;
    ensure_parent_dir(output_path).map_err(|source| PopularityError::CreateParent {
        path: output_path
            .parent()
            .map_or_else(|| Utf8Path::new(".").to_path_buf(), Utf8Path::to_path_buf),
        source,
    })?;
    let file =
        File::create(output_path.as_std_path()).map_err(|source| PopularityError::WriteFile {
            path: output_path.to_path_buf(),
            source,
        })?;
    let writer = BufWriter::new(file);
    bincode_options()
        .serialize_into(writer, &scores)
        .map_err(|source| PopularityError::Serialise {
            path: output_path.to_path_buf(),
            source,
        })?;
    Ok(scores)
}

fn read_raw_scores(
    connection: &mut Connection,
    weights: PopularityWeights,
) -> Result<HashMap<u64, f32>, PopularityError> {
    let mut resolver = SitelinkResolver::new(connection)?;
    let mut statement = connection
        .prepare(
            "SELECT
                pois.id,
                pois.tags,
                links.entity_id,
                CASE
                    WHEN links.entity_id IS NULL THEN 0
                    ELSE EXISTS(
                        SELECT 1 FROM wikidata_entity_claims AS claims
                        WHERE claims.entity_id = links.entity_id
                          AND claims.property_id = ?1
                          AND claims.value_entity_id = ?2
                    )
                END AS is_heritage
             FROM pois
             LEFT JOIN poi_wikidata_links AS links ON links.poi_id = pois.id",
        )
        .map_err(|source| PopularityError::Query {
            operation: "prepare POI selection",
            source,
        })?;

    let rows = statement
        .query_map([HERITAGE_PROPERTY, UNESCO_WORLD_HERITAGE], |row| {
            let poi_id_raw: i64 = row.get(0)?;
            let tags: String = row.get(1)?;
            let entity_id: Option<String> = row.get(2)?;
            let heritage: bool = row.get(3)?;

            Ok((poi_id_raw, tags, entity_id, heritage))
        })
        .map_err(|source| PopularityError::Query {
            operation: "query POIs",
            source,
        })?;

    let mut raw_scores = HashMap::new();
    for row in rows {
        let (poi_id_raw, tags, entity_id, heritage) =
            row.map_err(|source| PopularityError::Query {
                operation: "read POI row",
                source,
            })?;
        let poi_id = u64::try_from(poi_id_raw)
            .map_err(|_| PopularityError::PoiIdOutOfRange { poi_id: poi_id_raw })?;
        let sitelinks = resolver.sitelink_count(entity_id.as_deref(), &tags, poi_id)?;
        let score = score_signals(sitelinks, heritage, weights);
        raw_scores.insert(poi_id, score);
    }

    Ok(raw_scores)
}

#[expect(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    reason = "popularity scoring requires floating-point weighting with bounded casts"
)]
fn score_signals(sitelinks: u32, heritage: bool, weights: PopularityWeights) -> f32 {
    let sitelinks_f32 = sitelinks as f32;
    let sitelink_component = weights.sitelink_weight * sitelinks_f32;
    let heritage_component = if heritage {
        weights.heritage_bonus
    } else {
        0.0_f32
    };
    (sitelink_component + heritage_component).max(0.0_f32)
}

#[expect(
    clippy::float_arithmetic,
    reason = "normalizing scores divides by the maximum raw value"
)]
pub(crate) fn normalise_scores(raw: &HashMap<u64, f32>) -> std::collections::BTreeMap<u64, f32> {
    let max = raw.values().copied().fold(0.0_f32, f32::max);
    if max == 0.0_f32 {
        return raw.keys().map(|&id| (id, 0.0_f32)).collect();
    }
    raw.iter()
        .map(|(&id, value)| (id, (value / max).clamp(0.0_f32, 1.0_f32)))
        .collect()
}

#[cfg(test)]
mod tests;
