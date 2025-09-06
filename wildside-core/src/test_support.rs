//! Test-only, in-memory `PoiStore` implementation used by unit and behaviour
//! tests.

use geo::{Intersects, Rect};
use std::str::FromStr;
use std::time::Duration;

use crate::{
    InterestProfile, PoiStore, PointOfInterest, Scorer, Theme, TravelTimeError, TravelTimeMatrix,
    TravelTimeProvider,
};

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
#[cfg(any(test, feature = "test-support"))]
#[cfg_attr(not(test), doc(cfg(feature = "test-support")))]
#[derive(Default, Debug, Copy, Clone)]
pub struct UnitTravelTimeProvider;

#[cfg(any(test, feature = "test-support"))]
#[cfg_attr(not(test), doc(cfg(feature = "test-support")))]
impl TravelTimeProvider for UnitTravelTimeProvider {
    fn get_travel_time_matrix(
        &self,
        pois: &[PointOfInterest],
    ) -> Result<TravelTimeMatrix, TravelTimeError> {
        if pois.is_empty() {
            return Err(TravelTimeError::EmptyInput);
        }
        let n = pois.len();
        let mut matrix = vec![vec![Duration::from_secs(1); n]; n];
        for (i, row) in matrix.iter_mut().enumerate() {
            row[i] = Duration::ZERO;
        }
        Ok(matrix)
    }
}

/// Test `Scorer` that sums profile weights for matching tags.
#[derive(Debug, Copy, Clone, Default)]
pub struct TagScorer;

impl Scorer for TagScorer {
    fn score(&self, poi: &PointOfInterest, profile: &InterestProfile) -> f32 {
        poi.tags
            .keys()
            .filter_map(|k| Theme::from_str(k).ok())
            .filter_map(|t| profile.weight(&t))
            .sum()
    }
}
