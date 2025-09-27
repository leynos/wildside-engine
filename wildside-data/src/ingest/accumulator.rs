//! OpenStreetMap (OSM) PBF ingestion.
//!
//! Provides parallel ingestion that summarises raw element counts and derives
//! Points of Interest (POIs) from tagged nodes and ways. Way POIs are anchored
//! to the first resolved node reference. The main entry points are:
//! - [`ingest_osm_pbf`] for a summary only
//! - [`ingest_osm_pbf_report`] for a summary plus derived POIs
//!
//! This module is thread-safe and performs a second pass to hydrate coordinates
//! for node references required by relevant ways.
use std::collections::{HashMap, HashSet};

use geo::Coord;
use osmpbf::Element;
use wildside_core::{PointOfInterest, poi::Tags as PoiTags};

use super::ids::{OsmElementKind, encode_element_id};
use super::tags::{collect_tags, has_relevant_key};
use super::{OsmIngestReport, OsmIngestSummary};

#[derive(Debug, Default)]
pub(super) struct OsmPoiAccumulator {
    summary: OsmIngestSummary,
    nodes: HashMap<u64, Coord<f64>>,
    pending_way_nodes: HashSet<u64>,
    node_pois: Vec<PointOfInterest>,
    way_candidates: Vec<WayCandidate>,
}

impl OsmPoiAccumulator {
    pub(super) fn process_element(&mut self, element: Element<'_>) {
        match element {
            Element::Node(node) => {
                self.process_node(node.id(), node.lon(), node.lat(), node.tags())
            }
            Element::DenseNode(node) => {
                self.process_node(node.id(), node.lon(), node.lat(), node.tags())
            }
            Element::Way(way) => self.process_way(way),
            Element::Relation(relation) => {
                self.summary.record_relation();
                // Encode to validate ID range and emit logs for unsupported values.
                let _ = encode_element_id(OsmElementKind::Relation, relation.id());
            }
        }
    }

    fn process_node<'a, T>(&mut self, raw_id: i64, lon: f64, lat: f64, tags_iter: T)
    where
        T: IntoIterator<Item = (&'a str, &'a str)>,
    {
        self.summary.record_node(lon, lat);
        let Some(encoded_id) = encode_element_id(OsmElementKind::Node, raw_id) else {
            return;
        };
        let borrowed: Vec<(&'a str, &'a str)> = tags_iter.into_iter().collect();
        let is_relevant = has_relevant_key(borrowed.iter().copied());
        let was_pending = self.pending_way_nodes.remove(&encoded_id);
        let Some(location) = validated_coord(lon, lat) else {
            // Coordinates outside the valid range cannot resolve pending ways;
            // the pending marker (if any) was cleared above.
            return;
        };
        if !is_relevant && !was_pending {
            return;
        }
        self.nodes.insert(encoded_id, location);
        if is_relevant {
            let tags = collect_tags(borrowed);
            self.node_pois
                .push(PointOfInterest::new(encoded_id, location, tags));
        }
    }

    fn process_way(&mut self, way: osmpbf::Way<'_>) {
        self.summary.record_way();
        if !has_relevant_key(way.tags()) {
            return;
        }
        let tags = collect_tags(way.tags());
        let Some(encoded_id) = encode_element_id(OsmElementKind::Way, way.id()) else {
            return;
        };
        let node_refs: Vec<u64> = way
            .refs()
            .filter_map(|node_id| encode_element_id(OsmElementKind::Node, node_id))
            .collect();
        for node_id in &node_refs {
            if !self.nodes.contains_key(node_id) {
                self.pending_way_nodes.insert(*node_id);
            }
        }
        self.way_candidates.push(WayCandidate {
            id: encoded_id,
            node_refs,
            tags,
        });
    }

    pub(super) fn combine(mut self, other: Self) -> Self {
        self.summary = self.summary.combine(other.summary);
        for (id, coord) in other.nodes {
            self.nodes.entry(id).or_insert(coord);
        }
        self.node_pois.extend(other.node_pois);
        self.way_candidates.extend(other.way_candidates);
        self.pending_way_nodes.extend(other.pending_way_nodes);
        self.pending_way_nodes
            .retain(|node_id| !self.nodes.contains_key(node_id));
        self
    }

    pub(super) fn has_pending_nodes(&self) -> bool {
        !self.pending_way_nodes.is_empty()
    }

    pub(super) fn pending_way_node_count(&self) -> usize {
        self.pending_way_nodes.len()
    }

    pub(super) fn resolve_pending_node(&mut self, raw_id: i64, lon: f64, lat: f64) {
        let Some(encoded_id) = encode_element_id(OsmElementKind::Node, raw_id) else {
            return;
        };
        if !self.pending_way_nodes.contains(&encoded_id) {
            return;
        }
        match validated_coord(lon, lat) {
            Some(location) => {
                self.pending_way_nodes.remove(&encoded_id);
                self.nodes.insert(encoded_id, location);
            }
            None => {
                self.pending_way_nodes.remove(&encoded_id);
            }
        }
    }

    pub(super) fn into_report(self) -> OsmIngestReport {
        let mut pois = self.node_pois;
        // Anchor way POIs to the first resolved node reference.
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

pub(super) fn validated_coord(lon: f64, lat: f64) -> Option<Coord<f64>> {
    (lon.is_finite()
        && lat.is_finite()
        && (-180.0..=180.0).contains(&lon)
        && (-90.0..=90.0).contains(&lat))
    .then_some(Coord { x: lon, y: lat })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_node_emits_poi_for_relevant_tags() {
        let mut accumulator = OsmPoiAccumulator::default();
        accumulator.process_node(1, 13.4, 52.5, [("historic", "memorial")]);

        assert_eq!(accumulator.node_pois.len(), 1);
        let poi = accumulator
            .node_pois
            .first()
            .expect("POI should be recorded");
        assert_eq!(poi.location.x, 13.4);
        assert_eq!(poi.location.y, 52.5);
        assert!(accumulator.nodes.contains_key(&poi.id));
    }

    #[test]
    fn process_node_retains_pending_coordinates_for_irrelevant_tags() {
        let mut accumulator = OsmPoiAccumulator::default();
        let encoded = encode_element_id(OsmElementKind::Node, 2).expect("id should encode");
        accumulator.pending_way_nodes.insert(encoded);

        accumulator.process_node(2, 0.5, -0.5, [("highway", "service")]);

        assert!(accumulator.nodes.contains_key(&encoded));
        assert!(accumulator.node_pois.is_empty());
        assert!(!accumulator.pending_way_nodes.contains(&encoded));
    }

    #[test]
    fn process_node_clears_pending_for_invalid_coordinates() {
        let mut accumulator = OsmPoiAccumulator::default();
        let encoded = encode_element_id(OsmElementKind::Node, 3).expect("id should encode");
        accumulator.pending_way_nodes.insert(encoded);

        accumulator.process_node(3, 200.0, 95.0, [("tourism", "attraction")]);

        assert!(!accumulator.nodes.contains_key(&encoded));
        assert!(accumulator.node_pois.is_empty());
        assert!(!accumulator.pending_way_nodes.contains(&encoded));
    }
}
