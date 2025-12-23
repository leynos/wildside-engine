//! Proptest strategies for VRP solver property-based tests.
//!
//! This module provides composable generators for creating valid test inputs
//! to property-based tests. The strategies ensure that generated inputs satisfy
//! the preconditions required by the solver, enabling robust invariant testing.

use std::collections::HashSet;

use geo::Coord;
use wildside_core::{PointOfInterest, Tags, Theme};

// Note: FixedMatrixTravelTimeProvider functions were removed since all property
// tests now use UnitTravelTimeProvider for dynamic matrix generation.

/// Construct a POI with a single theme tag.
#[must_use]
pub fn poi_with_theme(id: u64, location: Coord<f64>, theme: &Theme) -> PointOfInterest {
    let tags: Tags = [(theme.to_string(), String::new())].into_iter().collect();
    PointOfInterest::new(id, location, tags)
}

/// Generate a collection of POIs with unique IDs near the origin.
///
/// The POIs are clustered around the origin with small coordinate offsets
/// to ensure they appear within the solver's candidate bounding box.
#[must_use]
#[expect(
    clippy::float_arithmetic,
    reason = "test helper uses simple float multiplication for coordinate offsets"
)]
pub fn generate_pois_near_origin(count: usize) -> Vec<PointOfInterest> {
    // Use a fixed set of themes to avoid modulo operation.
    let themes = [Theme::History, Theme::Art, Theme::Nature, Theme::Culture];

    (1..=count)
        .map(|i| {
            let id = i as u64;
            #[expect(
                clippy::cast_precision_loss,
                reason = "small test counts do not exceed f64 precision"
            )]
            let offset = 0.001 * (i as f64);
            // Use safe indexing with saturating subtraction to cycle through themes.
            let theme_idx = i.saturating_sub(1).checked_rem(themes.len()).unwrap_or(0);
            let theme = themes.get(theme_idx).unwrap_or(&Theme::Art);
            poi_with_theme(id, Coord { x: offset, y: 0.0 }, theme)
        })
        .collect()
}

/// Assert that a collection of POIs contains no duplicate IDs.
///
/// # Panics
///
/// Panics if any POI ID appears more than once.
pub fn assert_no_duplicate_poi_ids(pois: &[PointOfInterest]) {
    let ids: Vec<u64> = pois.iter().map(|p| p.id).collect();
    let unique: HashSet<u64> = ids.iter().copied().collect();
    assert_eq!(
        ids.len(),
        unique.len(),
        "Route contains duplicate POI IDs: {ids:?}"
    );
}
