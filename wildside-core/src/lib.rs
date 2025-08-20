//! Core domain types for the Wildside engine.

use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::time::Duration;

use geo::Coord;

/// Themes describing broad categories of interest.
///
/// The enum offers compile-time safety for interest lookups.
///
/// # Examples
/// ```
/// use wildside_core::Theme;
///
/// assert_eq!(Theme::History.as_str(), "history");
/// assert_eq!(Theme::Art.to_string(), "art");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Theme {
    History,
    Art,
    Nature,
    Food,
    Architecture,
    Shopping,
    Entertainment,
    Culture,
}

impl Theme {
    /// Return the theme as a lowercase `&str`.
    ///
    /// # Examples
    /// ```
    /// use wildside_core::Theme;
    ///
    /// assert_eq!(Theme::Nature.as_str(), "nature");
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::History => "history",
            Self::Art => "art",
            Self::Nature => "nature",
            Self::Food => "food",
            Self::Architecture => "architecture",
            Self::Shopping => "shopping",
            Self::Entertainment => "entertainment",
            Self::Culture => "culture",
        }
    }
}

impl Display for Theme {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Theme {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "history" => Ok(Self::History),
            "art" => Ok(Self::Art),
            "nature" => Ok(Self::Nature),
            "food" => Ok(Self::Food),
            "architecture" => Ok(Self::Architecture),
            "shopping" => Ok(Self::Shopping),
            "entertainment" => Ok(Self::Entertainment),
            "culture" => Ok(Self::Culture),
            other => Err(format!("unknown theme '{other}'")),
        }
    }
}

/// A location worth visiting.
///
/// Tags mirror OpenStreetMap's free-form key/value structure.
///
/// # Examples
/// ```
/// use std::collections::HashMap;
/// use geo::Coord;
/// use wildside_core::PointOfInterest;
///
/// let poi = PointOfInterest::new(
///     1,
///     Coord { x: 1.0, y: 2.0 },
///     HashMap::from([("name".into(), "Museum".into())]),
/// );
///
/// assert_eq!(poi.id, 1);
/// assert_eq!(poi.tags.get("name"), Some(&"Museum".to_string()));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PointOfInterest {
    pub id: u64,
    pub location: Coord<f64>,
    pub tags: HashMap<String, String>,
}

impl PointOfInterest {
    /// Construct a `PointOfInterest` with the provided tags.
    ///
    /// # Examples
    /// ```
    /// use std::collections::HashMap;
    /// use geo::Coord;
    /// use wildside_core::PointOfInterest;
    ///
    /// let poi = PointOfInterest::new(1, Coord { x: 0.0, y: 0.0 }, HashMap::new());
    /// assert_eq!(poi.id, 1);
    /// ```
    pub fn new(id: u64, location: Coord<f64>, tags: HashMap<String, String>) -> Self {
        Self { id, location, tags }
    }

    /// Construct a `PointOfInterest` without tags.
    ///
    /// # Examples
    /// ```
    /// use geo::Coord;
    /// use wildside_core::PointOfInterest;
    ///
    /// let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    /// assert!(poi.tags.is_empty());
    /// ```
    pub fn with_empty_tags(id: u64, location: Coord<f64>) -> Self {
        Self::new(id, location, HashMap::new())
    }
}

/// User interest weighting across themes.
///
/// # Examples
/// ```
/// use wildside_core::{InterestProfile, Theme};
///
/// let profile = InterestProfile::new()
///     .with_weight(Theme::History, 0.8)
///     .with_weight(Theme::Art, 0.6);
/// assert_eq!(profile.weight(&Theme::History), Some(0.8));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct InterestProfile {
    weights: HashMap<Theme, f32>,
}

impl Default for InterestProfile {
    fn default() -> Self {
        Self::new()
    }
}

impl InterestProfile {
    /// Construct an empty profile.
    ///
    /// # Examples
    /// ```
    /// use wildside_core::InterestProfile;
    ///
    /// let profile = InterestProfile::new();
    /// assert!(profile.weight(&wildside_core::Theme::Food).is_none());
    /// ```
    pub fn new() -> Self {
        Self {
            weights: HashMap::new(),
        }
    }

    /// Return the weight for a theme, if present.
    ///
    /// # Examples
    /// ```
    /// use wildside_core::{InterestProfile, Theme};
    ///
    /// let profile = InterestProfile::new().with_weight(Theme::Art, 0.5);
    /// assert_eq!(profile.weight(&Theme::Art), Some(0.5));
    /// assert!(profile.weight(&Theme::History).is_none());
    /// ```
    pub fn weight(&self, theme: &Theme) -> Option<f32> {
        self.weights.get(theme).copied()
    }

    /// Insert or update a theme weight.
    ///
    /// # Examples
    /// ```
    /// use wildside_core::{InterestProfile, Theme};
    ///
    /// let mut profile = InterestProfile::new();
    /// profile.set_weight(Theme::Shopping, 0.7);
    /// assert_eq!(profile.weight(&Theme::Shopping), Some(0.7));
    /// ```
    pub fn set_weight(&mut self, theme: Theme, weight: f32) {
        self.weights.insert(theme, weight);
    }

    /// Add a theme weight while returning `self` for chaining.
    ///
    /// # Examples
    /// ```
    /// use wildside_core::{InterestProfile, Theme};
    ///
    /// let profile = InterestProfile::new().with_weight(Theme::History, 0.8);
    /// assert_eq!(profile.weight(&Theme::History), Some(0.8));
    /// ```
    pub fn with_weight(mut self, theme: Theme, weight: f32) -> Self {
        self.set_weight(theme, weight);
        self
    }
}

/// An ordered path through points of interest with an overall duration.
///
/// # Examples
/// ```
/// use geo::Coord;
/// use std::time::Duration;
/// use wildside_core::{PointOfInterest, Route};
///
/// let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
/// let route = Route::new(vec![poi], Duration::from_secs(60));
///
/// assert_eq!(route.pois.len(), 1);
/// assert_eq!(route.total_duration.as_secs(), 60);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub pois: Vec<PointOfInterest>,
    pub total_duration: Duration,
}

impl Route {
    /// Construct a route from points and total duration.
    ///
    /// # Examples
    /// ```
    /// use geo::Coord;
    /// use std::time::Duration;
    /// use wildside_core::{PointOfInterest, Route};
    ///
    /// let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    /// let route = Route::new(vec![poi.clone()], Duration::from_secs(30));
    /// assert_eq!(route.pois, vec![poi]);
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
    /// ```
    /// use wildside_core::Route;
    ///
    /// let route = Route::empty();
    /// assert!(route.pois.is_empty());
    /// assert_eq!(route.total_duration.as_secs(), 0);
    /// ```
    pub fn empty() -> Self {
        Self::new(Vec::new(), Duration::ZERO)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    fn poi_stores_tags() {
        let poi = PointOfInterest::new(
            1,
            Coord { x: 0.0, y: 0.0 },
            HashMap::from([("key".into(), "value".into())]),
        );
        assert_eq!(poi.tags.get("key"), Some(&"value".to_string()));
    }

    #[rstest]
    #[case(Theme::History, Some(0.5))]
    #[case(Theme::Art, None)]
    fn interest_lookup(#[case] theme: Theme, #[case] expected: Option<f32>) {
        let profile = InterestProfile::new().with_weight(Theme::History, 0.5);
        assert_eq!(profile.weight(&theme), expected);
    }

    #[rstest]
    fn multiple_theme_lookup() {
        let mut profile = InterestProfile::new();
        profile.set_weight(Theme::Food, 0.8);
        profile.set_weight(Theme::Nature, 0.5);
        profile.set_weight(Theme::Art, 0.3);

        assert_eq!(profile.weight(&Theme::Food), Some(0.8));
        assert_eq!(profile.weight(&Theme::Nature), Some(0.5));
        assert_eq!(profile.weight(&Theme::Art), Some(0.3));
        assert!(profile.weight(&Theme::Shopping).is_none());
    }

    #[rstest]
    fn empty_profile_returns_none() {
        let profile = InterestProfile::new();
        assert!(profile.weight(&Theme::Nature).is_none());
    }

    #[fixture]
    fn two_pois() -> (PointOfInterest, PointOfInterest) {
        let poi1 = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
        let poi2 = PointOfInterest::with_empty_tags(2, Coord { x: 1.0, y: 1.0 });
        (poi1, poi2)
    }

    #[rstest]
    fn route_preserves_order(two_pois: (PointOfInterest, PointOfInterest)) {
        let (poi1, poi2) = two_pois;
        let route = Route::new(vec![poi1.clone(), poi2.clone()], Duration::from_secs(120));
        assert_eq!(route.pois, vec![poi1, poi2]);
        assert_eq!(route.total_duration.as_secs(), 120);
    }

    #[rstest]
    fn empty_route_has_zero_duration() {
        let route = Route::empty();
        assert!(route.pois.is_empty());
        assert_eq!(route.total_duration.as_secs(), 0);
    }
}
