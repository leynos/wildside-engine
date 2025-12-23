//! Proptest strategies for VRP solver property-based tests.
//!
//! This module provides composable generators for creating valid test inputs
//! to property-based tests. The strategies ensure that generated inputs satisfy
//! the preconditions required by the solver, enabling robust invariant testing.

use std::collections::HashSet;

use geo::Coord;
use proptest::prelude::*;
use wildside_core::{PointOfInterest, Tags, Theme};

// Note: FixedMatrixTravelTimeProvider functions were removed since all property
// tests now use UnitTravelTimeProvider for dynamic matrix generation.

/// Strategy for generating a vector of POIs with varying count and distribution.
///
/// The count ranges from `min_count` to `max_count`, and POIs are clustered
/// around the origin with small coordinate offsets to remain within the solver's
/// candidate bounding box.
pub fn poi_set_strategy(
    min_count: usize,
    max_count: usize,
) -> impl Strategy<Value = Vec<PointOfInterest>> {
    (min_count..=max_count).prop_flat_map(|count| {
        proptest::collection::vec(poi_strategy(), count).prop_map(|pois| {
            // Ensure unique IDs by re-assigning based on position.
            pois.into_iter()
                .enumerate()
                .map(|(idx, poi)| {
                    #[expect(
                        clippy::arithmetic_side_effects,
                        reason = "index + 1 cannot overflow for reasonable test sizes"
                    )]
                    let id = (idx + 1) as u64;
                    PointOfInterest::new(id, poi.location, poi.tags.clone())
                })
                .collect()
        })
    })
}

/// Strategy for generating a single POI with random theme and location near origin.
fn poi_strategy() -> impl Strategy<Value = PointOfInterest> {
    // Use small coordinate offsets to stay within the solver's search radius.
    let x_strategy = -0.01_f64..0.01_f64;
    let y_strategy = -0.01_f64..0.01_f64;
    let theme_strategy = prop_oneof![
        Just(Theme::History),
        Just(Theme::Art),
        Just(Theme::Nature),
        Just(Theme::Culture),
    ];

    (x_strategy, y_strategy, theme_strategy).prop_map(|(x, y, theme)| {
        let tags: Tags = [(theme.to_string(), String::new())].into_iter().collect();
        // Use a placeholder ID; the caller (poi_set_strategy) will assign unique IDs.
        PointOfInterest::new(0, Coord { x, y }, tags)
    })
}

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

/// Calculate the Euclidean distance between two coordinates.
///
/// Uses simple Euclidean distance which is sufficient for property tests
/// with coordinates near the origin.
#[must_use]
#[expect(
    clippy::float_arithmetic,
    reason = "distance calculation requires floating-point arithmetic"
)]
pub fn euclidean_distance(a: &Coord<f64>, b: &Coord<f64>) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

/// Assert that a collection of POIs contains no duplicate IDs.
///
/// Returns a `Result` suitable for use with `prop_assert!` so that failures
/// integrate with proptest's shrinking and produce informative property failures
/// rather than hard panics.
///
/// # Errors
///
/// Returns an error if any POI ID appears more than once.
pub fn assert_no_duplicate_poi_ids(
    pois: &[PointOfInterest],
) -> Result<(), proptest::test_runner::TestCaseError> {
    let ids: Vec<u64> = pois.iter().map(|p| p.id).collect();
    let unique: HashSet<u64> = ids.iter().copied().collect();
    proptest::prop_assert_eq!(
        ids.len(),
        unique.len(),
        "Route contains duplicate POI IDs: {:?}",
        ids
    );
    Ok(())
}
