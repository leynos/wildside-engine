//! Configuration and construction errors for [`HttpTravelTimeProvider`].
//!
//! [`HttpTravelTimeProvider`]: super::HttpTravelTimeProvider

use std::time::Duration;

/// Error type for [`HttpTravelTimeProvider`] construction failures.
///
/// [`HttpTravelTimeProvider`]: super::HttpTravelTimeProvider
#[derive(Debug)]
pub enum ProviderBuildError {
    /// Failed to build the HTTP client.
    HttpClient(reqwest::Error),
    /// Failed to build the Tokio runtime.
    Runtime(std::io::Error),
}

impl std::fmt::Display for ProviderBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HttpClient(err) => write!(f, "failed to build HTTP client: {err}"),
            Self::Runtime(err) => write!(f, "failed to build Tokio runtime: {err}"),
        }
    }
}

impl std::error::Error for ProviderBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HttpClient(err) => Some(err),
            Self::Runtime(err) => Some(err),
        }
    }
}

/// Default user agent for OSRM requests.
pub const DEFAULT_USER_AGENT: &str = "wildside-routing/0.1";

/// Default request timeout in seconds.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Configuration for [`HttpTravelTimeProvider`].
///
/// [`HttpTravelTimeProvider`]: super::HttpTravelTimeProvider
#[derive(Debug, Clone)]
pub struct HttpTravelTimeProviderConfig {
    /// Base URL for the OSRM service (e.g., `"http://localhost:5000"`).
    pub base_url: String,
    /// Request timeout duration.
    pub timeout: Duration,
    /// User agent string for requests.
    pub user_agent: String,
}

impl Default for HttpTravelTimeProviderConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:5000".to_string(),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            user_agent: DEFAULT_USER_AGENT.to_string(),
        }
    }
}

impl HttpTravelTimeProviderConfig {
    /// Create a new configuration with the given base URL.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            ..Default::default()
        }
    }

    /// Set the request timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the user agent string.
    #[must_use]
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }
}
