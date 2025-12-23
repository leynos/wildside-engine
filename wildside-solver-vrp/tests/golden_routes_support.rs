//! Shared utilities for golden route tests.
//!
//! This module contains data structures and helper functions used by both
//! the rstest parameterised tests and the BDD behavioural tests.
//!
//! # Matrix Ordering Requirement
//!
//! The `FixedMatrixTravelTimeProvider` returns the travel time matrix as-is,
//! without reordering based on POI IDs. Since the VRP solver sorts candidates
//! by score (descending) then by ID (ascending), test fixtures must ensure
//! that all POIs have equal scores to guarantee stable ordering by ID. This
//! ensures the matrix indices align correctly with the POI order the solver
//! constructs.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use geo::Coord;
use serde::Deserialize;
use wildside_core::{InterestProfile, PointOfInterest, SolveRequest, Tags, Theme};

/// Deserialised golden route test case.
#[derive(Debug, Deserialize, Clone)]
#[expect(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    reason = "fields are used by some test binaries but not others"
)]
pub struct GoldenRoute {
    /// Name of the test case (used in error messages).
    #[allow(dead_code)]
    pub name: String,
    /// Human-readable description of what the test validates.
    #[allow(dead_code)]
    pub description: String,
    /// POI specifications to load.
    pub pois: Vec<PoiSpec>,
    /// Travel time matrix in seconds (row/col indices match POI order).
    pub travel_time_matrix_seconds: Vec<Vec<u64>>,
    /// Request parameters.
    pub request: RequestSpec,
    /// Expected results for validation.
    pub expected: ExpectedResult,
}

/// POI specification from JSON.
#[derive(Debug, Deserialize, Clone)]
pub struct PoiSpec {
    /// Unique POI identifier.
    pub id: u64,
    /// Longitude.
    pub x: f64,
    /// Latitude.
    pub y: f64,
    /// Tags mapping theme keys to values.
    pub tags: HashMap<String, String>,
}

/// Request specification from JSON.
#[derive(Debug, Deserialize, Clone)]
pub struct RequestSpec {
    /// Starting coordinate.
    pub start: CoordSpec,
    /// Optional ending coordinate (if different from start).
    pub end: Option<CoordSpec>,
    /// Time budget in minutes.
    pub duration_minutes: u16,
    /// Interest weights by theme.
    pub interests: HashMap<String, f32>,
    /// Random seed for solver.
    pub seed: u64,
    /// Optional limit on candidates to consider.
    pub max_nodes: Option<u16>,
}

/// Coordinate specification from JSON.
#[derive(Debug, Deserialize, Clone)]
pub struct CoordSpec {
    /// Longitude.
    pub x: f64,
    /// Latitude.
    pub y: f64,
}

/// Expected result from JSON.
#[derive(Debug, Deserialize, Clone)]
pub struct ExpectedResult {
    /// Expected POI IDs in the route (compared as set, not order).
    pub route_poi_ids: Vec<u64>,
    /// Minimum acceptable score.
    pub min_score: f32,
    /// Maximum acceptable score.
    pub max_score: f32,
    /// Whether the route should respect the time budget.
    pub respects_budget: bool,
}

/// Load a golden route from the data directory by name (without extension).
///
/// # Panics
///
/// Panics if the file cannot be read or parsed.
#[must_use]
pub fn load_golden_route(name: &str) -> GoldenRoute {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden_routes/data")
        .join(format!("{name}.json"));
    let content = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "failed to read golden route file at {}: {}",
            path.display(),
            e
        )
    });
    serde_json::from_str(&content).unwrap_or_else(|e| {
        panic!(
            "failed to parse golden route JSON at {}: {}",
            path.display(),
            e
        )
    })
}

/// Convert POI specs to domain POIs.
#[must_use]
pub fn build_pois(specs: &[PoiSpec]) -> Vec<PointOfInterest> {
    specs
        .iter()
        .map(|s| {
            let tags: Tags = s.tags.clone().into_iter().collect();
            PointOfInterest::new(s.id, Coord { x: s.x, y: s.y }, tags)
        })
        .collect()
}

/// Convert request spec to domain request.
///
/// # Panics
///
/// Panics if the request contains an invalid theme string.
#[must_use]
pub fn build_request(spec: &RequestSpec) -> SolveRequest {
    let mut interests = InterestProfile::new();
    for (theme_str, weight) in &spec.interests {
        let theme: Theme = theme_str
            .parse()
            .unwrap_or_else(|_| panic!("golden route contains invalid theme: {theme_str}"));
        interests.set_weight(theme, *weight);
    }
    SolveRequest {
        start: Coord {
            x: spec.start.x,
            y: spec.start.y,
        },
        end: spec.end.as_ref().map(|e| Coord { x: e.x, y: e.y }),
        duration_minutes: spec.duration_minutes,
        interests,
        seed: spec.seed,
        max_nodes: spec.max_nodes,
    }
}
