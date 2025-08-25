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
///     fn get_pois_in_bbox(&self, bbox: &Rect<f64>) -> Vec<PointOfInterest> {
///         self.pois
///             .iter()
///             .filter(|p| bbox.contains(&p.location))
///             .cloned()
///             .collect()
///     }
/// }
///
/// let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
/// let store = MemoryStore { pois: vec![poi.clone()] };
/// let bbox = Rect::new(Coord { x: -1.0, y: -1.0 }, Coord { x: 1.0, y: 1.0 });
///
/// assert_eq!(store.get_pois_in_bbox(&bbox), vec![poi]);
/// ```
pub trait PoiStore {
    /// Return all POIs that fall within the provided bounding box.
    fn get_pois_in_bbox(&self, bbox: &Rect<f64>) -> Vec<PointOfInterest>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{Contains, Coord};
    use rstest::rstest;

    struct MemoryStore {
        pois: Vec<PointOfInterest>,
    }

    impl PoiStore for MemoryStore {
        fn get_pois_in_bbox(&self, bbox: &Rect<f64>) -> Vec<PointOfInterest> {
            self.pois
                .iter()
                .filter(|p| bbox.contains(&p.location))
                .cloned()
                .collect()
        }
    }

    #[rstest]
    fn returns_pois_inside_bbox() {
        let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
        let store = MemoryStore {
            pois: vec![poi.clone()],
        };
        let bbox = Rect::new(Coord { x: -1.0, y: -1.0 }, Coord { x: 1.0, y: 1.0 });
        assert_eq!(store.get_pois_in_bbox(&bbox), vec![poi]);
    }

    #[rstest]
    fn returns_empty_when_no_pois() {
        let store = MemoryStore { pois: vec![] };
        let bbox = Rect::new(Coord { x: -1.0, y: -1.0 }, Coord { x: 1.0, y: 1.0 });
        assert!(store.get_pois_in_bbox(&bbox).is_empty());
    }
}
