//! Interest profiles: per-theme user preference weights in `[0.0, 1.0]`.
//!
//! Provides an API to set, get, and chain theme weights. Prefer the
//! non-panicking `try_*` methods (to be added) for validation in library
//! code.

use std::collections::HashMap;

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
    /// Values are clamped into `0.0..=1.0`.
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
        let clamped = weight.clamp(0.0, 1.0);
        self.weights.insert(theme, clamped);
    }

    /// Add a theme weight while returning `self` for chaining.
    ///
    /// # Examples
    /// ```
    /// use wildside_core::{InterestProfile, Theme};
    ///
    /// let profile = InterestProfile::new().with_weight(Theme::History, 0.8);
    /// assert_eq!(profile.weight(&Theme::History), Some(0.8));
    /// ```
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
    fn set_weight_clamps_out_of_range() {
        let mut profile = InterestProfile::new();
        profile.set_weight(Theme::History, 1.2);
        assert_eq!(profile.weight(&Theme::History), Some(1.0));
        profile.set_weight(Theme::Art, -0.5);
        assert_eq!(profile.weight(&Theme::Art), Some(0.0));
    }
}
