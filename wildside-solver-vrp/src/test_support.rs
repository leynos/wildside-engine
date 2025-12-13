//! Test-only utilities for `wildside-solver-vrp`.
//!
//! The helpers in this module are available to unit tests and behavioural
//! tests. They are gated behind the `test-support` feature (and `cfg(test)`).

use geo::Coord;
use wildside_core::{PointOfInterest, Tags};

/// Construct a `PointOfInterest` tagged with a theme key.
///
/// # Examples
/// ```rust
/// use wildside_solver_vrp::test_support::poi;
///
/// let poi = poi(1, 0.0, 0.0, "art");
/// assert_eq!(poi.id, 1);
/// assert!(poi.tags.contains_key("art"));
/// ```
#[must_use]
pub fn poi(id: u64, x: f64, y: f64, theme: &str) -> PointOfInterest {
    PointOfInterest::new(
        id,
        Coord { x, y },
        Tags::from([(theme.to_owned(), String::new())]),
    )
}
