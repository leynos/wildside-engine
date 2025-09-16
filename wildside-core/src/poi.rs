//! Points of interest (POIs).
//!
//! Defines the `PointOfInterest` domain type and helpers.
//! Coordinates are WGS84 (`x = longitude`, `y = latitude`); tags mirror
//! OpenStreetMap keys.

use std::collections::HashMap;

use geo::Coord;

/// Map of tag key/value pairs (typically OSM-like).
pub type Tags = HashMap<String, String>;

/// A location worth visiting.
///
/// # Examples
/// ```rust
/// use geo::Coord;
/// use wildside_core::{PointOfInterest, Tags};
///
/// let poi = PointOfInterest::new(
///     1,
///     Coord { x: 1.0, y: 2.0 },
///     Tags::from([("name".into(), "Museum".into())]),
/// );
///
/// assert_eq!(poi.id, 1);
/// assert_eq!(poi.tags.get("name"), Some(&"Museum".to_string()));
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct PointOfInterest {
    /// Unique identifier for the POI.
    pub id: u64,
    /// Geographic location (WGS84; `x = longitude`, `y = latitude`).
    pub location: Coord<f64>,
    /// Free-form tags, e.g., from OpenStreetMap.
    pub tags: Tags,
}

impl PointOfInterest {
    /// Construct a `PointOfInterest` with the provided tags.
    ///
    /// # Examples
    /// ```rust
    /// use geo::Coord;
    /// use wildside_core::{PointOfInterest, Tags};
    ///
    /// let poi = PointOfInterest::new(1, Coord { x: 0.0, y: 0.0 }, Tags::new());
    /// assert_eq!(poi.id, 1);
    /// ```
    pub fn new(id: u64, location: Coord<f64>, tags: Tags) -> Self {
        Self { id, location, tags }
    }

    /// Construct a `PointOfInterest` without tags.
    ///
    /// # Examples
    /// ```rust
    /// use geo::Coord;
    /// use wildside_core::PointOfInterest;
    ///
    /// let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    /// assert!(poi.tags.is_empty());
    /// ```
    pub fn with_empty_tags(id: u64, location: Coord<f64>) -> Self {
        Self::new(id, location, Tags::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poi_stores_tags() {
        let poi = PointOfInterest::new(
            1,
            Coord { x: 0.0, y: 0.0 },
            Tags::from([("key".into(), "value".into())]),
        );
        assert_eq!(poi.tags.get("key"), Some(&"value".to_string()));
    }
}
