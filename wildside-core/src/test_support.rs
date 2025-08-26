//! Test-only, in-memory PoiStore implementation used by unit and behaviour
//! tests.

use geo::{Intersects, Rect};

use crate::{PoiStore, PointOfInterest};

/// In-memory `PoiStore` implementation used in tests.
///
/// The store performs a linear scan and is intended only for small datasets.
#[derive(Default, Debug)]
pub struct MemoryStore {
    pois: Vec<PointOfInterest>,
}

impl MemoryStore {
    /// Create a store containing a single point of interest.
    pub fn with_poi(poi: PointOfInterest) -> Self {
        Self { pois: vec![poi] }
    }

    /// Create a store from a collection of points of interest.
    pub fn with_pois(pois: Vec<PointOfInterest>) -> Self {
        Self { pois }
    }
}

impl PoiStore for MemoryStore {
    fn get_pois_in_bbox(
        &self,
        bbox: &Rect<f64>,
    ) -> Box<dyn Iterator<Item = PointOfInterest> + Send + '_> {
        let bbox = *bbox;
        Box::new(
            self.pois
                .iter()
                // `Intersects` treats boundary points as inside the rectangle.
                .filter(move |p| bbox.intersects(&p.location))
                .cloned(),
        )
    }
}
