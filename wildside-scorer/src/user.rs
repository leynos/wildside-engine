//! User relevance scoring that combines personalized interests with
//! pre-computed popularity, returning a normalized value in `0.0..=1.0` via
//! the `Scorer` trait.
//!
//! The scorer inspects Wikidata claims stored in `pois.db` to determine whether
//! a point of interest matches the visitor's declared themes. It blends these
//! matches with the global popularity score loaded from `popularity.bin`,
//! returning a normalized value in `0.0..=1.0` via the `Scorer` trait.

#![forbid(unsafe_code)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use bincode::Options;
use camino::{Utf8Path, Utf8PathBuf};
use log::warn;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use thiserror::Error;
use wildside_core::{InterestProfile, PointOfInterest, Scorer, Theme};

use crate::{PopularityScores, bincode_options};

pub use crate::score_weights::ScoreWeights;

const CLAIM_LOOKUP_SQL: &str = concat!(
    "SELECT 1 FROM poi_wikidata_claims WHERE poi_id = ?1 AND property_id = ?2 ",
    "AND value_entity_id = ?3 LIMIT 1"
);
const DEFAULT_HISTORY_PROPERTY: &str = "P1435";
const DEFAULT_HISTORY_VALUE: &str = "Q9259";

/// Declarative mapping from a theme to one or more Wikidata property/value
/// pairs.
#[derive(Debug, Clone)]
pub struct ThemeClaimMapping {
    map: HashMap<Theme, Vec<ClaimSelector>>,
}

impl ThemeClaimMapping {
    /// Create an empty mapping.
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Insert a claim selector for the given theme.
    pub fn insert(&mut self, theme: Theme, selector: ClaimSelector) {
        self.map.entry(theme).or_default().push(selector);
    }

    /// Add a selector while consuming `self`, enabling chaining.
    #[must_use]
    pub fn with_selector(mut self, theme: Theme, selector: ClaimSelector) -> Self {
        self.insert(theme, selector);
        self
    }

    /// Retrieve selectors for a theme, if present (test-only helper).
    #[cfg(test)]
    fn selectors(&self, theme: &Theme) -> Option<&[ClaimSelector]> {
        self.map.get(theme).map(Vec::as_slice)
    }

    /// Iterate over all configured selectors grouped by theme.
    fn iter(&self) -> impl Iterator<Item = (&Theme, &[ClaimSelector])> {
        self.map
            .iter()
            .map(|(theme, selectors)| (theme, selectors.as_slice()))
    }
}

impl Default for ThemeClaimMapping {
    fn default() -> Self {
        let selector = ClaimSelector::new(DEFAULT_HISTORY_PROPERTY, DEFAULT_HISTORY_VALUE)
            .unwrap_or_else(|_| ClaimSelector {
                property_id: DEFAULT_HISTORY_PROPERTY.to_owned(),
                value_entity_id: DEFAULT_HISTORY_VALUE.to_owned(),
            });
        Self::new().with_selector(Theme::History, selector)
    }
}

/// Identify a Wikidata claim by property and value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClaimSelector {
    property_id: String,
    value_entity_id: String,
}

impl ClaimSelector {
    /// Build a selector from property and value identifiers.
    ///
    /// # Errors
    /// Returns [`UserRelevanceError::InvalidSelector`] when either identifier
    /// is empty or whitespace.
    pub fn new(
        property_id: impl Into<String>,
        value_entity_id: impl Into<String>,
    ) -> Result<Self, UserRelevanceError> {
        let property = property_id.into();
        let value = value_entity_id.into();
        if property.trim().is_empty() || value.trim().is_empty() {
            return Err(UserRelevanceError::InvalidSelector);
        }
        Ok(Self {
            property_id: property,
            value_entity_id: value,
        })
    }
}

/// Errors raised when initializing or configuring the user relevance scorer.
#[derive(Debug, Error)]
pub enum UserRelevanceError {
    /// Opening the `SQLite` database failed.
    #[error("failed to open read-only SQLite database at {path}")]
    OpenDatabase {
        /// Requested database path.
        path: Utf8PathBuf,
        /// Source error from `rusqlite`.
        #[source]
        source: rusqlite::Error,
    },
    /// Preparing the claim lookup statement failed.
    #[error("failed to prepare claim lookup statement")]
    PrepareStatement {
        /// Source error from `rusqlite`.
        #[source]
        source: rusqlite::Error,
    },
    /// Reading the popularity artefact failed.
    #[error("failed to read popularity file at {path}")]
    ReadPopularity {
        /// Path to the popularity artefact.
        path: Utf8PathBuf,
        /// Source error from std I/O.
        #[source]
        source: std::io::Error,
    },
    /// Decoding the popularity artefact failed.
    #[error("failed to decode popularity file at {path}")]
    DecodePopularity {
        /// Path to the popularity artefact.
        path: Utf8PathBuf,
        /// Source error from `bincode`.
        #[source]
        source: bincode::Error,
    },
    /// Provided weights were unusable.
    #[error("weights must be finite and sum to a positive value")]
    InvalidWeights,
    /// A claim selector was missing identifiers.
    #[error("claim selector must include non-empty property and value identifiers")]
    InvalidSelector,
}

/// Scorer that blends per-user interests with global popularity.
#[derive(Debug, Clone)]
pub struct UserRelevanceScorer {
    connection: Arc<Mutex<Connection>>,
    mapping: ThemeClaimMapping,
    weights: ScoreWeights,
    popularity: PopularityScores,
}

impl UserRelevanceScorer {
    /// Construct a scorer from artefact paths using default mapping and weights.
    ///
    /// # Errors
    /// Propagates filesystem, decoding, and database preparation failures.
    pub fn with_defaults(
        database_path: &Utf8Path,
        popularity_path: &Utf8Path,
    ) -> Result<Self, UserRelevanceError> {
        Self::from_paths(
            database_path,
            popularity_path,
            ThemeClaimMapping::default(),
            ScoreWeights::default(),
        )
    }

    /// Construct a scorer from artefact paths, mapping, and weights.
    ///
    /// # Errors
    /// Returns [`UserRelevanceError`] when artefacts are unreadable, the
    /// mapping is invalid, or `SQLite` refuses to prepare the lookup
    /// statement.
    pub fn from_paths(
        database_path: &Utf8Path,
        popularity_path: &Utf8Path,
        mapping: ThemeClaimMapping,
        weights: ScoreWeights,
    ) -> Result<Self, UserRelevanceError> {
        let validated_weights = weights.validate()?;
        let connection = Connection::open_with_flags(
            database_path.as_std_path(),
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|source| UserRelevanceError::OpenDatabase {
            path: database_path.to_path_buf(),
            source,
        })?;
        prepare_claim_statement(&connection)?;

        let bytes = wildside_fs::read_file(popularity_path.as_std_path()).map_err(|source| {
            UserRelevanceError::ReadPopularity {
                path: popularity_path.to_path_buf(),
                source,
            }
        })?;
        let popularity: PopularityScores =
            bincode_options().deserialize(&bytes).map_err(|source| {
                UserRelevanceError::DecodePopularity {
                    path: popularity_path.to_path_buf(),
                    source,
                }
            })?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            mapping,
            weights: validated_weights,
            popularity,
        })
    }

    #[expect(
        clippy::float_arithmetic,
        reason = "relevance scoring sums matching theme weights"
    )]
    fn user_relevance(&self, poi: &PointOfInterest, profile: &InterestProfile) -> f32 {
        let Ok(poi_id) = i64::try_from(poi.id) else {
            return 0.0;
        };
        let Ok(connection) = self.connection.lock() else {
            warn!("user relevance scoring skipped: SQLite connection lock was poisoned");
            return 0.0;
        };

        let Ok(mut statement) = connection.prepare_cached(CLAIM_LOOKUP_SQL) else {
            warn!("user relevance scoring skipped: failed to prepare claim lookup statement");
            return 0.0;
        };

        let mut relevance = 0.0_f32;
        for (theme, selectors) in self.mapping.iter() {
            let Some(weight) = profile.weight(theme) else {
                continue;
            };
            if weight <= 0.0_f32 || !weight.is_finite() {
                continue;
            }
            if selectors
                .iter()
                .any(|selector| claim_exists(&mut statement, poi_id, selector))
            {
                relevance += weight;
            }
        }

        <Self as Scorer>::sanitise(relevance)
    }
}

impl Scorer for UserRelevanceScorer {
    fn score(&self, poi: &PointOfInterest, profile: &InterestProfile) -> f32 {
        let popularity = <Self as Scorer>::sanitise(self.popularity.get(poi.id).unwrap_or(0.0_f32));
        let user_relevance = self.user_relevance(poi, profile);
        let blended = self.weights.blend(popularity, user_relevance);
        <Self as Scorer>::sanitise(blended)
    }
}

fn prepare_claim_statement(connection: &Connection) -> Result<(), UserRelevanceError> {
    connection
        .prepare_cached(CLAIM_LOOKUP_SQL)
        .map(|_| ())
        .map_err(|source| UserRelevanceError::PrepareStatement { source })
}

fn claim_exists(
    statement: &mut rusqlite::CachedStatement<'_>,
    poi_id: i64,
    selector: &ClaimSelector,
) -> bool {
    statement
        .query_row(
            (
                poi_id,
                selector.property_id.as_str(),
                selector.value_entity_id.as_str(),
            ),
            |_| Ok(()),
        )
        .optional()
        .map_or_else(
            |err| {
                warn!("claim query failed for POI {poi_id}: {err}");
                false
            },
            |row| row.is_some(),
        )
}

#[cfg(test)]
#[path = "user_tests.rs"]
mod tests;
