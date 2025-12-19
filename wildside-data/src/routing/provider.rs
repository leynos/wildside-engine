//! HTTP-based `TravelTimeProvider` using OSRM's Table API.
//!
//! This module provides [`HttpTravelTimeProvider`], an implementation of the
//! [`TravelTimeProvider`] trait that fetches travel time matrices from an OSRM
//! routing service via HTTP.
//!
//! # Architecture
//!
//! The [`TravelTimeProvider`] trait is synchronous to keep the core library
//! embeddable in synchronous contexts. This provider bridges the async HTTP
//! calls to the sync interface by blocking on a Tokio runtime internally.
//!
//! # Example
//!
//! ```no_run
//! use wildside_data::routing::HttpTravelTimeProvider;
//! use wildside_core::{PointOfInterest, TravelTimeProvider};
//! use geo::Coord;
//!
//! let provider = HttpTravelTimeProvider::new("http://localhost:5000")?;
//! let pois = vec![
//!     PointOfInterest::with_empty_tags(1, Coord { x: -0.1, y: 51.5 }),
//!     PointOfInterest::with_empty_tags(2, Coord { x: -0.2, y: 51.6 }),
//! ];
//!
//! let matrix = provider.get_travel_time_matrix(&pois)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::time::Duration;

use reqwest::Client;
use tokio::runtime::{Handle, Runtime, RuntimeFlavor};
use wildside_core::{PointOfInterest, TravelTimeError, TravelTimeMatrix, TravelTimeProvider};

use super::osrm::TableResponse;

/// Error type for [`HttpTravelTimeProvider`] construction failures.
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

/// HTTP-based travel time provider using OSRM Table API.
///
/// This provider implements the synchronous [`TravelTimeProvider`] trait
/// by internally blocking on asynchronous HTTP requests. It owns a Tokio
/// runtime that is reused across calls, avoiding the overhead of creating
/// a new runtime per request.
///
/// # Runtime behaviour
///
/// When called from outside any Tokio runtime, the provider uses its own
/// stored runtime. When called from within an existing multi-threaded Tokio
/// runtime (detected via [`Handle::try_current()`] and
/// [`RuntimeFlavor::MultiThread`]), it uses that runtime's handle with
/// [`tokio::task::block_in_place`] to avoid nested runtime panics.
///
/// When called from within a `current_thread` Tokio runtime, the provider
/// falls back to using its own internal runtime. This avoids the panic that
/// `block_in_place` would cause, but may lead to deadlocks if the caller's
/// runtime is driving IO or timers that this request depends on.
///
/// # Supported routing modes
///
/// The provider computes an n×n travel time matrix for all provided POIs.
/// Both round-trip and point-to-point routing are supported; the routing
/// mode is determined by the caller (solver) which includes synthetic
/// start/end POIs in the request as needed.
pub struct HttpTravelTimeProvider {
    client: Client,
    config: HttpTravelTimeProviderConfig,
    runtime: Runtime,
}

impl std::fmt::Debug for HttpTravelTimeProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpTravelTimeProvider")
            .field("client", &self.client)
            .field("config", &self.config)
            .field("runtime", &"<tokio::runtime::Runtime>")
            .finish()
    }
}

impl HttpTravelTimeProvider {
    /// Create a new provider with default configuration.
    ///
    /// # Arguments
    ///
    /// * `base_url` - Base URL for the OSRM service (e.g., `"http://localhost:5000"`)
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client or Tokio runtime fails to build.
    pub fn new(base_url: impl Into<String>) -> Result<Self, ProviderBuildError> {
        Self::with_config(HttpTravelTimeProviderConfig::new(base_url))
    }

    /// Create a new provider with explicit configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client or Tokio runtime fails to build.
    pub fn with_config(config: HttpTravelTimeProviderConfig) -> Result<Self, ProviderBuildError> {
        let client = Client::builder()
            .user_agent(&config.user_agent)
            .connect_timeout(config.timeout)
            .timeout(config.timeout)
            .build()
            .map_err(ProviderBuildError::HttpClient)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(ProviderBuildError::Runtime)?;
        Ok(Self {
            client,
            config,
            runtime,
        })
    }

    /// Build the OSRM Table API URL for the given POIs.
    ///
    /// The URL format is: `{base_url}/table/v1/walking/{coordinates}`
    /// where coordinates are semicolon-separated `lon,lat` pairs.
    fn build_table_url(&self, pois: &[PointOfInterest]) -> String {
        let coords: String = pois
            .iter()
            .map(|poi| format!("{},{}", poi.location.x, poi.location.y))
            .collect::<Vec<_>>()
            .join(";");

        format!(
            "{}/table/v1/walking/{}",
            self.config.base_url.trim_end_matches('/'),
            coords
        )
    }

    /// Fetch the travel time matrix asynchronously.
    async fn fetch_matrix_async(
        &self,
        pois: &[PointOfInterest],
    ) -> Result<TravelTimeMatrix, TravelTimeError> {
        let url = self.build_table_url(pois);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|err| self.convert_reqwest_error(&err, &url))?
            .error_for_status()
            .map_err(|err| self.convert_reqwest_error(&err, &url))?;

        let table_response: TableResponse =
            response
                .json()
                .await
                .map_err(|err| TravelTimeError::ParseError {
                    message: err.to_string(),
                })?;

        self.convert_response(table_response)
    }

    /// Convert a reqwest error to a `TravelTimeError`.
    fn convert_reqwest_error(&self, error: &reqwest::Error, url: &str) -> TravelTimeError {
        if error.is_timeout() {
            return TravelTimeError::Timeout {
                url: url.to_owned(),
                timeout_secs: self.config.timeout.as_secs(),
            };
        }

        if let Some(status) = error.status() {
            return TravelTimeError::HttpError {
                url: url.to_owned(),
                status: status.as_u16(),
                message: error.to_string(),
            };
        }

        TravelTimeError::NetworkError {
            url: url.to_owned(),
            message: error.to_string(),
        }
    }

    /// Convert an OSRM response to a `TravelTimeMatrix`.
    fn convert_response(
        &self,
        response: TableResponse,
    ) -> Result<TravelTimeMatrix, TravelTimeError> {
        if !response.is_ok() {
            return Err(TravelTimeError::ServiceError {
                code: response.code,
                message: response.message.unwrap_or_default(),
            });
        }

        let durations = response
            .durations
            .ok_or_else(|| TravelTimeError::ParseError {
                message: "OSRM response missing durations array".to_string(),
            })?;

        // Convert f64 seconds to Duration, treating null as Duration::MAX
        // to indicate unreachable pairs. Invalid values (negative, NaN, infinite)
        // are also treated as unreachable to avoid panics from Duration::from_secs_f64.
        let matrix = durations
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|cell| {
                        cell.filter(|&v| v >= 0.0 && v.is_finite())
                            .map_or(Duration::MAX, Duration::from_secs_f64)
                    })
                    .collect()
            })
            .collect();

        Ok(matrix)
    }
}

impl TravelTimeProvider for HttpTravelTimeProvider {
    /// Fetch the travel time matrix for the given POIs.
    ///
    /// # Runtime requirements
    ///
    /// When called from within an existing Tokio runtime, the runtime must be
    /// multi-threaded (`flavor = "multi_thread"`). If called from within a
    /// `current_thread` runtime, the method falls back to using its own
    /// internal runtime, which may block the caller's runtime and cause
    /// deadlocks if the caller's runtime is driving IO or timers needed by
    /// this request.
    fn get_travel_time_matrix(
        &self,
        pois: &[PointOfInterest],
    ) -> Result<TravelTimeMatrix, TravelTimeError> {
        if pois.is_empty() {
            return Err(TravelTimeError::EmptyInput);
        }

        // If we're already inside a Tokio runtime, check the runtime flavour.
        // block_in_place requires a multi-threaded runtime; for current_thread
        // runtimes we fall back to our own stored runtime.
        let future = self.fetch_matrix_async(pois);
        match Handle::try_current() {
            Ok(handle) if handle.runtime_flavor() == RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(|| handle.block_on(future))
            }
            // No runtime detected, or current_thread runtime: use our own runtime.
            _ => self.runtime.block_on(future),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::Coord;
    use rstest::{fixture, rstest};

    #[fixture]
    fn sample_pois() -> Vec<PointOfInterest> {
        vec![
            PointOfInterest::with_empty_tags(1, Coord { x: -0.1, y: 51.5 }),
            PointOfInterest::with_empty_tags(2, Coord { x: -0.2, y: 51.6 }),
        ]
    }

    #[rstest]
    fn build_table_url_formats_coordinates(sample_pois: Vec<PointOfInterest>) {
        let provider =
            HttpTravelTimeProvider::new("http://osrm.example.com").expect("provider should build");

        let url = provider.build_table_url(&sample_pois);

        assert_eq!(
            url,
            "http://osrm.example.com/table/v1/walking/-0.1,51.5;-0.2,51.6"
        );
    }

    #[rstest]
    fn build_table_url_strips_trailing_slash(sample_pois: Vec<PointOfInterest>) {
        let provider =
            HttpTravelTimeProvider::new("http://osrm.example.com/").expect("provider should build");

        let url = provider.build_table_url(&sample_pois);

        assert!(url.starts_with("http://osrm.example.com/table/"));
        assert!(!url.contains("//table"));
    }

    #[rstest]
    fn convert_response_handles_success() {
        let provider =
            HttpTravelTimeProvider::new("http://localhost:5000").expect("provider should build");
        let response = TableResponse {
            code: "Ok".to_string(),
            message: None,
            durations: Some(vec![
                vec![Some(0.0), Some(120.5)],
                vec![Some(120.5), Some(0.0)],
            ]),
        };

        let matrix = provider.convert_response(response).expect("should parse");

        assert_eq!(matrix.len(), 2);
        assert_eq!(matrix[0][0], Duration::ZERO);
        assert_eq!(matrix[0][1], Duration::from_secs_f64(120.5));
        assert_eq!(matrix[1][0], Duration::from_secs_f64(120.5));
        assert_eq!(matrix[1][1], Duration::ZERO);
    }

    #[rstest]
    fn convert_response_handles_null_durations() {
        let provider =
            HttpTravelTimeProvider::new("http://localhost:5000").expect("provider should build");
        let response = TableResponse {
            code: "Ok".to_string(),
            message: None,
            durations: Some(vec![vec![Some(0.0), None], vec![None, Some(0.0)]]),
        };

        let matrix = provider.convert_response(response).expect("should parse");

        assert_eq!(matrix[0][1], Duration::MAX);
        assert_eq!(matrix[1][0], Duration::MAX);
    }

    #[rstest]
    fn convert_response_handles_invalid_durations() {
        let provider =
            HttpTravelTimeProvider::new("http://localhost:5000").expect("provider should build");
        let response = TableResponse {
            code: "Ok".to_string(),
            message: None,
            durations: Some(vec![
                vec![Some(0.0), Some(-1.0), Some(f64::NAN)],
                vec![Some(f64::INFINITY), Some(0.0), Some(f64::NEG_INFINITY)],
                vec![Some(100.0), Some(200.0), Some(0.0)],
            ]),
        };

        let matrix = provider.convert_response(response).expect("should parse");

        // Negative values become Duration::MAX
        assert_eq!(matrix[0][1], Duration::MAX);
        // NaN becomes Duration::MAX
        assert_eq!(matrix[0][2], Duration::MAX);
        // Positive infinity becomes Duration::MAX
        assert_eq!(matrix[1][0], Duration::MAX);
        // Negative infinity becomes Duration::MAX
        assert_eq!(matrix[1][2], Duration::MAX);
        // Valid values are converted correctly
        assert_eq!(matrix[2][0], Duration::from_secs(100));
        assert_eq!(matrix[2][1], Duration::from_secs(200));
    }

    #[rstest]
    fn convert_response_handles_service_error() {
        let provider =
            HttpTravelTimeProvider::new("http://localhost:5000").expect("provider should build");
        let response = TableResponse {
            code: "InvalidQuery".to_string(),
            message: Some("Too many coordinates".to_string()),
            durations: None,
        };

        let err = provider
            .convert_response(response)
            .expect_err("should fail");

        match err {
            TravelTimeError::ServiceError { code, message } => {
                assert_eq!(code, "InvalidQuery");
                assert_eq!(message, "Too many coordinates");
            }
            _ => panic!("expected ServiceError, got {err:?}"),
        }
    }

    #[rstest]
    fn convert_response_handles_missing_durations() {
        let provider =
            HttpTravelTimeProvider::new("http://localhost:5000").expect("provider should build");
        let response = TableResponse {
            code: "Ok".to_string(),
            message: None,
            durations: None,
        };

        let err = provider
            .convert_response(response)
            .expect_err("should fail");

        assert!(matches!(err, TravelTimeError::ParseError { .. }));
    }

    #[rstest]
    fn empty_input_returns_error() {
        let provider =
            HttpTravelTimeProvider::new("http://localhost:5000").expect("provider should build");

        let err = provider
            .get_travel_time_matrix(&[])
            .expect_err("should fail");

        assert_eq!(err, TravelTimeError::EmptyInput);
    }

    #[rstest]
    fn config_builder_pattern() {
        let config = HttpTravelTimeProviderConfig::new("http://example.com")
            .with_timeout(Duration::from_secs(60))
            .with_user_agent("test-agent/1.0");

        assert_eq!(config.base_url, "http://example.com");
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.user_agent, "test-agent/1.0");
    }
}
