//! Public configuration and output types for popularity scoring.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Tunable weights applied to raw popularity signals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopularityWeights {
    /// Multiplier applied to the sitelink count.
    pub sitelink_weight: f32,
    /// Additive bonus applied when a POI is a UNESCO World Heritage Site.
    pub heritage_bonus: f32,
}

impl Default for PopularityWeights {
    fn default() -> Self {
        Self {
            sitelink_weight: 1.0_f32,
            heritage_bonus: 25.0_f32,
        }
    }
}

/// Normalised popularity scores keyed by POI identifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PopularityScores {
    scores: BTreeMap<u64, f32>,
}

impl PopularityScores {
    /// Construct a new set of scores from a pre-computed map.
    #[expect(
        clippy::missing_const_for_fn,
        reason = "scores are produced at runtime from database reads"
    )]
    #[must_use]
    pub fn new(scores: BTreeMap<u64, f32>) -> Self {
        Self { scores }
    }

    /// Return the score for a POI, if present.
    #[must_use]
    pub fn get(&self, poi_id: u64) -> Option<f32> {
        self.scores.get(&poi_id).copied()
    }

    /// Return the number of scored POIs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.scores.len()
    }

    /// Report whether any scores are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.scores.is_empty()
    }

    /// Consume the wrapper and return the underlying map.
    #[must_use]
    pub fn into_inner(self) -> BTreeMap<u64, f32> {
        self.scores
    }
}
