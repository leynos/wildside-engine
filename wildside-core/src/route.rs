//! Routes: ordered paths through points of interest with a caller-supplied duration.
//!
//! This module defines [`Route`], representing an ordered sequence of points of
//! interest along with an overall duration. The duration is not computed from
//! POIs; supply it from your planning logic.

use std::time::Duration;

use geo::Coord;

use crate::PointOfInterest;

/// An ordered path through points of interest with an overall duration.
///
/// A route represents a path from a start coordinate, through zero or more
/// points of interest, to an end coordinate. The start and end may be the
/// same location (round-trip) or different (point-to-point).
///
/// # Examples
/// ```rust
/// use geo::Coord;
/// use std::time::Duration;
/// use wildside_core::{PointOfInterest, Route};
///
/// let start = Coord { x: 0.0, y: 0.0 };
/// let end = Coord { x: 1.0, y: 1.0 };
/// let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.5, y: 0.5 });
/// let route = Route::with_endpoints(start, end, vec![poi], Duration::from_secs(60));
///
/// assert_eq!(route.start(), start);
/// assert_eq!(route.end(), end);
/// assert_eq!(route.pois().len(), 1);
/// assert_eq!(route.total_duration().as_secs(), 60);
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Route {
    /// Starting coordinate of the route.
    start: Coord<f64>,
    /// Ending coordinate of the route.
    end: Coord<f64>,
    /// Points of interest visited in order.
    pois: Vec<PointOfInterest>,
    /// Total duration of the route.
    total_duration: Duration,
}

impl Default for Route {
    fn default() -> Self {
        Self {
            start: Coord { x: 0.0, y: 0.0 },
            end: Coord { x: 0.0, y: 0.0 },
            pois: Vec::new(),
            total_duration: Duration::ZERO,
        }
    }
}

impl Route {
    /// Construct a route with explicit start and end coordinates.
    ///
    /// # Examples
    /// ```rust
    /// use geo::Coord;
    /// use std::time::Duration;
    /// use wildside_core::{PointOfInterest, Route};
    ///
    /// let start = Coord { x: 0.0, y: 0.0 };
    /// let end = Coord { x: 1.0, y: 1.0 };
    /// let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.5, y: 0.5 });
    /// let route = Route::with_endpoints(start, end, vec![poi.clone()], Duration::from_secs(30));
    /// assert_eq!(route.start(), start);
    /// assert_eq!(route.end(), end);
    /// assert_eq!(route.pois(), &[poi]);
    /// ```
    pub fn with_endpoints(
        start: Coord<f64>,
        end: Coord<f64>,
        pois: Vec<PointOfInterest>,
        total_duration: Duration,
    ) -> Self {
        Self {
            start,
            end,
            pois,
            total_duration,
        }
    }

    /// Construct a route from points and total duration.
    ///
    /// The start and end coordinates default to the origin. Use
    /// [`Route::with_endpoints`] to specify explicit start/end coordinates.
    ///
    /// # Examples
    /// ```rust
    /// use geo::Coord;
    /// use std::time::Duration;
    /// use wildside_core::{PointOfInterest, Route};
    ///
    /// let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    /// let route = Route::new(vec![poi.clone()], Duration::from_secs(30));
    /// assert_eq!(route.pois(), &[poi]);
    /// ```
    pub fn new(pois: Vec<PointOfInterest>, total_duration: Duration) -> Self {
        Self {
            start: Coord { x: 0.0, y: 0.0 },
            end: Coord { x: 0.0, y: 0.0 },
            pois,
            total_duration,
        }
    }

    /// Construct an empty route.
    ///
    /// # Examples
    /// ```rust
    /// use wildside_core::Route;
    ///
    /// let route = Route::empty();
    /// assert!(route.pois().is_empty());
    /// assert_eq!(route.total_duration().as_secs(), 0);
    /// ```
    #[rustfmt::skip]
    pub fn empty() -> Self { Self::default() }

    /// Starting coordinate of the route.
    #[rustfmt::skip]
    pub fn start(&self) -> Coord<f64> { self.start }

    /// Ending coordinate of the route.
    #[rustfmt::skip]
    pub fn end(&self) -> Coord<f64> { self.end }

    /// Points of interest in order.
    #[rustfmt::skip]
    pub fn pois(&self) -> &[PointOfInterest] { &self.pois }

    /// Total duration of the route.
    #[rustfmt::skip]
    pub fn total_duration(&self) -> Duration { self.total_duration }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_preserves_order() {
        let poi1 = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
        let poi2 = PointOfInterest::with_empty_tags(2, Coord { x: 1.0, y: 1.0 });
        let route = Route::new(vec![poi1.clone(), poi2.clone()], Duration::from_secs(120));
        assert_eq!(route.pois(), &[poi1, poi2]);
        assert_eq!(route.total_duration().as_secs(), 120);
    }

    #[test]
    fn empty_route_has_zero_duration() {
        let route = Route::empty();
        assert!(route.pois().is_empty());
        assert_eq!(route.total_duration().as_secs(), 0);
    }

    #[test]
    fn route_with_endpoints_stores_coordinates() {
        let start = Coord { x: 1.0, y: 2.0 };
        let end = Coord { x: 3.0, y: 4.0 };
        let poi = PointOfInterest::with_empty_tags(1, Coord { x: 2.0, y: 3.0 });
        let route = Route::with_endpoints(start, end, vec![poi], Duration::from_secs(60));
        assert_eq!(route.start(), start);
        assert_eq!(route.end(), end);
    }
}
