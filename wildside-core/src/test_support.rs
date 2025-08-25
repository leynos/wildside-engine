use geo::{Contains, Rect};

use crate::{PoiStore, PointOfInterest};

/// In-memory `PoiStore` implementation used in tests.
///
/// The store performs a linear scan and is intended only for small datasets.
#[derive(Default)]
pub struct MemoryStore {
    pub pois: Vec<PointOfInterest>,
}

impl MemoryStore {
    /// Create a store containing a single point of interest.
    pub fn with_poi(poi: PointOfInterest) -> Self {
        Self { pois: vec![poi] }
    }
}

impl PoiStore for MemoryStore {
    fn get_pois_in_bbox(&self, bbox: &Rect<f64>) -> Box<dyn Iterator<Item = PointOfInterest> + '_> {
        let bbox = *bbox;
        Box::new(
            self.pois
                .iter()
                .filter(move |p| bbox.contains(&p.location))
                .cloned(),
        )
    }
}
