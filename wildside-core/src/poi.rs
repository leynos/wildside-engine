//! Points of interest (POIs).
//!
//! Defines the `PointOfInterest` domain type and helpers.
//! Coordinates are WGS84 (`x = longitude`, `y = latitude`); tags mirror
//! OpenStreetMap keys.

use std::collections::HashMap;

use geo::Coord;
use rstar::{AABB, RTree, RTreeObject};

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

impl RTreeObject for PointOfInterest {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_point([self.location.x, self.location.y])
    }
}

/// Build an R\*-tree spatial index for the provided points of interest.
///
/// The returned tree owns cloned copies of the POIs and can be used for
/// efficient bounding-box or nearest-neighbour queries.
///
/// # Examples
/// ```rust
/// use geo::Coord;
/// use rstar::RTree;
/// use wildside_core::{build_spatial_index, PointOfInterest};
///
/// let pois = [
///     PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 }),
///     PointOfInterest::with_empty_tags(2, Coord { x: 1.0, y: 1.0 }),
/// ];
/// let tree: RTree<PointOfInterest> = build_spatial_index(&pois);
/// assert_eq!(tree.size(), 2);
/// ```
pub fn build_spatial_index(pois: &[PointOfInterest]) -> RTree<PointOfInterest> {
    RTree::bulk_load(pois.to_vec())
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

    use rstar::{AABB, RTree};
    use rstest::rstest;

    fn poi(id: u64, x: f64, y: f64) -> PointOfInterest {
        PointOfInterest::with_empty_tags(id, Coord { x, y })
    }

    #[rstest]
    fn poi_stores_tags() {
        let poi = PointOfInterest::new(
            1,
            Coord { x: 0.0, y: 0.0 },
            Tags::from([("key".into(), "value".into())]),
        );
        assert_eq!(poi.tags.get("key"), Some(&"value".to_string()));
    }

    #[rstest]
    fn spatial_index_contains_all_points() {
        let pois = [poi(1, 0.0, 0.0), poi(2, 1.0, 1.0)];

        let tree = build_spatial_index(&pois);

        assert_eq!(tree.size(), pois.len());
        for expected in pois {
            let envelope = AABB::from_point([expected.location.x, expected.location.y]);
            let mut matches = tree.locate_in_envelope_intersecting(&envelope);
            let found = matches.next().expect("point present in tree");
            assert_eq!(found, &expected);
            assert!(
                matches.next().is_none(),
                "envelope should match exactly one POI"
            );
        }
    }

    #[rstest]
    fn spatial_index_handles_empty_input() {
        let tree: RTree<PointOfInterest> = build_spatial_index(&[]);
        assert_eq!(tree.size(), 0);
    }
}
