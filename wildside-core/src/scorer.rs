//! Score points of interest for a user profile.
//!
//! The `Scorer` trait assigns a relevance score to a
//! [`PointOfInterest`](crate::PointOfInterest) given a visitor's
//! [`InterestProfile`](crate::InterestProfile).

use crate::{InterestProfile, PointOfInterest};

/// Calculate a relevance score for a point of interest.
///
/// Higher scores indicate a better match between the POI and the
/// caller's interests. The method is infallible; implementers must return
/// `0.0` when no information is available.
///
/// # Examples
///
/// ```
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
pub trait Scorer {
    /// Return a score for `poi` according to `profile`.
    fn score(&self, poi: &PointOfInterest, profile: &InterestProfile) -> f32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Theme, poi::Tags};
    use geo::Coord;
    use rstest::rstest;
    use std::str::FromStr;

    struct TagScorer;

    impl Scorer for TagScorer {
        fn score(&self, poi: &PointOfInterest, profile: &InterestProfile) -> f32 {
            poi.tags
                .keys()
                .filter_map(|k| Theme::from_str(k).ok())
                .filter_map(|t| profile.weight(&t))
                .sum()
        }
    }

    fn make_poi(tag: &str) -> PointOfInterest {
        PointOfInterest::new(
            1,
            Coord { x: 0.0, y: 0.0 },
            Tags::from([(tag.to_string(), String::new())]),
        )
    }

    #[rstest]
    fn sums_matching_weights() {
        let mut profile = InterestProfile::new();
        profile.set_weight(Theme::Art, 0.7);
        let poi = make_poi("art");
        let scorer = TagScorer;
        assert_eq!(scorer.score(&poi, &profile), 0.7);
    }

    #[rstest]
    fn zero_when_no_match() {
        let mut profile = InterestProfile::new();
        profile.set_weight(Theme::Art, 0.7);
        let poi = make_poi("history");
        let scorer = TagScorer;
        assert_eq!(scorer.score(&poi, &profile), 0.0);
    }

    #[rstest]
    fn zero_for_empty_profile() {
        let profile = InterestProfile::new();
        let poi = make_poi("art");
        let scorer = TagScorer;
        assert_eq!(scorer.score(&poi, &profile), 0.0);
    }
}
