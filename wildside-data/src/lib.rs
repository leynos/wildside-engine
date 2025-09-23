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

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use geo::{Coord, Rect};
use osmpbf::{Element, ElementReader};
use thiserror::Error;
use wildside_core::PointOfInterest;
use wildside_core::poi::Tags as PoiTags;

const WAY_ID_PREFIX: u64 = 1 << 62;
const TYPE_ID_MASK: u64 = (1 << 62) - 1;

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
    /// Coordinates are WGS84 with `x = longitude`, `y = latitude`.
    pub bounds: Option<Rect<f64>>,
}

impl OsmIngestSummary {
    fn combine(mut self, other: Self) -> Self {
        self.nodes += other.nodes;
        self.ways += other.ways;
        self.relations += other.relations;
        if let Some(bounds) = other.bounds {
            self.include_bounds(bounds);
        }
        self
    }

    fn include_bounds(&mut self, bounds: Rect<f64>) {
        match &mut self.bounds {
            Some(existing) => {
                let min = Coord {
                    x: existing.min().x.min(bounds.min().x),
                    y: existing.min().y.min(bounds.min().y),
                };
                let max = Coord {
                    x: existing.max().x.max(bounds.max().x),
                    y: existing.max().y.max(bounds.max().y),
                };
                *existing = Rect::new(min, max);
            }
            None => self.bounds = Some(bounds),
        }
    }

    fn record_node(&mut self, lon: f64, lat: f64) {
        self.nodes += 1;
        if let Some(bounds) = Self::coordinate_bounds(lon, lat) {
            self.include_bounds(bounds);
        }
    }

    fn record_way(&mut self) {
        self.ways += 1;
    }

    fn record_relation(&mut self) {
        self.relations += 1;
    }

    fn coordinate_bounds(lon: f64, lat: f64) -> Option<Rect<f64>> {
        (lon.is_finite()
            && lat.is_finite()
            && (-180.0..=180.0).contains(&lon)
            && (-90.0..=90.0).contains(&lat))
        .then(|| {
            let coordinate = Coord { x: lon, y: lat };
            Rect::new(coordinate, coordinate)
        })
    }
}

/// Detailed report of an OSM ingestion run.
#[derive(Debug, Clone, PartialEq)]
pub struct OsmIngestReport {
    /// Element counts and bounding box information.
    pub summary: OsmIngestSummary,
    /// Points of interest derived from relevant OSM elements.
    pub pois: Vec<PointOfInterest>,
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
    #[error("failed to decode OSM PBF data at {path:?}")]
    Decode {
        #[source]
        source: osmpbf::Error,
        path: PathBuf,
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
    ingest_osm_pbf_report(path).map(|report| report.summary)
}

/// Ingest an OSM PBF file, producing both counts and derived POIs.
///
/// # Examples
/// ```no_run
/// use std::path::Path;
/// use wildside_data::ingest_osm_pbf_report;
///
/// # fn main() -> Result<(), wildside_data::OsmIngestError> {
/// let report = ingest_osm_pbf_report(Path::new("berlin.osm.pbf"))?;
/// println!("Loaded {} points of interest", report.pois.len());
/// # Ok(())
/// # }
/// ```
pub fn ingest_osm_pbf_report(path: &Path) -> Result<OsmIngestReport, OsmIngestError> {
    let reader = ElementReader::from_path(path).map_err(|source| OsmIngestError::Open {
        source,
        path: path.to_path_buf(),
    })?;

    let accumulator = reader
        .par_map_reduce(
            |element| {
                let mut accumulator = OsmPoiAccumulator::default();
                accumulator.process_element(element);
                accumulator
            },
            OsmPoiAccumulator::default,
            OsmPoiAccumulator::combine,
        )
        .map_err(|source| OsmIngestError::Decode {
            source,
            path: path.to_path_buf(),
        })?;

    Ok(accumulator.into_report())
}

#[derive(Debug, Default)]
struct OsmPoiAccumulator {
    summary: OsmIngestSummary,
    nodes: HashMap<u64, Coord<f64>>,
    node_pois: Vec<PointOfInterest>,
    way_candidates: Vec<WayCandidate>,
}

impl OsmPoiAccumulator {
    fn process_element(&mut self, element: Element<'_>) {
        match element {
            Element::Node(node) => {
                self.process_node(node.id(), node.lon(), node.lat(), node.tags())
            }
            Element::DenseNode(node) => {
                self.process_node(node.id(), node.lon(), node.lat(), node.tags())
            }
            Element::Way(way) => self.process_way(way),
            Element::Relation(_) => self.summary.record_relation(),
        }
    }

    fn process_node<'a, T>(&mut self, raw_id: i64, lon: f64, lat: f64, tags: T)
    where
        T: IntoIterator<Item = (&'a str, &'a str)>,
    {
        self.summary.record_node(lon, lat);
        let Some(encoded_id) = encode_element_id(OsmElementKind::Node, raw_id) else {
            return;
        };
        let Some(location) = coordinate(lon, lat) else {
            return;
        };

        self.nodes.entry(encoded_id).or_insert(location);
        let TagCollection { tags, is_relevant } = collect_tags(tags);
        if is_relevant {
            self.node_pois
                .push(PointOfInterest::new(encoded_id, location, tags));
        }
    }

    fn process_way(&mut self, way: osmpbf::Way<'_>) {
        self.summary.record_way();
        let TagCollection { tags, is_relevant } = collect_tags(way.tags());
        if !is_relevant {
            return;
        }
        let Some(encoded_id) = encode_element_id(OsmElementKind::Way, way.id()) else {
            return;
        };
        let node_refs: Vec<u64> = way
            .refs()
            .filter_map(|node_id| encode_element_id(OsmElementKind::Node, node_id))
            .collect();
        self.way_candidates.push(WayCandidate {
            id: encoded_id,
            node_refs,
            tags,
        });
    }

    fn combine(mut self, other: Self) -> Self {
        self.summary = self.summary.combine(other.summary);
        self.nodes.extend(other.nodes);
        self.node_pois.extend(other.node_pois);
        self.way_candidates.extend(other.way_candidates);
        self
    }

    fn into_report(self) -> OsmIngestReport {
        let mut pois = self.node_pois;
        for candidate in self.way_candidates {
            if let Some(location) = candidate
                .node_refs
                .iter()
                .find_map(|node_id| self.nodes.get(node_id))
                .copied()
            {
                pois.push(PointOfInterest::new(candidate.id, location, candidate.tags));
            }
        }
        pois.sort_by_key(|poi| poi.id);
        OsmIngestReport {
            summary: self.summary,
            pois,
        }
    }
}

#[derive(Debug)]
struct WayCandidate {
    id: u64,
    node_refs: Vec<u64>,
    tags: PoiTags,
}

#[derive(Copy, Clone)]
enum OsmElementKind {
    Node,
    Way,
}

fn encode_element_id(kind: OsmElementKind, raw_id: i64) -> Option<u64> {
    let base = u64::try_from(raw_id).ok()?;
    if base > TYPE_ID_MASK {
        return None;
    }
    let prefix = match kind {
        OsmElementKind::Node => 0,
        OsmElementKind::Way => WAY_ID_PREFIX,
    };
    Some(prefix | base)
}

fn coordinate(lon: f64, lat: f64) -> Option<Coord<f64>> {
    (lon.is_finite()
        && lat.is_finite()
        && (-180.0..=180.0).contains(&lon)
        && (-90.0..=90.0).contains(&lat))
    .then_some(Coord { x: lon, y: lat })
}

struct TagCollection {
    tags: PoiTags,
    is_relevant: bool,
}

fn collect_tags<'a, T>(tags: T) -> TagCollection
where
    T: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut collected = PoiTags::new();
    let mut is_relevant = false;
    for (key, value) in tags {
        if is_relevant_key(key) {
            is_relevant = true;
        }
        collected.insert(key.to_owned(), value.to_owned());
    }
    TagCollection {
        tags: collected,
        is_relevant,
    }
}

fn is_relevant_key(key: &str) -> bool {
    matches!(key, "historic" | "tourism")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};
    use std::path::PathBuf;
    use tempfile::TempPath;

    mod support {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support.rs"));
    }

    use support::{assert_close, decode_fixture};

    #[fixture]
    fn fixtures_dir() -> PathBuf {
        support::fixtures_dir()
    }

    #[fixture]
    fn valid_pbf(#[from(fixtures_dir)] dir: PathBuf) -> TempPath {
        decode_fixture(&dir, "triangle")
    }

    #[fixture]
    fn invalid_pbf(#[from(fixtures_dir)] dir: PathBuf) -> TempPath {
        decode_fixture(&dir, "invalid")
    }

    #[fixture]
    fn poi_pbf(#[from(fixtures_dir)] dir: PathBuf) -> TempPath {
        decode_fixture(&dir, "poi_tags")
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
    fn extracts_relevant_pois(poi_pbf: TempPath) -> Result<(), OsmIngestError> {
        let report = ingest_osm_pbf_report(poi_pbf.as_ref())?;
        assert_eq!(report.summary.nodes, 3, "expected three nodes in fixture");
        assert_eq!(report.summary.ways, 3, "expected three ways in fixture");
        assert_eq!(
            report.summary.relations, 1,
            "expected one relation in fixture"
        );
        assert_eq!(
            report.pois.len(),
            3,
            "expected two nodes and one way to become POIs"
        );

        let names: Vec<String> = report
            .pois
            .iter()
            .filter_map(|poi| poi.tags.get("name").cloned())
            .collect();
        assert!(names.contains(&"Brandenburg Gate".to_string()));
        assert!(names.contains(&"Pergamon Museum".to_string()));
        assert!(names.contains(&"Museum Island Walk".to_string()));

        let walk = report
            .pois
            .iter()
            .find(|poi| poi.tags.get("name") == Some(&"Museum Island Walk".to_string()))
            .expect("way POI should be present");
        assert_eq!(walk.tags.get("tourism"), Some(&"attraction".to_string()));
        assert_close(walk.location.x, 13.404954);
        assert_close(walk.location.y, 52.520008);

        let ruins_count = report
            .pois
            .iter()
            .filter(|poi| poi.tags.get("historic") == Some(&"ruins".to_string()))
            .count();
        assert_eq!(
            ruins_count, 0,
            "ways without resolvable nodes should be ignored"
        );
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
            OsmIngestError::Decode { source, path } => {
                let extension = path.extension().and_then(|ext| ext.to_str());
                assert_eq!(extension, Some("pbf"), "unexpected path in error: {path:?}");
                assert!(
                    !source.to_string().is_empty(),
                    "decode error should preserve the source message"
                );
            }
            other => panic!("expected decode error, got {other:?}"),
        }
    }
}
