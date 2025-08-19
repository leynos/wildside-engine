//! Core domain types for the Wildside engine.

use std::collections::HashMap;
use std::time::Duration;

use geo::Coord;

/// A location worth visiting.
///
/// Tags mirror OpenStreetMap's free-form key/value structure.
///
/// # Examples
/// ```
/// use std::collections::HashMap;
/// use geo::Coord;
/// use wildside_core::PointOfInterest;
///
/// let location = Coord { x: 1.0, y: 2.0 };
/// let mut tags = HashMap::new();
/// tags.insert("name".into(), "Museum".into());
/// let poi = PointOfInterest { id: 1, location, tags };
///
/// assert_eq!(poi.id, 1);
/// assert_eq!(poi.tags.get("name"), Some(&"Museum".to_string()));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PointOfInterest {
    pub id: u64,
    pub location: Coord<f64>,
    pub tags: HashMap<String, String>,
}

/// User interest weighting across themes.
///
/// # Examples
/// ```
/// use std::collections::HashMap;
/// use wildside_core::InterestProfile;
///
/// let profile =
///     InterestProfile::new(HashMap::from([("history".to_string(), 0.8)]));
/// assert_eq!(profile.weight("history"), Some(0.8));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct InterestProfile {
    weights: HashMap<String, f32>,
}

impl InterestProfile {
    /// Construct a profile from the provided weights.
    ///
    /// # Examples
    /// ```
    /// use std::collections::HashMap;
    /// use wildside_core::InterestProfile;
    ///
    /// let profile =
    ///     InterestProfile::new(HashMap::from([("art".to_string(), 0.5)]));
    /// assert_eq!(profile.weight("art"), Some(0.5));
    /// ```
    pub fn new(weights: HashMap<String, f32>) -> Self {
        Self { weights }
    }

    /// Return the weight for a theme, if present.
    ///
    /// # Examples
    /// ```
    /// use std::collections::HashMap;
    /// use wildside_core::InterestProfile;
    ///
    /// let profile =
    ///     InterestProfile::new(HashMap::from([("art".to_string(), 0.5)]));
    /// assert_eq!(profile.weight("art"), Some(0.5));
    /// assert!(profile.weight("history").is_none());
    /// ```
    pub fn weight(&self, theme: &str) -> Option<f32> {
        self.weights.get(theme).copied()
    }

    /// Insert or update a theme weight.
    ///
    /// # Examples
    /// ```
    /// use std::collections::HashMap;
    /// use wildside_core::InterestProfile;
    ///
    /// let mut profile = InterestProfile::new(HashMap::new());
    /// profile.set_weight("music".into(), 0.7);
    /// assert_eq!(profile.weight("music"), Some(0.7));
    /// ```
    pub fn set_weight(&mut self, theme: String, weight: f32) {
        self.weights.insert(theme, weight);
    }
}

/// An ordered path through points of interest with an overall duration.
///
/// # Examples
/// ```
/// use std::collections::HashMap;
/// use geo::Coord;
/// use std::time::Duration;
/// use wildside_core::{PointOfInterest, Route};
///
/// let poi = PointOfInterest { id: 1, location: Coord { x: 0.0, y: 0.0 }, tags: HashMap::new() };
/// let route = Route { pois: vec![poi], total_duration: Duration::from_secs(60) };
///
/// assert_eq!(route.pois.len(), 1);
/// assert_eq!(route.total_duration.as_secs(), 60);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub pois: Vec<PointOfInterest>,
    pub total_duration: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    fn poi_stores_tags() {
        let mut tags = HashMap::new();
        tags.insert("key".into(), "value".into());
        let poi = PointOfInterest {
            id: 1,
            location: Coord { x: 0.0, y: 0.0 },
            tags,
        };
        assert_eq!(poi.tags.get("key"), Some(&"value".to_string()));
    }

    #[rstest]
    #[case("history", Some(0.5))]
    #[case("art", None)]
    fn interest_lookup(#[case] theme: &str, #[case] expected: Option<f32>) {
        let profile = InterestProfile::new(HashMap::from([("history".to_string(), 0.5)]));
        assert_eq!(profile.weight(theme), expected);
    }

    #[rstest]
    fn multiple_theme_lookup() {
        let mut profile = InterestProfile::new(HashMap::new());
        profile.set_weight("sports".into(), 0.8);
        profile.set_weight("music".into(), 0.5);
        profile.set_weight("art".into(), 0.3);

        assert_eq!(profile.weight("sports"), Some(0.8));
        assert_eq!(profile.weight("music"), Some(0.5));
        assert_eq!(profile.weight("art"), Some(0.3));
        assert!(profile.weight("science").is_none());
    }

    #[rstest]
    fn empty_profile_returns_none() {
        let profile = InterestProfile::new(HashMap::new());
        assert!(profile.weight("nature").is_none());
    }

    #[fixture]
    fn two_pois() -> (PointOfInterest, PointOfInterest) {
        let poi1 = PointOfInterest {
            id: 1,
            location: Coord { x: 0.0, y: 0.0 },
            tags: HashMap::new(),
        };
        let poi2 = PointOfInterest {
            id: 2,
            location: Coord { x: 1.0, y: 1.0 },
            tags: HashMap::new(),
        };
        (poi1, poi2)
    }

    #[rstest]
    fn route_preserves_order(two_pois: (PointOfInterest, PointOfInterest)) {
        let (poi1, poi2) = two_pois;
        let route = Route {
            pois: vec![poi1.clone(), poi2.clone()],
            total_duration: Duration::from_secs(120),
        };
        assert_eq!(route.pois, vec![poi1, poi2]);
        assert_eq!(route.total_duration.as_secs(), 120);
    }

    #[rstest]
    fn empty_route_has_zero_duration() {
        let route = Route {
            pois: Vec::new(),
            total_duration: Duration::from_secs(0),
        };
        assert!(route.pois.is_empty());
        assert_eq!(route.total_duration.as_secs(), 0);
    }
}
