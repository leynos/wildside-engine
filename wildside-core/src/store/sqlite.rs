//! SQLite-backed store implementation for persisted POIs.

use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
};

use geo::{Coord, Rect};
use rstar::{AABB, RTree};
use rusqlite::{Connection, OpenFlags, params_from_iter};
use thiserror::Error;

use crate::PointOfInterest;

use super::PoiStore;
use super::spatial_index::{SpatialIndexError, load_index_entries};

/// SQLite limits bound parameters per statement to 999 by default. The store
/// chunks `IN` queries to remain below that ceiling.
const SQLITE_MAX_VARIABLE_NUMBER: usize = 999;

/// Error raised when reading or validating persisted POI artefacts.
#[derive(Debug, Error)]
pub enum SqlitePoiStoreError {
    /// Opening the SQLite database failed.
    #[error("failed to open SQLite database at {path}: {source}")]
    OpenDatabase {
        /// Location of the SQLite database on disk.
        path: PathBuf,
        /// Source error returned by `rusqlite`.
        #[source]
        source: rusqlite::Error,
    },
    /// Errors encountered while loading or validating the persisted R\*-tree.
    #[error(transparent)]
    SpatialIndex(#[from] SpatialIndexError),
    /// The SQLite database did not contain a POI referenced by the index.
    #[error("point of interest {id} listed in the index is missing from the database")]
    MissingPoi {
        /// Identifier of the missing POI.
        id: u64,
    },
    /// The stored tag payload was not valid JSON.
    #[error("failed to parse tags for POI {id}: {source}")]
    InvalidTags {
        /// Identifier of the POI whose tags failed to parse.
        id: u64,
        /// JSON decoding failure.
        #[source]
        source: serde_json::Error,
    },
    /// Generic SQLite error when reading POI rows.
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
}

/// Read-only POI store backed by SQLite metadata and a persisted R\*-tree.
pub struct SqlitePoiStore {
    index: RTree<PointOfInterest>,
}

impl fmt::Debug for SqlitePoiStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqlitePoiStore")
            .field("entries", &self.index.size())
            .finish_non_exhaustive()
    }
}

impl SqlitePoiStore {
    /// Open a store backed by the provided SQLite database and R\*-tree artefact.
    pub fn open<P, Q>(database_path: P, index_path: Q) -> Result<Self, SqlitePoiStoreError>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let database_path = database_path.as_ref();
        let index_path = index_path.as_ref();

        let connection =
            Connection::open_with_flags(database_path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(
                |source| SqlitePoiStoreError::OpenDatabase {
                    path: database_path.to_path_buf(),
                    source,
                },
            )?;

        let entries = load_index_entries(index_path)?;
        ensure_index_pois_exist(&connection, &entries)?;

        Ok(Self {
            index: RTree::bulk_load(entries),
        })
    }
}

impl PoiStore for SqlitePoiStore {
    fn get_pois_in_bbox(
        &self,
        bbox: &Rect<f64>,
    ) -> Box<dyn Iterator<Item = PointOfInterest> + Send + '_> {
        let envelope =
            AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);
        let mut pois: Vec<_> = self
            .index
            .locate_in_envelope_intersecting(&envelope)
            .cloned()
            .collect();

        pois.sort_unstable_by_key(|poi| poi.id);

        Box::new(pois.into_iter())
    }
}

fn find_missing_poi_in_chunk(chunk: &[u64], pois: &[PointOfInterest]) -> Option<u64> {
    if pois.len() == chunk.len() {
        return None;
    }

    for id in chunk {
        if pois.binary_search_by_key(id, |poi| poi.id).is_err() {
            return Some(*id);
        }
    }

    unreachable!("chunk length mismatch should reveal missing id");
}

fn ensure_index_pois_exist(
    connection: &Connection,
    entries: &[PointOfInterest],
) -> Result<(), SqlitePoiStoreError> {
    if entries.is_empty() {
        return Ok(());
    }

    let mut ids: Vec<u64> = entries.iter().map(|entry| entry.id).collect();
    ids.sort_unstable();
    ids.dedup();

    let max_parameters = max_variable_limit(connection);
    for chunk in ids.chunks(max_parameters) {
        let pois = load_pois_chunk(connection, chunk)?;
        if let Some(missing_id) = find_missing_poi_in_chunk(chunk, &pois) {
            return Err(SqlitePoiStoreError::MissingPoi { id: missing_id });
        }
    }

    Ok(())
}

fn max_variable_limit(connection: &Connection) -> usize {
    let _ = connection; // connection kept for symmetry with future tunables.
    SQLITE_MAX_VARIABLE_NUMBER
}

fn load_pois_chunk(
    connection: &Connection,
    ids: &[u64],
) -> Result<Vec<PointOfInterest>, SqlitePoiStoreError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = vec!["?"; ids.len()].join(", ");
    let query = format!("SELECT id, lon, lat, tags FROM pois WHERE id IN ({placeholders})");
    let mut statement = connection.prepare(&query)?;
    let mut rows = statement.query(params_from_iter(ids.iter()))?;
    let mut pois = Vec::new();

    while let Some(row) = rows.next()? {
        let id: u64 = row.get(0)?;
        let lon: f64 = row.get(1)?;
        let lat: f64 = row.get(2)?;
        let tags_json: String = row.get(3)?;
        let tags: HashMap<String, String> = serde_json::from_str(&tags_json)
            .map_err(|source| SqlitePoiStoreError::InvalidTags { id, source })?;

        let poi = PointOfInterest::new(id, Coord { x: lon, y: lat }, tags);
        pois.push(poi);
    }

    pois.sort_unstable_by_key(|poi| poi.id);

    Ok(pois)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tags;
    use crate::store::spatial_index::{SPATIAL_INDEX_MAGIC, SPATIAL_INDEX_VERSION};
    use crate::test_support::{write_sqlite_database, write_sqlite_spatial_index};
    use bincode::serialize_into;
    use geo::Coord;
    use rstest::{fixture, rstest};
    use std::{fs::File, io::Write, path::PathBuf};
    use tempfile::TempDir;

    fn poi(id: u64, x: f64, y: f64, name: &str) -> PointOfInterest {
        PointOfInterest::new(
            id,
            Coord { x, y },
            Tags::from([(String::from("name"), String::from(name))]),
        )
    }

    #[fixture]
    fn temp_artifacts() -> (TempDir, PathBuf, PathBuf) {
        let dir = TempDir::new().expect("create temp dir");
        let db_path = dir.path().join("pois.db");
        let index_path = dir.path().join("pois.rstar");
        (dir, db_path, index_path)
    }

    #[fixture]
    fn sample_pois() -> Vec<PointOfInterest> {
        vec![poi(1, 0.0, 0.0, "centre"), poi(2, 2.0, 2.0, "museum")]
    }

    #[fixture]
    fn sqlite_store_fixture(
        #[from(temp_artifacts)] (dir, db_path, index_path): (TempDir, PathBuf, PathBuf),
        sample_pois: Vec<PointOfInterest>,
    ) -> (TempDir, PathBuf, PathBuf, Vec<PointOfInterest>) {
        write_sqlite_database(&db_path, &sample_pois).expect("persist database");
        write_sqlite_spatial_index(&index_path, &sample_pois).expect("persist index");
        (dir, db_path, index_path, sample_pois)
    }

    #[rstest]
    fn sqlite_store_returns_pois_in_bbox(
        sqlite_store_fixture: (TempDir, PathBuf, PathBuf, Vec<PointOfInterest>),
    ) {
        let (_dir, db_path, index_path, pois) = sqlite_store_fixture;
        let store = SqlitePoiStore::open(&db_path, &index_path).expect("open store");
        let bbox = Rect::new(Coord { x: -0.5, y: -0.5 }, Coord { x: 0.5, y: 0.5 });
        let found: Vec<_> = store.get_pois_in_bbox(&bbox).collect();
        assert_eq!(found, vec![pois[0].clone()]);
    }

    #[rstest]
    fn sqlite_store_returns_sorted_results(
        #[from(temp_artifacts)] (_dir, db_path, index_path): (TempDir, PathBuf, PathBuf),
    ) {
        let pois = vec![
            poi(3, 3.0, 3.0, "gallery"),
            poi(1, 0.0, 0.0, "centre"),
            poi(2, 1.0, 1.0, "library"),
        ];
        write_sqlite_database(&db_path, &pois).expect("persist database");
        write_sqlite_spatial_index(&index_path, &pois).expect("persist index");

        let store = SqlitePoiStore::open(&db_path, &index_path).expect("open store");
        let bbox = Rect::new(Coord { x: -10.0, y: -10.0 }, Coord { x: 10.0, y: 10.0 });
        let found: Vec<_> = store.get_pois_in_bbox(&bbox).collect();

        let mut expected = pois;
        expected.sort_unstable_by_key(|poi| poi.id);
        assert_eq!(found, expected);
    }

    #[rstest]
    fn sqlite_store_returns_empty_outside_bbox(
        sqlite_store_fixture: (TempDir, PathBuf, PathBuf, Vec<PointOfInterest>),
    ) {
        let (_dir, db_path, index_path, _pois) = sqlite_store_fixture;
        let store = SqlitePoiStore::open(&db_path, &index_path).expect("open store");
        let bbox = Rect::new(Coord { x: 5.0, y: 5.0 }, Coord { x: 6.0, y: 6.0 });
        assert!(store.get_pois_in_bbox(&bbox).next().is_none());
    }

    #[rstest]
    fn sqlite_store_errors_when_index_has_unknown_poi(
        #[from(temp_artifacts)] (_dir, db_path, index_path): (TempDir, PathBuf, PathBuf),
        sample_pois: Vec<PointOfInterest>,
    ) {
        write_sqlite_database(&db_path, &sample_pois).expect("persist database");
        let mut pois = sample_pois;
        pois.push(poi(99, 9.0, 9.0, "ghost"));
        write_sqlite_spatial_index(&index_path, &pois).expect("persist index");

        let error =
            SqlitePoiStore::open(&db_path, &index_path).expect_err("missing POI should fail");
        assert!(matches!(error, SqlitePoiStoreError::MissingPoi { id: 99 }));
    }

    #[rstest]
    fn sqlite_store_errors_on_corrupted_magic(
        #[from(temp_artifacts)] (_dir, db_path, index_path): (TempDir, PathBuf, PathBuf),
        sample_pois: Vec<PointOfInterest>,
    ) {
        write_sqlite_database(&db_path, &sample_pois).expect("persist database");
        std::fs::write(&index_path, b"BAD!").expect("write corrupt file");

        let error =
            SqlitePoiStore::open(&db_path, &index_path).expect_err("invalid magic should fail");
        assert!(matches!(
            error,
            SqlitePoiStoreError::SpatialIndex(SpatialIndexError::InvalidMagic { .. })
        ));
    }

    #[rstest]
    fn sqlite_store_errors_on_unsupported_version(
        #[from(temp_artifacts)] (_dir, db_path, index_path): (TempDir, PathBuf, PathBuf),
        sample_pois: Vec<PointOfInterest>,
    ) {
        write_sqlite_database(&db_path, &sample_pois).expect("persist database");
        {
            let mut file = File::create(&index_path).expect("create index file");
            file.write_all(&SPATIAL_INDEX_MAGIC)
                .expect("write magic header");
            file.write_all(&(SPATIAL_INDEX_VERSION + 1).to_le_bytes())
                .expect("write version");
            serialize_into(&mut file, &Vec::<PointOfInterest>::new())
                .expect("write unsupported payload");
        }

        let error = SqlitePoiStore::open(&db_path, &index_path)
            .expect_err("unsupported version should fail");
        assert!(matches!(
            error,
            SqlitePoiStoreError::SpatialIndex(SpatialIndexError::UnsupportedVersion { found, supported })
                if found == SPATIAL_INDEX_VERSION + 1 && supported == SPATIAL_INDEX_VERSION
        ));
    }

    #[rstest]
    fn sqlite_store_errors_on_invalid_tags(
        #[from(temp_artifacts)] (_dir, db_path, index_path): (TempDir, PathBuf, PathBuf),
        sample_pois: Vec<PointOfInterest>,
    ) {
        write_sqlite_spatial_index(&index_path, &sample_pois).expect("persist index");
        let connection = Connection::open(&db_path).expect("create SQLite database");
        connection
            .execute(
                "CREATE TABLE pois (
                    id INTEGER PRIMARY KEY,
                    lon REAL NOT NULL,
                    lat REAL NOT NULL,
                    tags TEXT NOT NULL
                )",
                [],
            )
            .expect("create table");
        connection
            .execute(
                "INSERT INTO pois (id, lon, lat, tags) VALUES (1, 0.0, 0.0, 'not-json')",
                [],
            )
            .expect("insert row");

        let error =
            SqlitePoiStore::open(&db_path, &index_path).expect_err("invalid tags should fail");
        assert!(matches!(
            error,
            SqlitePoiStoreError::InvalidTags { id: 1, .. }
        ));
    }
}
