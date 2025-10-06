//! Data access traits for points of interest.
//!
//! The `PoiStore` trait defines a read-only interface for retrieving
//! [`PointOfInterest`] values. Consumers can use it to query a set of POIs
//! within a geographic bounding box.

use std::{
    collections::HashMap,
    fs::File,
    path::{Path, PathBuf},
};

use bincode::{deserialize, serialize_into};
use geo::{Coord, Rect};
use rstar::{AABB, RTree, RTreeObject};
use rusqlite::{Connection, OpenFlags, params_from_iter};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::PointOfInterest;

/// File identifier for persisted spatial indices.
pub(crate) const SPATIAL_INDEX_MAGIC: [u8; 4] = *b"WSPI";

/// Supported version of the persisted spatial index format.
pub(crate) const SPATIAL_INDEX_VERSION: u16 = 1;

/// Entry stored inside the persisted spatial index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IndexedPoi {
    id: u64,
    location: Coord<f64>,
}

impl From<&PointOfInterest> for IndexedPoi {
    fn from(poi: &PointOfInterest) -> Self {
        Self {
            id: poi.id,
            location: poi.location,
        }
    }
}

impl RTreeObject for IndexedPoi {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_point([self.location.x, self.location.y])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpatialIndexFile {
    magic: [u8; 4],
    version: u16,
    entries: Vec<IndexedPoi>,
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
    /// Reading the persisted R\*-tree from disk failed.
    #[error("failed to read spatial index from {path}: {source}")]
    IndexIo {
        /// Location of the persisted R\*-tree artefact.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The file did not contain the expected header.
    #[error("invalid spatial index magic: expected {expected:?}, found {found:?}")]
    InvalidIndexMagic {
        /// Expected byte sequence identifying a spatial index file.
        expected: [u8; 4],
        /// Sequence read from the file.
        found: [u8; 4],
    },
    /// The reader encountered an unsupported format version.
    #[error("unsupported spatial index version {found}; supported version is {supported}")]
    UnsupportedIndexVersion {
        /// Version present in the file header.
        found: u16,
        /// Latest version supported by this binary.
        supported: u16,
    },
    /// The serialised R\*-tree could not be decoded.
    #[error("failed to decode spatial index from {path}: {source}")]
    IndexDecode {
        /// Location of the persisted R\*-tree artefact.
        path: PathBuf,
        /// Decoder error returned by `bincode`.
        #[source]
        source: bincode::Error,
    },
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
    #[error("database error: {source}")]
    Database {
        /// Source error raised by the SQLite driver.
        #[from]
        source: rusqlite::Error,
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
#[derive(Debug)]
pub struct SqlitePoiStore {
    index: RTree<PointOfInterest>,
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

        let index_entries = load_index_entries(&index_path)?;
        let ids: Vec<u64> = index_entries.iter().map(|entry| entry.id).collect();
        let all_pois = load_pois(&connection, &ids)?;

        let mut pois = Vec::with_capacity(index_entries.len());
        for entry in index_entries {
            let poi_position = all_pois
                .binary_search_by_key(&entry.id, |poi| poi.id)
                .map_err(|_| SqlitePoiStoreError::MissingPoi { id: entry.id })?;
            pois.push(all_pois[poi_position].clone());
        }

        Ok(Self {
            index: RTree::bulk_load(pois),
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
        let mut pois: Vec<PointOfInterest> = self
            .index
            .locate_in_envelope_intersecting(&envelope)
            .cloned()
            .collect();
        // Sort to provide deterministic ordering for callers consuming the
        // iterator directly (e.g., behaviour specs that assert on POI IDs).
        pois.sort_unstable_by_key(|poi| poi.id);

        Box::new(pois.into_iter())
    }
}

fn load_index_entries(path: &Path) -> Result<Vec<IndexedPoi>, SqlitePoiStoreError> {
    let bytes = std::fs::read(path).map_err(|source| SqlitePoiStoreError::IndexIo {
        path: path.to_path_buf(),
        source,
    })?;

    let file: SpatialIndexFile = match deserialize(&bytes) {
        Ok(file) => file,
        Err(source) => {
            if bytes.len() < SPATIAL_INDEX_MAGIC.len() {
                let mut found = [0_u8; 4];
                found[..bytes.len()].copy_from_slice(&bytes);
                return Err(SqlitePoiStoreError::InvalidIndexMagic {
                    expected: SPATIAL_INDEX_MAGIC,
                    found,
                });
            }

            let mut found = [0_u8; 4];
            found.copy_from_slice(&bytes[..SPATIAL_INDEX_MAGIC.len()]);
            if found != SPATIAL_INDEX_MAGIC {
                return Err(SqlitePoiStoreError::InvalidIndexMagic {
                    expected: SPATIAL_INDEX_MAGIC,
                    found,
                });
            }

            return Err(SqlitePoiStoreError::IndexDecode {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    if file.magic != SPATIAL_INDEX_MAGIC {
        return Err(SqlitePoiStoreError::InvalidIndexMagic {
            expected: SPATIAL_INDEX_MAGIC,
            found: file.magic,
        });
    }

    if file.version != SPATIAL_INDEX_VERSION {
        return Err(SqlitePoiStoreError::UnsupportedIndexVersion {
            found: file.version,
            supported: SPATIAL_INDEX_VERSION,
        });
    }

    Ok(file.entries)
}

pub(crate) fn write_index(
    path: &Path,
    entries: &[IndexedPoi],
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

fn load_pois(
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
            SqlitePoiStoreError::InvalidIndexMagic { .. }
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
                .expect("write index magic");
            let unsupported = SPATIAL_INDEX_VERSION + 1;
            file.write_all(&unsupported.to_le_bytes())
                .expect("write unsupported version");
            serialize_into(&mut file, &Vec::<IndexedPoi>::new()).expect("write empty index");
        }

        let error = SqlitePoiStore::open(&db_path, &index_path)
            .expect_err("unsupported version should fail");
        assert!(matches!(
            error,
            SqlitePoiStoreError::UnsupportedIndexVersion { found, supported }
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
