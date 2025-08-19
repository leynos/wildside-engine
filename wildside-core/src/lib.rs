//! Core domain types for the Wildside engine.
//!
//! These models provide basic validation to keep downstream
//! components honest. Constructors return `Result` to surface
//! invalid input early.

use geo::Coord;
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;

/// A single location that may interest a user.
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use geo::Coord;
/// use wildside_core::PointOfInterest;
///
/// # fn main() -> Result<(), wildside_core::PointOfInterestError> {
/// let mut tags = HashMap::new();
/// tags.insert("name".to_string(), "Museum".to_string());
/// let poi = PointOfInterest::new(1, Coord { x: 0.0, y: 0.0 }, tags)?;
/// assert_eq!(poi.id, 1);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PointOfInterest {
    /// Unique identifier.
    pub id: u64,
    /// Geospatial position.
    pub location: Coord,
    /// OpenStreetMap-style tags.
    pub tags: HashMap<String, String>,
}

/// Errors returned by [`PointOfInterest::new`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PointOfInterestError {
    /// No tags were supplied.
    #[error("point of interest must have at least one tag")]
    MissingTags,
}

impl PointOfInterest {
    /// Validates and constructs a [`PointOfInterest`].
    pub fn new(
        id: u64,
        location: Coord,
        tags: HashMap<String, String>,
    ) -> Result<Self, PointOfInterestError> {
        if tags.is_empty() {
            return Err(PointOfInterestError::MissingTags);
        }
        Ok(Self { id, location, tags })
    }
}

/// A user's thematic interests with associated weights.
///
/// The map keys are arbitrary theme identifiers and the values
/// are weights in the inclusive range `[0.0, 1.0]`.
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use wildside_core::InterestProfile;
///
/// # fn main() -> Result<(), wildside_core::InterestProfileError> {
/// let mut weights = HashMap::new();
/// weights.insert("art".to_string(), 0.8);
/// let profile = InterestProfile::new(weights)?;
/// assert!(profile.weights.contains_key("art"));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct InterestProfile {
    /// Interest weights keyed by theme name.
    pub weights: HashMap<String, f32>,
}

/// Errors returned by [`InterestProfile::new`].
#[derive(Debug, Error, PartialEq)]
pub enum InterestProfileError {
    /// No themes were provided.
    #[error("interest profile must contain at least one theme")]
    Empty,
    /// A weight was outside the valid range.
    #[error("interest weight must be between 0.0 and 1.0")]
    InvalidWeight,
}

impl InterestProfile {
    /// Validates and constructs an [`InterestProfile`].
    pub fn new(weights: HashMap<String, f32>) -> Result<Self, InterestProfileError> {
        if weights.is_empty() {
            return Err(InterestProfileError::Empty);
        }
        if weights.values().any(|w| !(0.0..=1.0).contains(w)) {
            return Err(InterestProfileError::InvalidWeight);
        }
        Ok(Self { weights })
    }
}

/// A complete route visiting a sequence of points.
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use geo::Coord;
/// use std::time::Duration;
/// use wildside_core::{PointOfInterest, Route};
///
/// # fn main() -> Result<(), wildside_core::RouteError> {
/// let mut tags = HashMap::new();
/// tags.insert("name".into(), "Museum".into());
/// let poi = PointOfInterest::new(1, Coord { x: 0.0, y: 0.0 }, tags).unwrap();
/// let route = Route::new(vec![poi], Duration::from_secs(60))?;
/// assert_eq!(route.points.len(), 1);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    /// Ordered points to visit.
    pub points: Vec<PointOfInterest>,
    /// Total travel and visit duration.
    pub total_duration: Duration,
}

/// Errors returned by [`Route::new`].
#[derive(Debug, Error, PartialEq)]
pub enum RouteError {
    /// No points were supplied.
    #[error("route must contain at least one point of interest")]
    Empty,
    /// Route duration was zero.
    #[error("route duration must be positive")]
    NonPositiveDuration,
}

impl Route {
    /// Validates and constructs a [`Route`].
    pub fn new(points: Vec<PointOfInterest>, total_duration: Duration) -> Result<Self, RouteError> {
        if points.is_empty() {
            return Err(RouteError::Empty);
        }
        if total_duration.is_zero() {
            return Err(RouteError::NonPositiveDuration);
        }
        Ok(Self {
            points,
            total_duration,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn sample_poi() -> PointOfInterest {
        let mut tags = HashMap::new();
        tags.insert("name".into(), "Museum".into());
        PointOfInterest::new(1, Coord { x: 0.0, y: 0.0 }, tags).unwrap()
    }

    #[rstest]
    fn poi_requires_tags() {
        let tags = HashMap::new();
        let result = PointOfInterest::new(1, Coord { x: 0.0, y: 0.0 }, tags);
        assert!(result.is_err());
    }

    #[rstest]
    #[case(0.0)]
    #[case(1.0)]
    fn profile_accepts_boundary_weights(#[case] weight: f32) {
        let mut weights = HashMap::new();
        weights.insert("art".into(), weight);
        assert!(InterestProfile::new(weights).is_ok());
    }

    #[rstest]
    #[case(-0.1)]
    #[case(1.1)]
    fn profile_rejects_out_of_range_weight(#[case] weight: f32) {
        let mut weights = HashMap::new();
        weights.insert("art".into(), weight);
        assert!(InterestProfile::new(weights).is_err());
    }

    #[rstest]
    fn route_requires_points() {
        let result = Route::new(Vec::new(), Duration::from_secs(10));
        assert!(matches!(result, Err(RouteError::Empty)));
    }

    #[rstest]
    fn route_requires_positive_duration(sample_poi: PointOfInterest) {
        let result = Route::new(vec![sample_poi], Duration::ZERO);
        assert!(matches!(result, Err(RouteError::NonPositiveDuration)));
    }
}
