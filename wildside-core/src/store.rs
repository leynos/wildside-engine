//! Data access traits for points of interest.
//!
//! The `PoiStore` trait defines a read-only interface for retrieving
//! [`PointOfInterest`] values. Consumers can use it to query a set of POIs
//! within a geographic bounding box.

use geo::Rect;

use crate::PointOfInterest;

/// Read-only access to persisted points of interest.
///
/// Implementors are expected to store POIs in a spatial index such as an
/// R\*-tree. The bounding box uses WGS84 coordinates (`x = longitude`,
/// `y = latitude`).
///
/// # Examples
///
/// ```
/// use geo::{Coord, Rect, Contains};
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
///     ) -> Box<dyn Iterator<Item = PointOfInterest> + '_> {
///         Box::new(
///             self.pois
///                 .iter()
///                 .filter(move |p| bbox.contains(&p.location))
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
    /// Coordinates use WGS84 on the sphere with axis order (lon, lat) in
    /// degrees. Longitudes are normalised to [-180.0, 180.0). Latitude is
    /// [-90.0, 90.0].
    ///
    /// Dateline semantics:
    /// - If `min_lon <= max_lon`, the bbox is a single interval [min_lon,
    ///   max_lon].
    /// - If `min_lon > max_lon`, the bbox crosses the antimeridian and
    ///   represents [min_lon, 180.0) ∪ [-180.0, max_lon].
    ///
    /// Polar semantics:
    /// - Boxes that approach ±90° latitude should be treated as geodesic
    ///   regions, not planar rectangles. Implementations MUST use great-circle
    ///   predicates for containment/intersection.
    ///
    /// Implementors MAY internally:
    /// - Split dateline-crossing boxes into two ranges, OR
    /// - Use a spherical index (e.g., S2/H3) to compute a covering, then
    ///   refine.
    ///
    /// Helper functions are provided:
    /// - `Bbox::normalized()`
    /// - `Bbox::split_at_dateline() -> SmallVec<[Bbox; 2]>`
    /// - `Region::polar_cap(min_lat: f64)`
    ///
    /// All POI filters MUST respect these semantics.
    fn get_pois_in_bbox(&self, bbox: &Rect<f64>) -> Box<dyn Iterator<Item = PointOfInterest> + '_>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::MemoryStore;
    use geo::Coord;
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
}
