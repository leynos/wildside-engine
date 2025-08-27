//! Test-only, in-memory PoiStore implementation used by unit and behaviour
//! tests.

use geo::{Intersects, Rect};
use std::time::Duration;

use crate::{PoiStore, PointOfInterest, TravelTimeError, TravelTimeProvider};

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
        Self::with_pois(std::iter::once(poi))
    }

    /// Create a store from a collection of points of interest.
    pub fn with_pois<I>(pois: I) -> Self
    where
        I: IntoIterator<Item = PointOfInterest>,
    {
        Self {
            pois: pois.into_iter().collect(),
        }
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

/// Deterministic `TravelTimeProvider` returning one-second edges.
#[derive(Default, Debug)]
pub struct UnitTravelTimeProvider;

impl TravelTimeProvider for UnitTravelTimeProvider {
    fn get_travel_time_matrix(
        &self,
        pois: &[PointOfInterest],
    ) -> Result<Vec<Vec<Duration>>, TravelTimeError> {
        if pois.is_empty() {
            return Err(TravelTimeError::EmptyInput);
        }
        let n = pois.len();
        let mut matrix = vec![vec![Duration::ZERO; n]; n];
        for (i, row) in matrix.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                if i != j {
                    *cell = Duration::from_secs(1);
                }
            }
        }
        Ok(matrix)
    }
}
