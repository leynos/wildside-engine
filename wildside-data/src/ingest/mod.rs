use std::path::{Path, PathBuf};

use geo::{Coord, Rect};
use log::warn;
use osmpbf::{Element, ElementReader};
use thiserror::Error;
use wildside_core::PointOfInterest;

mod accumulator;
mod ids;
mod tags;

use accumulator::OsmPoiAccumulator;

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

    let mut accumulator = reader
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

    if accumulator.has_pending_nodes() {
        let path_buf = path.to_path_buf();
        let resolver = ElementReader::from_path(path).map_err(|source| OsmIngestError::Open {
            source,
            path: path_buf.clone(),
        })?;
        {
            let accumulator_ref = &mut accumulator;
            resolver
                .for_each(|element| match element {
                    Element::Node(node) => {
                        accumulator_ref.resolve_pending_node(node.id(), node.lon(), node.lat());
                    }
                    Element::DenseNode(node) => {
                        accumulator_ref.resolve_pending_node(node.id(), node.lon(), node.lat());
                    }
                    Element::Way(_) | Element::Relation(_) => {}
                })
                .map_err(|source| OsmIngestError::Decode {
                    source,
                    path: path_buf.clone(),
                })?;
        }
        if accumulator.has_pending_nodes() {
            warn!(
                "Skipped {} way node references without coordinates",
                accumulator.pending_way_node_count()
            );
        }
    }

    Ok(accumulator.into_report())
}
