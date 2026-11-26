//! Relative weighting between global popularity and user relevance.

#![forbid(unsafe_code)]

use crate::UserRelevanceError;

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
    pub const fn validate(self) -> Result<Self, UserRelevanceError> {
        if self.is_valid() {
            Ok(self)
        } else {
            Err(UserRelevanceError::InvalidWeights)
        }
    }

    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "ScoreWeights is a tiny Copy type; pass-by-ref keeps the signature consistent"
    )]
    const fn is_valid(&self) -> bool {
        self.has_finite_values() && self.has_non_negative_values() && self.has_non_zero_total()
    }

    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "ScoreWeights is Copy; borrowing avoids repeated copies"
    )]
    const fn has_finite_values(&self) -> bool {
        self.popularity.is_finite() && self.user_relevance.is_finite()
    }

    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "ScoreWeights is Copy; borrowing avoids repeated copies"
    )]
    const fn has_non_negative_values(&self) -> bool {
        self.popularity >= 0.0_f32 && self.user_relevance >= 0.0_f32
    }

    #[expect(
        clippy::float_arithmetic,
        reason = "validation sums weights to ensure a non-zero total"
    )]
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "ScoreWeights is Copy; borrowing avoids repeated copies"
    )]
    const fn has_non_zero_total(&self) -> bool {
        (self.popularity + self.user_relevance) != 0.0_f32
    }

    #[expect(
        clippy::float_arithmetic,
        reason = "score blending requires weighted averages"
    )]
    pub(crate) fn blend(self, popularity: f32, user_relevance: f32) -> f32 {
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
