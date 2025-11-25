//! Data access traits for points of interest.
//!
//! The `PoiStore` trait defines a read-only interface for retrieving
//! [`PointOfInterest`] values. Consumers can use it to query a set of POIs
//! within a geographic bounding box.

use std::{
    collections::HashMap,
    fmt,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use bincode::{deserialize_from, serialize_into};
use geo::{Coord, Rect};
use rstar::{AABB, RTree};
use rusqlite::{Connection, OpenFlags, params_from_iter};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::PointOfInterest;

/// File identifier for persisted spatial indices.
pub(crate) const SPATIAL_INDEX_MAGIC: [u8; 4] = *b"WSPI";

/// Supported version of the persisted spatial index format.
pub(crate) const SPATIAL_INDEX_VERSION: u16 = 2;

/// SQLite limits bound parameters per statement to 999 by default. The store
/// chunks `IN` queries to remain below that ceiling.
const SQLITE_MAX_VARIABLE_NUMBER: usize = 999;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpatialIndexFile {
    magic: [u8; 4],
    version: u16,
    entries: Vec<PointOfInterest>,
}

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

/// Error emitted when loading or validating the persisted spatial index.
#[derive(Debug, Error)]
pub enum SpatialIndexError {
    /// The index file could not be read from disk.
    #[error("failed to read spatial index from {path}: {source}")]
    Io {
        /// Location of the persisted R\*-tree artefact.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The serialised R\*-tree could not be decoded.
    #[error("failed to decode spatial index from {path}: {source}")]
    Decode {
        /// Location of the persisted R\*-tree artefact.
        path: PathBuf,
        /// Decoder error returned by `bincode`.
        #[source]
        source: bincode::Error,
    },
    /// The file did not contain the expected header.
    #[error("invalid spatial index magic: expected {expected:?}, found {found:?}")]
    InvalidMagic {
        /// Expected byte sequence identifying a spatial index file.
        expected: [u8; 4],
        /// Sequence read from the file.
        found: [u8; 4],
    },
    /// The reader encountered an unsupported format version.
    #[error("unsupported spatial index version {found}; supported version is {supported}")]
    UnsupportedVersion {
        /// Version present in the file header.
        found: u16,
        /// Latest version supported by this binary.
        supported: u16,
    },
}

/// Error emitted when serialising a spatial index to disk.
#[derive(Debug, Error)]
pub enum SpatialIndexWriteError {
    /// Writing bytes to disk failed.
    #[error("failed to write spatial index to {path}: {source}")]
    Io {
        /// Destination file path.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The in-memory representation could not be encoded.
    #[error("failed to encode spatial index for {path}: {source}")]
    Encode {
        /// Destination file path.
        path: PathBuf,
        /// Encoder failure from `bincode`.
        #[source]
        source: bincode::Error,
    },
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
        let database_path = database_path.as_ref().to_path_buf();
        let index_path = index_path.as_ref().to_path_buf();

        let connection =
            Connection::open_with_flags(&database_path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(
                |source| SqlitePoiStoreError::OpenDatabase {
                    path: database_path.clone(),
                    source,
                },
            )?;

        let entries = load_index_entries(&index_path)?;
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

    debug_assert!(false, "chunk length mismatch should reveal missing id");

    None
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

fn load_index_entries(path: &Path) -> Result<Vec<PointOfInterest>, SpatialIndexError> {
    let mut file = File::open(path).map_err(|source| SpatialIndexError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    let mut magic = [0_u8; 4];
    file.read_exact(&mut magic)
        .map_err(|source| SpatialIndexError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    if magic != SPATIAL_INDEX_MAGIC {
        return Err(SpatialIndexError::InvalidMagic {
            expected: SPATIAL_INDEX_MAGIC,
            found: magic,
        });
    }

    let mut version_bytes = [0_u8; 2];
    file.read_exact(&mut version_bytes)
        .map_err(|source| SpatialIndexError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let version = u16::from_le_bytes(version_bytes);
    if version != SPATIAL_INDEX_VERSION {
        return Err(SpatialIndexError::UnsupportedVersion {
            found: version,
            supported: SPATIAL_INDEX_VERSION,
        });
    }

    deserialize_from(&mut file).map_err(|source| SpatialIndexError::Decode {
        path: path.to_path_buf(),
        source,
    })
}

/// Persist a spatial index artefact containing the provided POIs.
///
/// The file is written in the `WSPI` binary format expected by
/// [`SqlitePoiStore`]. It combines a fixed header with a `bincode` payload of
/// [`PointOfInterest`] entries. Existing files are truncated.
pub fn write_spatial_index(
    path: &Path,
    entries: &[PointOfInterest],
) -> Result<(), SpatialIndexWriteError> {
    write_index(path, entries)
}

pub(crate) fn write_index(
    path: &Path,
    entries: &[PointOfInterest],
) -> Result<(), SpatialIndexWriteError> {
    let mut file = File::create(path).map_err(|source| SpatialIndexWriteError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let payload = SpatialIndexFile {
        magic: SPATIAL_INDEX_MAGIC,
        version: SPATIAL_INDEX_VERSION,
        entries: entries.to_vec(),
    };
    serialize_into(&mut file, &payload).map_err(|source| SpatialIndexWriteError::Encode {
        path: path.to_path_buf(),
        source,
    })?;
    file.sync_all()
        .map_err(|source| SpatialIndexWriteError::Io {
            path: path.to_path_buf(),
            source,
        })
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
    use super::*;
    use crate::{
        Tags,
        test_support::{MemoryStore, write_sqlite_database, write_sqlite_spatial_index},
    };
    use bincode::{deserialize_from, serialize_into};
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
            let payload = SpatialIndexFile {
                magic: SPATIAL_INDEX_MAGIC,
                version: SPATIAL_INDEX_VERSION + 1,
                entries: Vec::new(),
            };
            serialize_into(&mut file, &payload).expect("write unsupported payload");
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

    #[rstest]
    fn load_index_entries_round_trips_entries(
        #[from(temp_artifacts)] (_dir, _db_path, index_path): (TempDir, PathBuf, PathBuf),
        sample_pois: Vec<PointOfInterest>,
    ) {
        write_index(&index_path, &sample_pois).expect("persist index");

        let loaded = load_index_entries(&index_path).expect("load index");
        assert_eq!(loaded, sample_pois);
    }

    #[rstest]
    fn load_index_entries_returns_io_error_for_missing_file() {
        let missing_path = PathBuf::from("/non-existent/index-file");
        let error = load_index_entries(&missing_path).expect_err("missing file should error");
        assert!(matches!(error, SpatialIndexError::Io { .. }));
    }

    #[rstest]
    fn load_index_entries_errors_on_invalid_magic(
        #[from(temp_artifacts)] (_dir, _db_path, index_path): (TempDir, PathBuf, PathBuf),
    ) {
        std::fs::write(&index_path, b"BAD!").expect("write corrupt header");

        let error = load_index_entries(&index_path).expect_err("invalid magic should fail");
        assert!(matches!(error, SpatialIndexError::InvalidMagic { .. }));
    }

    #[rstest]
    fn load_index_entries_errors_on_decode_failure(
        #[from(temp_artifacts)] (_dir, _db_path, index_path): (TempDir, PathBuf, PathBuf),
    ) {
        let mut file = File::create(&index_path).expect("create index file");
        file.write_all(&SPATIAL_INDEX_MAGIC)
            .expect("write magic header");
        file.write_all(&SPATIAL_INDEX_VERSION.to_le_bytes())
            .expect("write version");
        drop(file);

        let error = load_index_entries(&index_path).expect_err("decode should fail");
        assert!(matches!(error, SpatialIndexError::Decode { .. }));
    }

    #[rstest]
    fn load_index_entries_errors_on_unsupported_version(
        #[from(temp_artifacts)] (_dir, _db_path, index_path): (TempDir, PathBuf, PathBuf),
    ) {
        let mut file = File::create(&index_path).expect("create index file");
        file.write_all(&SPATIAL_INDEX_MAGIC)
            .expect("write magic header");
        let unsupported = (SPATIAL_INDEX_VERSION + 1).to_le_bytes();
        file.write_all(&unsupported).expect("write version");
        serialize_into(&mut file, &Vec::<PointOfInterest>::new()).expect("write payload");
        drop(file);

        let error = load_index_entries(&index_path).expect_err("unsupported version should fail");
        assert!(matches!(
            error,
            SpatialIndexError::UnsupportedVersion { found, supported }
                if found == SPATIAL_INDEX_VERSION + 1 && supported == SPATIAL_INDEX_VERSION
        ));
    }

    #[rstest]
    fn load_index_entries_errors_on_legacy_version(
        #[from(temp_artifacts)] (_dir, _db_path, index_path): (TempDir, PathBuf, PathBuf),
    ) {
        let mut file = File::create(&index_path).expect("create index file");
        file.write_all(&SPATIAL_INDEX_MAGIC)
            .expect("write magic header");
        let legacy = (SPATIAL_INDEX_VERSION - 1).to_le_bytes();
        file.write_all(&legacy).expect("write version");
        drop(file);

        let error = load_index_entries(&index_path).expect_err("legacy version should fail");
        assert!(matches!(
            error,
            SpatialIndexError::UnsupportedVersion { found, supported }
                if found == SPATIAL_INDEX_VERSION - 1 && supported == SPATIAL_INDEX_VERSION
        ));
    }

    #[rstest]
    fn write_index_persists_spatial_index_file(
        #[from(temp_artifacts)] (_dir, _db_path, index_path): (TempDir, PathBuf, PathBuf),
        sample_pois: Vec<PointOfInterest>,
    ) {
        write_index(&index_path, &sample_pois).expect("persist index");
        let mut file = File::open(&index_path).expect("open index");
        let payload: SpatialIndexFile = deserialize_from(&mut file).expect("decode payload");

        assert_eq!(payload.magic, SPATIAL_INDEX_MAGIC);
        assert_eq!(payload.version, SPATIAL_INDEX_VERSION);
        assert_eq!(payload.entries, sample_pois);
    }
}
