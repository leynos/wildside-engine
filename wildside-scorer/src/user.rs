//! User relevance scoring that combines personalised interests with
//! pre-computed popularity.
//!
//! The scorer inspects Wikidata claims stored in `pois.db` to determine whether
//! a point of interest matches the visitor's declared themes. It blends these
//! matches with the global popularity score loaded from `popularity.bin`,
//! returning a normalised value in `0.0..=1.0` via the `Scorer` trait.

#![forbid(unsafe_code)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use bincode::Options;
use camino::{Utf8Path, Utf8PathBuf};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use thiserror::Error;
use wildside_core::{InterestProfile, PointOfInterest, Scorer, Theme};

use crate::{PopularityScores, bincode_options};

const CLAIM_LOOKUP_SQL: &str = "SELECT 1 FROM poi_wikidata_claims WHERE poi_id = ?1 AND property_id = ?2\
     AND value_entity_id = ?3 LIMIT 1";
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
        let selector = ClaimSelector {
            property_id: DEFAULT_HISTORY_PROPERTY.to_owned(),
            value_entity_id: DEFAULT_HISTORY_VALUE.to_owned(),
        };
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

/// Relative weighting between global popularity and user relevance.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ScoreWeights {
    /// Multiplier applied to the global popularity component.
    pub popularity: f32,
    /// Multiplier applied to the user relevance component.
    pub user_relevance: f32,
}

impl ScoreWeights {
    /// Validate the weights and return a copy.
    ///
    /// # Errors
    /// Returns [`UserRelevanceError::InvalidWeights`] when either value is not
    /// finite or the total weight is zero.
    #[expect(
        clippy::float_arithmetic,
        reason = "validation requires a simple sum of weights"
    )]
    pub fn validate(self) -> Result<Self, UserRelevanceError> {
        let _ = self.popularity + self.user_relevance;
        if self.is_valid() {
            Ok(self)
        } else {
            Err(UserRelevanceError::InvalidWeights)
        }
    }

    const fn is_valid(self) -> bool {
        self.has_finite_values() && self.has_non_negative_values() && self.has_non_zero_total()
    }

    const fn has_finite_values(self) -> bool {
        self.popularity.is_finite() && self.user_relevance.is_finite()
    }

    const fn has_non_negative_values(self) -> bool {
        self.popularity >= 0.0_f32 && self.user_relevance >= 0.0_f32
    }

    #[expect(
        clippy::float_arithmetic,
        reason = "validation sums weights to ensure a non-zero total"
    )]
    const fn has_non_zero_total(self) -> bool {
        (self.popularity + self.user_relevance) != 0.0_f32
    }

    #[expect(
        clippy::float_arithmetic,
        reason = "score blending requires weighted averages"
    )]
    fn blend(self, popularity: f32, user_relevance: f32) -> f32 {
        let user_weight = if user_relevance > 0.0_f32 {
            self.user_relevance
        } else {
            0.0_f32
        };
        let total = self.popularity + user_weight;
        if total == 0.0 {
            return 0.0;
        }
        (popularity * self.popularity + user_relevance * user_weight) / total
    }
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            popularity: 0.5_f32,
            user_relevance: 0.5_f32,
        }
    }
}

/// Errors raised when initialising or configuring the user relevance scorer.
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

        let bytes = std::fs::read(popularity_path.as_std_path()).map_err(|source| {
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
            return 0.0;
        };

        let Ok(mut statement) = connection.prepare_cached(CLAIM_LOOKUP_SQL) else {
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
        .map(|row| row.is_some())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    //! Unit coverage for user relevance scoring.

    use std::collections::BTreeMap;

    use bincode::Options;
    use camino::Utf8PathBuf;
    use geo::Coord;
    use rstest::rstest;
    use rusqlite::Connection;
    use tempfile::TempDir;
    use wildside_core::{InterestProfile, PointOfInterest, Scorer, Theme};

    use super::{
        ClaimSelector, ScoreWeights, ThemeClaimMapping, UserRelevanceError, UserRelevanceScorer,
    };
    use crate::{PopularityScores, bincode_options};

    const TEST_PROPERTY: &str = "P999";
    const TEST_VALUE: &str = "Q_TEST_ART";

    #[rstest]
    fn defaults_include_history_mapping() {
        let mapping = ThemeClaimMapping::default();
        assert!(mapping.selectors(&Theme::History).is_some());
    }

    #[rstest]
    fn selector_rejects_empty_fields() {
        let err = ClaimSelector::new("", TEST_VALUE).expect_err("empty property should error");
        assert!(matches!(err, UserRelevanceError::InvalidSelector));
    }

    #[rstest]
    fn weights_reject_zero_total() {
        let err = ScoreWeights {
            popularity: 0.0,
            user_relevance: 0.0,
        }
        .validate()
        .expect_err("zero weights should be invalid");
        assert!(matches!(err, UserRelevanceError::InvalidWeights));
    }

    #[rstest]
    #[expect(
        clippy::float_arithmetic,
        reason = "tests compare floating point values"
    )]
    fn scoring_blends_popularity_and_interest() {
        let temp = TempDir::new().expect("tempdir");
        let db_path =
            Utf8PathBuf::from_path_buf(temp.path().join("pois.db")).expect("utf8 db path");
        seed_claims_database(&db_path);
        let popularity_path = write_popularity_fixture(&temp, 1, 0.25_f32);

        let mut mapping = ThemeClaimMapping::new();
        mapping.insert(
            Theme::Art,
            ClaimSelector::new(TEST_PROPERTY, TEST_VALUE).expect("valid selector"),
        );
        let scorer = UserRelevanceScorer::from_paths(
            &db_path,
            &popularity_path,
            mapping,
            ScoreWeights::default(),
        )
        .expect("construct scorer");

        let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
        let profile = InterestProfile::new().with_weight(Theme::Art, 0.8_f32);

        let score = scorer.score(&poi, &profile);

        let expected = f32::midpoint(0.25_f32, 0.8_f32);
        assert!(
            (score - expected).abs() < 0.000_1_f32,
            "score should blend components"
        );
    }

    #[rstest]
    #[expect(
        clippy::float_arithmetic,
        reason = "tests compare floating point values"
    )]
    fn non_matching_interest_yields_popularity_only() {
        let temp = TempDir::new().expect("tempdir");
        let db_path =
            Utf8PathBuf::from_path_buf(temp.path().join("pois.db")).expect("utf8 db path");
        seed_claims_database(&db_path);
        let popularity_path = write_popularity_fixture(&temp, 1, 0.6_f32);

        let scorer = UserRelevanceScorer::with_defaults(&db_path, &popularity_path)
            .expect("construct scorer with defaults");
        let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
        let profile = InterestProfile::new().with_weight(Theme::Art, 1.0_f32);

        let score = scorer.score(&poi, &profile);

        assert!(
            (score - 0.6_f32).abs() < 0.000_1_f32,
            "non matching interest falls back to popularity"
        );
    }

    #[rstest]
    #[expect(
        clippy::float_arithmetic,
        reason = "tests compare floating point values"
    )]
    fn missing_popularity_falls_back_to_interest() {
        let temp = TempDir::new().expect("tempdir");
        let db_path =
            Utf8PathBuf::from_path_buf(temp.path().join("pois.db")).expect("utf8 db path");
        seed_claims_database(&db_path);
        let popularity_path = write_popularity_fixture(&temp, 2, 0.0_f32);

        let mapping = ThemeClaimMapping::default();
        let scorer = UserRelevanceScorer::from_paths(
            &db_path,
            &popularity_path,
            mapping.clone(),
            ScoreWeights {
                popularity: 0.3_f32,
                user_relevance: 0.7_f32,
            },
        )
        .expect("construct scorer");
        let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
        let profile = InterestProfile::new().with_weight(Theme::History, 1.0_f32);

        let score = scorer.score(&poi, &profile);

        assert!(
            (score - 0.7_f32).abs() < 0.000_1_f32,
            "interest match should contribute even without popularity"
        );
    }

    fn seed_claims_database(path: &Utf8PathBuf) {
        let connection = Connection::open(path.as_std_path()).expect("open sqlite database");
        connection
            .execute(
                "CREATE TABLE poi_wikidata_links (poi_id INTEGER NOT NULL, entity_id TEXT NOT NULL)",
                [],
            )
            .expect("create links table");
        connection
            .execute(
                "CREATE TABLE wikidata_entity_claims (entity_id TEXT NOT NULL, property_id TEXT NOT NULL, value_entity_id TEXT NOT NULL)",
                [],
            )
            .expect("create claims table");
        connection
            .execute(
                "CREATE VIEW poi_wikidata_claims AS SELECT links.poi_id AS poi_id, claims.entity_id AS entity_id, claims.property_id AS property_id, claims.value_entity_id AS value_entity_id FROM poi_wikidata_links AS links JOIN wikidata_entity_claims AS claims ON claims.entity_id = links.entity_id",
                [],
            )
            .expect("create claims view");
        connection
            .execute(
                "INSERT INTO poi_wikidata_links (poi_id, entity_id) VALUES (1, 'Q_ART')",
                [],
            )
            .expect("insert link");
        connection
            .execute(
                "INSERT INTO wikidata_entity_claims (entity_id, property_id, value_entity_id) VALUES ('Q_ART', ?1, ?2)",
                (TEST_PROPERTY, TEST_VALUE),
            )
            .expect("insert claim");
        connection
            .execute(
                "INSERT INTO wikidata_entity_claims (entity_id, property_id, value_entity_id) VALUES ('Q_ART', 'P1435', 'Q9259')",
                [],
            )
            .expect("insert heritage claim");
    }

    fn write_popularity_fixture(dir: &TempDir, poi_id: u64, score: f32) -> Utf8PathBuf {
        let popularity = PopularityScores::new(BTreeMap::from([(poi_id, score)]));
        let path =
            Utf8PathBuf::from_path_buf(dir.path().join("popularity.bin")).expect("utf8 path");
        let bytes = bincode_options()
            .serialize(&popularity)
            .expect("serialise popularity");
        std::fs::write(path.as_std_path(), bytes).expect("write popularity fixture");
        path
    }
}
