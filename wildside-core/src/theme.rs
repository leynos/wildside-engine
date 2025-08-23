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
        match s.to_lowercase().as_str() {
            "history" => Ok(Self::History),
            "art" => Ok(Self::Art),
            "nature" => Ok(Self::Nature),
            "food" => Ok(Self::Food),
            "architecture" => Ok(Self::Architecture),
            "shopping" => Ok(Self::Shopping),
            "entertainment" => Ok(Self::Entertainment),
            "culture" => Ok(Self::Culture),
            _ => Err(format!("unknown theme '{s}'")),
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
}
