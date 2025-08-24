//! Themes describing broad categories of interest.
//!
//! The enum offers compile-time safety for interest lookups.
//!
//! # Examples
//! ```
//! use wildside_core::Theme;
//!
//! assert_eq!(Theme::History.as_str(), "history");
//! assert_eq!(Theme::Art.to_string(), "art");
//! ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Theme {
    /// Historical attractions.
    History,
    /// Artistic venues and galleries.
    Art,
    /// Natural landscapes and parks.
    Nature,
    /// Food and cuisine experiences.
    Food,
    /// Architectural landmarks.
    Architecture,
    /// Shopping districts and markets.
    Shopping,
    /// Entertainment and nightlife.
    Entertainment,
    /// Cultural centres and events.
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

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Theme {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("history") {
            Ok(Self::History)
        } else if s.eq_ignore_ascii_case("art") {
            Ok(Self::Art)
        } else if s.eq_ignore_ascii_case("nature") {
            Ok(Self::Nature)
        } else if s.eq_ignore_ascii_case("food") {
            Ok(Self::Food)
        } else if s.eq_ignore_ascii_case("architecture") {
            Ok(Self::Architecture)
        } else if s.eq_ignore_ascii_case("shopping") {
            Ok(Self::Shopping)
        } else if s.eq_ignore_ascii_case("entertainment") {
            Ok(Self::Entertainment)
        } else if s.eq_ignore_ascii_case("culture") {
            Ok(Self::Culture)
        } else {
            Err(format!("unknown theme '{s}'"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn display_matches_as_str() {
        assert_eq!(Theme::Art.to_string(), Theme::Art.as_str());
    }

    #[test]
    fn parsing_rejects_unknown() {
        let err = Theme::from_str("unknown").unwrap_err();
        assert!(err.contains("unknown theme"));
    }

    #[test]
    fn parses_case_insensitively() {
        assert_eq!(Theme::from_str("HiStOrY").expect("parse"), Theme::History);
        assert_eq!(Theme::from_str("ART").expect("parse"), Theme::Art);
    }
}
