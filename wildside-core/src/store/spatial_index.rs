//! Persisted spatial index file format helpers.
//!
//! These helpers define the on-disk representation for the R\*-tree indices
//! used by the SQLite-backed POI store.

use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use bincode::{deserialize_from, serialize_into};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::PointOfInterest;

/// File identifier for persisted spatial indices.
pub(crate) const SPATIAL_INDEX_MAGIC: [u8; 4] = *b"WSPI";

/// Supported version of the persisted spatial index format.
pub(crate) const SPATIAL_INDEX_VERSION: u16 = 2;

/// Payload stored after the spatial index header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SpatialIndexFile {
    pub(crate) magic: [u8; 4],
    pub(crate) version: u16,
    pub(crate) entries: Vec<PointOfInterest>,
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

/// Persist a spatial index artefact containing the provided POIs.
///
/// The file is written in the `WSPI` binary format expected by
/// `SqlitePoiStore`. It combines a fixed header with a `bincode` payload of
/// `PointOfInterest` entries. Existing files are truncated.
pub fn write_spatial_index(
    path: &Path,
    entries: &[PointOfInterest],
) -> Result<(), SpatialIndexWriteError> {
    write_index(path, entries)
}

/// Persist a spatial index file without exposing the public wrapper signature.
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

/// Load POI entries from a spatial index artefact.
pub(crate) fn load_index_entries(path: &Path) -> Result<Vec<PointOfInterest>, SpatialIndexError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PointOfInterest, Tags};
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
    fn temp_index_path() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("create temp dir");
        let index_path = dir.path().join("pois.rstar");
        (dir, index_path)
    }

    #[fixture]
    fn sample_pois() -> Vec<PointOfInterest> {
        vec![poi(1, 0.0, 0.0, "centre"), poi(2, 2.0, 2.0, "museum")]
    }

    #[rstest]
    fn load_index_entries_round_trips_entries(
        #[from(temp_index_path)] (_dir, index_path): (TempDir, PathBuf),
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
        #[from(temp_index_path)] (_dir, index_path): (TempDir, PathBuf),
    ) {
        std::fs::write(&index_path, b"BAD!").expect("write corrupt header");

        let error = load_index_entries(&index_path).expect_err("invalid magic should fail");
        assert!(matches!(error, SpatialIndexError::InvalidMagic { .. }));
    }

    #[rstest]
    fn load_index_entries_errors_on_decode_failure(
        #[from(temp_index_path)] (_dir, index_path): (TempDir, PathBuf),
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
        #[from(temp_index_path)] (_dir, index_path): (TempDir, PathBuf),
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
        #[from(temp_index_path)] (_dir, index_path): (TempDir, PathBuf),
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
        #[from(temp_index_path)] (_dir, index_path): (TempDir, PathBuf),
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
