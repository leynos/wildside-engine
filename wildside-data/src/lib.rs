//! Data access and ingestion logic for the Wildside engine.
//!
//! Responsibilities:
//! - Define repository and source traits for ingestion and query.
//! - Provide adapters for files, HTTP and databases.
//! - Encapsulate serialization formats and schema evolution.
//!
//! Boundaries:
//! - Do not encode domain rules (live in `wildside-core`).
//! - Keep blocking I/O off async executors; prefer async-capable clients.
//!
//! Invariants:
//! - Thread-safe by default where feasible.
//! - No global mutable state.

use std::path::{Path, PathBuf};

use geo::{Coord, Rect};
use osmpbf::{Element, ElementReader};
use thiserror::Error;

/// Summary of raw OSM elements discovered during ingestion.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OsmIngestSummary {
    /// Number of nodes discovered, including dense-node entries.
    pub nodes: u64,
    /// Number of ways discovered.
    pub ways: u64,
    /// Number of relations discovered.
    pub relations: u64,
    /// Bounding box covering all node coordinates, if any nodes were present.
    pub bounds: Option<Rect<f64>>,
}

impl OsmIngestSummary {
    fn combine(mut self, other: Self) -> Self {
        self.nodes += other.nodes;
        self.ways += other.ways;
        self.relations += other.relations;
        self.bounds = Self::merge_bounds(self.bounds, other.bounds);
        self
    }

    fn merge_bounds(lhs: Option<Rect<f64>>, rhs: Option<Rect<f64>>) -> Option<Rect<f64>> {
        match (lhs, rhs) {
            (Some(left), Some(right)) => {
                let left_min = left.min();
                let left_max = left.max();
                let right_min = right.min();
                let right_max = right.max();

                Some(Rect::new(
                    Coord {
                        x: left_min.x.min(right_min.x),
                        y: left_min.y.min(right_min.y),
                    },
                    Coord {
                        x: left_max.x.max(right_max.x),
                        y: left_max.y.max(right_max.y),
                    },
                ))
            }
            (Some(bounds), None) | (None, Some(bounds)) => Some(bounds),
            (None, None) => None,
        }
    }

    fn from_element(element: Element<'_>) -> Self {
        match element {
            Element::Node(node) => Self::from_coordinate(node.lon(), node.lat()),
            Element::DenseNode(node) => Self::from_coordinate(node.lon(), node.lat()),
            Element::Way(_) => Self {
                nodes: 0,
                ways: 1,
                relations: 0,
                bounds: None,
            },
            Element::Relation(_) => Self {
                nodes: 0,
                ways: 0,
                relations: 1,
                bounds: None,
            },
        }
    }

    fn from_coordinate(lon: f64, lat: f64) -> Self {
        let bounds = if lon.is_finite() && lat.is_finite() {
            let coord = Coord { x: lon, y: lat };
            Some(Rect::new(coord, coord))
        } else {
            None
        };

        Self {
            nodes: 1,
            ways: 0,
            relations: 0,
            bounds,
        }
    }
}

/// Errors returned when ingesting an OSM PBF file.
#[derive(Debug, Error)]
pub enum OsmIngestError {
    #[error("failed to open OSM PBF file at {path:?}")]
    Open {
        #[source]
        source: osmpbf::Error,
        path: PathBuf,
    },
    #[error("failed to decode OSM PBF data")]
    Decode {
        #[source]
        source: osmpbf::Error,
    },
}

/// Parallel OSM PBF ingestion that summarises the raw element counts.
///
/// # Examples
/// ```no_run
/// use std::path::Path;
/// use wildside_data::ingest_osm_pbf;
///
/// # fn main() -> Result<(), wildside_data::OsmIngestError> {
/// let summary = ingest_osm_pbf(Path::new("planet.osm.pbf"))?;
/// println!("Nodes: {}", summary.nodes);
/// # Ok(())
/// # }
/// ```
pub fn ingest_osm_pbf(path: &Path) -> Result<OsmIngestSummary, OsmIngestError> {
    let reader = ElementReader::from_path(path).map_err(|source| OsmIngestError::Open {
        source,
        path: path.to_path_buf(),
    })?;

    reader
        .par_map_reduce(
            OsmIngestSummary::from_element,
            OsmIngestSummary::default,
            OsmIngestSummary::combine,
        )
        .map_err(|source| OsmIngestError::Decode { source })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose};
    use rstest::{fixture, rstest};
    use std::{
        fs,
        io::Write,
        path::{Path, PathBuf},
    };
    use tempfile::{Builder, TempPath};

    fn assert_close(actual: f64, expected: f64) {
        let delta = (actual - expected).abs();
        assert!(
            delta <= 1.0e-7,
            "expected {expected}, got {actual} (|Δ| = {delta})"
        );
    }

    #[fixture]
    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    fn decode_fixture(dir: &Path, stem: &str) -> TempPath {
        let encoded_path = dir.join(format!("{stem}.osm.pbf.b64"));
        let encoded = fs::read_to_string(&encoded_path).unwrap_or_else(|err| {
            panic!("failed to read base64 fixture {encoded_path:?}: {err}");
        });
        let cleaned: String = encoded
            .chars()
            .filter(|ch| !ch.is_ascii_whitespace())
            .collect();
        let decoded = general_purpose::STANDARD
            .decode(cleaned.as_bytes())
            .unwrap_or_else(|err| {
                panic!("failed to decode base64 fixture {encoded_path:?}: {err}");
            });
        let mut tempfile = Builder::new()
            .prefix(stem)
            .suffix(".osm.pbf")
            .tempfile()
            .unwrap_or_else(|err| {
                panic!("failed to create temporary fixture for {stem}: {err}");
            });
        tempfile.write_all(&decoded).unwrap_or_else(|err| {
            panic!("failed to write decoded fixture for {stem}: {err}");
        });
        tempfile.into_temp_path()
    }

    #[fixture]
    fn valid_pbf(#[from(fixtures_dir)] dir: PathBuf) -> TempPath {
        decode_fixture(&dir, "triangle")
    }

    #[fixture]
    fn invalid_pbf(#[from(fixtures_dir)] dir: PathBuf) -> TempPath {
        decode_fixture(&dir, "invalid")
    }

    #[rstest]
    fn summarises_small_fixture(valid_pbf: TempPath) -> Result<(), OsmIngestError> {
        let summary = ingest_osm_pbf(valid_pbf.as_ref())?;
        assert_eq!(summary.nodes, 3, "expected three nodes");
        assert_eq!(summary.ways, 1, "expected one way");
        assert_eq!(summary.relations, 1, "expected one relation");

        let bounds = summary.bounds.expect("expected bounds for sample nodes");
        let min = bounds.min();
        let max = bounds.max();
        assert_close(min.x, 11.62564468943);
        assert_close(max.x, 11.63101926915);
        assert_close(min.y, 52.11989910567);
        assert_close(max.y, 52.12240315616);
        Ok(())
    }

    #[rstest]
    fn propagates_open_error(#[from(fixtures_dir)] dir: PathBuf) {
        let missing = dir.join("missing.osm.pbf");
        let err = ingest_osm_pbf(&missing).expect_err("expected failure for missing file");
        match err {
            OsmIngestError::Open { path, .. } => assert_eq!(path, missing),
            other => panic!("expected open error, got {other:?}"),
        }
    }

    #[rstest]
    fn rejects_invalid_payload(invalid_pbf: TempPath) {
        let err = ingest_osm_pbf(invalid_pbf.as_ref())
            .expect_err("expected failure when decoding invalid data");
        match err {
            OsmIngestError::Decode { .. } => {}
            other => panic!("expected decode error, got {other:?}"),
        }
    }
}
