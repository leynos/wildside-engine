//! Routes: ordered paths through points of interest with a caller-supplied duration.
//!
//! This module defines [`Route`], representing an ordered sequence of points of
//! interest along with an overall duration. The duration is not computed from
//! POIs; supply it from your planning logic.

use std::time::Duration;

use crate::PointOfInterest;

/// An ordered path through points of interest with an overall duration.
///
/// # Examples
/// ```rust
/// use geo::Coord;
/// use std::time::Duration;
/// use wildside_core::{PointOfInterest, Route};
///
/// let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
/// let route = Route::new(vec![poi], Duration::from_secs(60));
///
/// assert_eq!(route.pois().len(), 1);
/// assert_eq!(route.total_duration().as_secs(), 60);
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
#[must_use]
pub struct Route {
    /// Points of interest visited in order.
    pois: Vec<PointOfInterest>,
    /// Total duration of the route.
    total_duration: Duration,
}

impl Route {
    /// Construct a route from points and total duration.
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
    use geo::Coord;

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
}
