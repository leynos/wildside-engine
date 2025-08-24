//! Interest profiles: per-theme user preference weights in `[0.0, 1.0]`.
//!
//! Provides an API to set, get, and chain theme weights. Prefer the
//! non-panicking `try_*` methods for validation in library code.

use std::collections::HashMap;
use thiserror::Error;

use crate::Theme;

/// User interest weighting across themes.
///
/// # Examples
/// ```
/// use wildside_core::{InterestProfile, Theme};
///
/// let profile = InterestProfile::new()
///     .with_weight(Theme::History, 0.8)
///     .with_weight(Theme::Art, 0.6);
/// assert_eq!(profile.weight(&Theme::History), Some(0.8));
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InterestProfile {
    weights: HashMap<Theme, f32>,
}

/// Errors from [`InterestProfile::try_set_weight`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WeightError {
    /// Weight is not within the `0.0..=1.0` range.
    #[error("weight must be within 0.0..=1.0")]
    OutOfRange,
    /// Weight is not finite.
    #[error("weight must be finite")]
    NonFinite,
}

impl InterestProfile {
    /// Construct an empty profile.
    ///
    /// # Examples
    /// ```
    /// use wildside_core::InterestProfile;
    ///
    /// let profile = InterestProfile::new();
    /// assert!(profile.weight(&wildside_core::Theme::Food).is_none());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the weight for a theme, if present.
    ///
    /// # Examples
    /// ```
    /// use wildside_core::{InterestProfile, Theme};
    ///
    /// let profile = InterestProfile::new().with_weight(Theme::Art, 0.5);
    /// assert_eq!(profile.weight(&Theme::Art), Some(0.5));
    /// assert!(profile.weight(&Theme::History).is_none());
    /// ```
    pub fn weight(&self, theme: &Theme) -> Option<f32> {
        self.weights.get(theme).copied()
    }

    /// Insert or update a theme weight.
    ///
    /// Delegates to [`Self::try_set_weight`] and panics on error.
    ///
    /// # Panics
    /// Panics if `weight` is outside `0.0..=1.0` or not finite (NaN/∞).
    ///
    /// # Examples
    /// ```
    /// use wildside_core::{InterestProfile, Theme};
    ///
    /// let mut profile = InterestProfile::new();
    /// profile.set_weight(Theme::Shopping, 0.7);
    /// assert_eq!(profile.weight(&Theme::Shopping), Some(0.7));
    /// ```
    pub fn set_weight(&mut self, theme: Theme, weight: f32) {
        self.try_set_weight(theme, weight)
            .expect("weight must be finite and within 0.0..=1.0");
    }

    /// Validate and set a theme weight.
    ///
    /// # Errors
    /// Returns [`WeightError::OutOfRange`] if `weight` is outside
    /// `0.0..=1.0`.
    /// Returns [`WeightError::NonFinite`] if `weight` is `NaN` or infinite.
    pub fn try_set_weight(&mut self, theme: Theme, weight: f32) -> Result<(), WeightError> {
        if !weight.is_finite() {
            return Err(WeightError::NonFinite);
        }
        if !(0.0..=1.0).contains(&weight) {
            return Err(WeightError::OutOfRange);
        }
        self.weights.insert(theme, weight);
        Ok(())
    }

    /// Add a theme weight while returning `self` for chaining.
    ///
    /// # Panics
    /// Panics if `weight` is outside `0.0..=1.0` or not finite (NaN/∞).
    ///
    /// # Examples
    /// ```
    /// use wildside_core::{InterestProfile, Theme};
    ///
    /// let profile = InterestProfile::new().with_weight(Theme::History, 0.8);
    /// assert_eq!(profile.weight(&Theme::History), Some(0.8));
    /// ```
    #[must_use]
    pub fn with_weight(mut self, theme: Theme, weight: f32) -> Self {
        self.set_weight(theme, weight);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interest_lookup() {
        let profile = InterestProfile::new().with_weight(Theme::History, 0.5);
        assert_eq!(profile.weight(&Theme::History), Some(0.5));
        assert!(profile.weight(&Theme::Art).is_none());
    }

    #[test]
    fn multiple_theme_lookup() {
        let mut profile = InterestProfile::new();
        profile.set_weight(Theme::Food, 0.8);
        profile.set_weight(Theme::Nature, 0.5);
        profile.set_weight(Theme::Art, 0.3);

        assert_eq!(profile.weight(&Theme::Food), Some(0.8));
        assert_eq!(profile.weight(&Theme::Nature), Some(0.5));
        assert_eq!(profile.weight(&Theme::Art), Some(0.3));
        assert!(profile.weight(&Theme::Shopping).is_none());
    }

    #[test]
    fn empty_profile_returns_none() {
        let profile = InterestProfile::new();
        assert!(profile.weight(&Theme::Nature).is_none());
    }

    #[test]
    fn try_set_weight_rejects_out_of_range() {
        let mut profile = InterestProfile::new();
        assert_eq!(
            profile.try_set_weight(Theme::History, 1.2),
            Err(WeightError::OutOfRange)
        );
        assert_eq!(
            profile.try_set_weight(Theme::Art, -0.5),
            Err(WeightError::OutOfRange)
        );
    }

    #[test]
    fn try_set_weight_rejects_non_finite() {
        let mut profile = InterestProfile::new();
        assert_eq!(
            profile.try_set_weight(Theme::History, f32::NAN),
            Err(WeightError::NonFinite)
        );
        assert_eq!(
            profile.try_set_weight(Theme::Art, f32::INFINITY),
            Err(WeightError::NonFinite)
        );
        assert_eq!(
            profile.try_set_weight(Theme::Food, f32::NEG_INFINITY),
            Err(WeightError::NonFinite)
        );
    }

    #[test]
    #[should_panic(expected = "finite")]
    fn set_weight_panics_on_non_finite() {
        let mut profile = InterestProfile::new();
        profile.set_weight(Theme::Nature, f32::NAN);
    }

    #[test]
    #[should_panic(expected = "0.0..=1.0")]
    fn set_weight_panics_on_out_of_range() {
        let mut profile = InterestProfile::new();
        profile.set_weight(Theme::History, 1.5);
    }
}
