//! Score points of interest for a user profile.
//!
//! The `Scorer` trait assigns a relevance score to a
//! [`PointOfInterest`](crate::PointOfInterest) given a visitor's
//! [`InterestProfile`](crate::InterestProfile).

use crate::{InterestProfile, PointOfInterest};

/// Calculate a relevance score for a point of interest.
///
/// Higher scores indicate a better match between the POI and the
/// caller's interests. Implementations must be thread-safe (`Send` + `Sync`)
/// so scorers can run across threads.
/// The method is infallible; implementers must return `0.0` when no
/// information is available.
///
/// Implementations must:
/// - Produce finite (`f32::is_finite`) scores.
/// - Return non-negative values.
/// - Normalise results to the range `0.0..=1.0`.
///
/// Use [`Scorer::sanitise`] to apply these guards.
///
/// # Examples
///
/// ```rust
/// use geo::Coord;
/// use wildside_core::{InterestProfile, PointOfInterest, Scorer};
///
/// struct UnitScorer;
///
/// impl Scorer for UnitScorer {
///     fn score(&self, _poi: &PointOfInterest, _profile: &InterestProfile) -> f32 {
///         1.0
///     }
/// }
///
/// let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
/// let profile = InterestProfile::new();
/// let scorer = UnitScorer;
/// assert_eq!(scorer.score(&poi, &profile), 1.0);
/// ```
pub trait Scorer: Send + Sync {
    /// Return a score for `poi` according to `profile`.
    fn score(&self, poi: &PointOfInterest, profile: &InterestProfile) -> f32;

    /// Clamp and validate a raw score.
    ///
    /// Returns `0.0` for non-finite values and clamps to `0.0..=1.0`.
    fn sanitise(score: f32) -> f32 {
        if !score.is_finite() {
            return 0.0;
        }
        score.clamp(0.0, 1.0)
    }
}
