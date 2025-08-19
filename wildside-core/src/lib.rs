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
    pub location: Coord,
    pub tags: HashMap<String, String>,
}

/// User interest weighting across themes.
///
/// # Examples
/// ```
/// use std::collections::HashMap;
/// use wildside_core::InterestProfile;
///
/// let mut weights = HashMap::new();
/// weights.insert("history".to_string(), 0.8);
/// let profile = InterestProfile { weights };
///
/// assert_eq!(profile.weights.get("history"), Some(&0.8));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct InterestProfile {
    pub weights: HashMap<String, f32>,
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
        let mut weights = HashMap::new();
        weights.insert("history".into(), 0.5);
        let profile = InterestProfile { weights };
        assert_eq!(profile.weights.get(theme).copied(), expected);
    }

    #[rstest]
    fn route_preserves_order() {
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
        let route = Route {
            pois: vec![poi1.clone(), poi2.clone()],
            total_duration: Duration::from_secs(120),
        };
        assert_eq!(route.pois[0], poi1);
        assert_eq!(route.pois[1], poi2);
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
