//! Data access traits for points of interest.
//!
//! The `PoiStore` trait defines a read-only interface for retrieving
//! [`PointOfInterest`] values. Consumers can use it to query a set of POIs
//! within a geographic bounding box.

use geo::Rect;

use crate::PointOfInterest;

#[cfg(feature = "store-sqlite")]
mod spatial_index;
#[cfg(feature = "store-sqlite")]
mod sqlite;

#[cfg(feature = "store-sqlite")]
pub use spatial_index::{SpatialIndexError, SpatialIndexWriteError, write_spatial_index};
#[cfg(feature = "store-sqlite")]
pub use sqlite::{SqlitePoiStore, SqlitePoiStoreError};

/// Read-only access to persisted points of interest.
///
/// Implementers are expected to store POIs in a spatial index such as an
/// R\*-tree. The bounding box uses WGS84 coordinates (`x = longitude`,
/// `y = latitude`).
///
/// # Examples
///
/// ```rust
/// use geo::{Coord, Rect, Intersects};
/// use wildside_core::{PointOfInterest, PoiStore};
///
/// struct MemoryStore {
///     pois: Vec<PointOfInterest>,
/// }
///
/// impl PoiStore for MemoryStore {
///     fn get_pois_in_bbox(
///         &self,
///         bbox: &Rect<f64>,
///     ) -> Box<dyn Iterator<Item = PointOfInterest> + Send + '_> {
///         Box::new(
///             self.pois
///                 .iter()
///                 // `Intersects` treats boundary points as inside the rectangle.
///                 .filter(move |p| bbox.intersects(&p.location))
///                 .cloned(),
///         )
///     }
/// }
///
/// let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
/// let store = MemoryStore { pois: vec![poi.clone()] };
/// let bbox = Rect::new(Coord { x: -1.0, y: -1.0 }, Coord { x: 1.0, y: 1.0 });
///
/// let found: Vec<_> = store.get_pois_in_bbox(&bbox).collect();
/// assert_eq!(found, vec![poi]);
/// ```
pub trait PoiStore {
    /// Return all POIs that fall within the provided bounding box.
    ///
    /// Coordinates use WGS84 with axis order (longitude, latitude) in
    /// degrees. The rectangle is axis-aligned in lon/lat space and
    /// `Rect::new` normalises corners so that `min ≤ max` on both axes.
    ///
    /// Antimeridian note: this method does not model regions that cross the
    /// antimeridian. Callers that need such queries MUST split the area into
    /// two `Rect` ranges and invoke this method for each range.
    ///
    /// Containment includes boundary points.
    fn get_pois_in_bbox(
        &self,
        bbox: &Rect<f64>,
    ) -> Box<dyn Iterator<Item = PointOfInterest> + Send + '_>;
}

#[cfg(test)]
mod tests {
    use super::PoiStore;
    use crate::{PointOfInterest, test_support::MemoryStore};
    use geo::{Coord, Rect};
    use rstest::rstest;

    #[rstest]
    fn returns_pois_inside_bbox() {
        let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
        let store = MemoryStore::with_poi(poi.clone());
        let bbox = Rect::new(Coord { x: -1.0, y: -1.0 }, Coord { x: 1.0, y: 1.0 });
        let found: Vec<_> = store.get_pois_in_bbox(&bbox).collect();
        assert_eq!(found, vec![poi]);
    }

    #[rstest]
    fn returns_empty_when_no_pois() {
        let store = MemoryStore::default();
        let bbox = Rect::new(Coord { x: -1.0, y: -1.0 }, Coord { x: 1.0, y: 1.0 });
        assert_eq!(store.get_pois_in_bbox(&bbox).count(), 0);
    }

    #[rstest]
    #[case(Coord { x: -1.0, y: 0.0 })] // left edge
    #[case(Coord { x: 1.0, y: 0.0 })] // right edge
    #[case(Coord { x: 0.0, y: -1.0 })] // bottom edge
    #[case(Coord { x: 0.0, y: 1.0 })] // top edge
    #[case(Coord { x: -1.0, y: -1.0 })] // bottom-left corner
    #[case(Coord { x: 1.0, y: 1.0 })] // top-right corner
    fn includes_poi_on_bbox_boundary(#[case] location: Coord<f64>) {
        let poi = PointOfInterest::with_empty_tags(42, location);
        let store = MemoryStore::with_poi(poi.clone());
        let bbox = Rect::new(Coord { x: -1.0, y: -1.0 }, Coord { x: 1.0, y: 1.0 });
        let found: Vec<_> = store.get_pois_in_bbox(&bbox).collect();
        assert_eq!(found, vec![poi]);
    }

    #[rstest]
    #[case(Coord { x: -1.0000001, y: 0.0 })]
    #[case(Coord { x: 1.0000001, y: 0.0 })]
    #[case(Coord { x: 0.0, y: -1.0000001 })]
    #[case(Coord { x: 0.0, y: 1.0000001 })]
    fn excludes_poi_just_outside_bbox(#[case] location: Coord<f64>) {
        let poi = PointOfInterest::with_empty_tags(7, location);
        let store = MemoryStore::with_poi(poi);
        let bbox = Rect::new(Coord { x: -1.0, y: -1.0 }, Coord { x: 1.0, y: 1.0 });
        assert_eq!(store.get_pois_in_bbox(&bbox).count(), 0);
    }
}
