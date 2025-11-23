//! Test-only utilities used by unit and behaviour tests:
//! - In-memory PoiStore (MemoryStore)
//! - Deterministic UnitTravelTimeProvider
//! - TagScorer for tag-based relevance scoring

use geo::{Intersects, Rect};
#[cfg(any(test, feature = "test-support"))]
use std::{path::Path, str::FromStr, time::Duration};

#[cfg(any(test, feature = "test-support"))]
use rusqlite::{Connection, Error as SqliteError};
#[cfg(any(test, feature = "test-support"))]
use serde_json::to_string;

use crate::{
    InterestProfile, PoiStore, PointOfInterest, TravelTimeError, TravelTimeMatrix,
    TravelTimeProvider,
};
#[cfg(any(test, feature = "test-support"))]
use crate::{Scorer, Theme, store::SpatialIndexWriteError, store::write_spatial_index};

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

/// Persist a SQLite database containing the provided POIs.
#[cfg(any(test, feature = "test-support"))]
pub fn write_sqlite_database(path: &Path, pois: &[PointOfInterest]) -> Result<(), rusqlite::Error> {
    let mut connection = Connection::open(path)?;
    let transaction = connection.transaction()?;
    transaction.execute(
        "CREATE TABLE pois (
            id INTEGER PRIMARY KEY,
            lon REAL NOT NULL,
            lat REAL NOT NULL,
            tags TEXT NOT NULL
        )",
        [],
    )?;
    {
        let mut statement =
            transaction.prepare("INSERT INTO pois (id, lon, lat, tags) VALUES (?1, ?2, ?3, ?4)")?;
        for poi in pois {
            let tags = to_string(&poi.tags)
                .map_err(|source| SqliteError::ToSqlConversionFailure(source.into()))?;
            statement.execute((poi.id, poi.location.x, poi.location.y, tags))?;
        }
    }
    transaction.commit()?;
    Ok(())
}

/// Write the persisted R\*-tree artefact for the provided POIs.
#[cfg(any(test, feature = "test-support"))]
pub fn write_sqlite_spatial_index(
    path: &Path,
    pois: &[PointOfInterest],
) -> Result<(), SpatialIndexWriteError> {
    write_spatial_index(path, pois)
}

/// Deterministic `TravelTimeProvider` returning one-second edges.
#[cfg(any(test, feature = "test-support"))]
#[cfg_attr(all(not(test), docsrs), doc(cfg(feature = "test-support")))]
#[derive(Default, Debug, Copy, Clone)]
pub struct UnitTravelTimeProvider;

#[cfg(any(test, feature = "test-support"))]
#[cfg_attr(all(not(test), docsrs), doc(cfg(feature = "test-support")))]
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
#[cfg(any(test, feature = "test-support"))]
#[cfg_attr(all(not(test), docsrs), doc(cfg(feature = "test-support")))]
#[derive(Debug, Copy, Clone, Default)]
pub struct TagScorer;

#[cfg(any(test, feature = "test-support"))]
impl Scorer for TagScorer {
    fn score(&self, poi: &PointOfInterest, profile: &InterestProfile) -> f32 {
        let sum: f32 = poi
            .tags
            .keys()
            .filter_map(|k| Theme::from_str(k).ok())
            .filter_map(|t| profile.weight(&t))
            .sum();
        <Self as Scorer>::sanitise(sum)
    }
}
