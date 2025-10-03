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

/// Enable spatial indexing by representing POIs as zero-dimensional points.
impl RTreeObject for PointOfInterest {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_point([self.location.x, self.location.y])
    }
}

/// A spatial index for locating [`PointOfInterest`] values.
#[derive(Clone, Debug)]
pub struct SpatialIndex {
    tree: RTree<PointOfInterest>,
}

impl SpatialIndex {
    /// Return the number of indexed points.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tree.size()
    }

    /// Report whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate over all indexed points.
    pub fn iter(&self) -> impl Iterator<Item = &PointOfInterest> {
        self.tree.iter()
    }

    /// Query the index for points intersecting the provided bounding box.
    ///
    /// The bounding box is normalised by taking the minimum and maximum of the
    /// provided coordinates, so the order of the arguments does not affect the
    /// result.
    #[must_use]
    pub fn query_within(&self, minimum: Coord<f64>, maximum: Coord<f64>) -> Vec<PointOfInterest> {
        let lower = Coord {
            x: minimum.x.min(maximum.x),
            y: minimum.y.min(maximum.y),
        };
        let upper = Coord {
            x: minimum.x.max(maximum.x),
            y: minimum.y.max(maximum.y),
        };
        let envelope = AABB::from_corners([lower.x, lower.y], [upper.x, upper.y]);
        self.tree
            .locate_in_envelope_intersecting(&envelope)
            .cloned()
            .collect()
    }
}

/// Build an R\*-tree spatial index for the provided points of interest.
///
/// The returned index owns the provided POIs and supports efficient
/// bounding-box or nearest-neighbour queries.
///
/// # Examples
/// ```rust
/// use geo::Coord;
/// use wildside_core::{build_spatial_index, PointOfInterest};
///
/// let pois = vec![
///     PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 }),
///     PointOfInterest::with_empty_tags(2, Coord { x: 1.0, y: 1.0 }),
/// ];
/// let index = build_spatial_index(pois);
/// assert_eq!(index.len(), 2);
/// ```
pub fn build_spatial_index<I>(pois: I) -> SpatialIndex
where
    I: IntoIterator<Item = PointOfInterest>,
{
    let tree = RTree::bulk_load(pois.into_iter().collect());
    SpatialIndex { tree }
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
    // Integration tests cover the `PointOfInterest` module. See
    // `tests/spatial_index.rs` for coverage of constructors and spatial
    // indexing behaviour.
}
