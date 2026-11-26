//! Conversion of transport and OSRM payloads into travel-time results.

use std::time::Duration;

use wildside_core::{TravelTimeError, TravelTimeMatrix};

use super::osrm::TableResponse;
use super::provider::HttpTravelTimeProvider;

impl HttpTravelTimeProvider {
    /// Convert a reqwest error to a `TravelTimeError`.
    pub(super) fn convert_reqwest_error(
        &self,
        error: &reqwest::Error,
        url: &str,
    ) -> TravelTimeError {
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
    pub(super) fn convert_response(
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
