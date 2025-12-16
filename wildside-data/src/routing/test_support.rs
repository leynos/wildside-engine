//! Test utilities for routing providers.
//!
//! This module provides [`StubTravelTimeProvider`], a deterministic test double
//! for [`TravelTimeProvider`] that returns pre-configured responses without
//! making actual HTTP requests.

use std::time::Duration;

use wildside_core::{PointOfInterest, TravelTimeError, TravelTimeMatrix, TravelTimeProvider};

/// Stub `TravelTimeProvider` for testing.
///
/// This provider returns pre-configured responses, allowing tests to verify
/// behaviour without requiring a running OSRM service.
///
/// # Example
///
/// ```
/// use std::time::Duration;
/// use wildside_data::routing::test_support::StubTravelTimeProvider;
/// use wildside_core::{PointOfInterest, TravelTimeProvider, TravelTimeError};
/// use geo::Coord;
///
/// // Create a provider that returns a specific matrix
/// let matrix = vec![
///     vec![Duration::ZERO, Duration::from_secs(60)],
///     vec![Duration::from_secs(60), Duration::ZERO],
/// ];
/// let provider = StubTravelTimeProvider::with_matrix(matrix);
///
/// let pois = vec![
///     PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 }),
///     PointOfInterest::with_empty_tags(2, Coord { x: 1.0, y: 1.0 }),
/// ];
///
/// let result = provider.get_travel_time_matrix(&pois);
/// assert!(result.is_ok());
/// ```
#[derive(Debug, Clone)]
pub struct StubTravelTimeProvider {
    response: StubResponse,
}

#[derive(Debug, Clone)]
enum StubResponse {
    Matrix(TravelTimeMatrix),
    Error(TravelTimeError),
}

impl StubTravelTimeProvider {
    /// Create a provider that returns the given matrix.
    ///
    /// The matrix will be returned regardless of the POIs provided,
    /// as long as the input is non-empty.
    #[must_use]
    pub fn with_matrix(matrix: TravelTimeMatrix) -> Self {
        Self {
            response: StubResponse::Matrix(matrix),
        }
    }

    /// Create a provider that returns the given error.
    ///
    /// The error will be returned for any non-empty input.
    /// Empty input still returns `TravelTimeError::EmptyInput`.
    #[must_use]
    pub fn with_error(error: TravelTimeError) -> Self {
        Self {
            response: StubResponse::Error(error),
        }
    }

    /// Create a provider returning a unit matrix of the given size.
    ///
    /// The matrix has zero on the diagonal and one second for all
    /// off-diagonal entries, matching the pattern used by
    /// `UnitTravelTimeProvider` in `wildside-core`.
    #[must_use]
    pub fn with_unit_matrix(size: usize) -> Self {
        Self::with_matrix(build_unit_matrix(size))
    }
}

/// Build a unit travel time matrix of the given size.
///
/// Returns a matrix with zero on the diagonal and one second for off-diagonal.
fn build_unit_matrix(size: usize) -> TravelTimeMatrix {
    (0..size).map(|i| build_unit_row(size, i)).collect()
}

/// Build a single row of a unit travel time matrix.
fn build_unit_row(size: usize, row_index: usize) -> Vec<Duration> {
    (0..size).map(|j| unit_duration(row_index, j)).collect()
}

/// Return the unit duration for cell (i, j): zero on diagonal, one second otherwise.
fn unit_duration(i: usize, j: usize) -> Duration {
    if i == j {
        Duration::ZERO
    } else {
        Duration::from_secs(1)
    }
}

impl TravelTimeProvider for StubTravelTimeProvider {
    fn get_travel_time_matrix(
        &self,
        pois: &[PointOfInterest],
    ) -> Result<TravelTimeMatrix, TravelTimeError> {
        if pois.is_empty() {
            return Err(TravelTimeError::EmptyInput);
        }

        match &self.response {
            StubResponse::Matrix(matrix) => Ok(matrix.clone()),
            StubResponse::Error(error) => Err(error.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::Coord;
    use rstest::rstest;

    fn sample_pois(count: usize) -> Vec<PointOfInterest> {
        (0..count)
            .map(|i| PointOfInterest::with_empty_tags(i as u64, Coord { x: 0.0, y: 0.0 }))
            .collect()
    }

    #[rstest]
    fn with_matrix_returns_configured_matrix() {
        let matrix = vec![
            vec![Duration::ZERO, Duration::from_secs(60)],
            vec![Duration::from_secs(60), Duration::ZERO],
        ];
        let provider = StubTravelTimeProvider::with_matrix(matrix.clone());

        let result = provider
            .get_travel_time_matrix(&sample_pois(2))
            .expect("should succeed");

        assert_eq!(result, matrix);
    }

    #[rstest]
    fn with_error_returns_configured_error() {
        let provider = StubTravelTimeProvider::with_error(TravelTimeError::NetworkError {
            url: "http://example.com".to_string(),
            message: "connection refused".to_string(),
        });

        let err = provider
            .get_travel_time_matrix(&sample_pois(2))
            .expect_err("should fail");

        assert!(matches!(err, TravelTimeError::NetworkError { .. }));
    }

    #[rstest]
    fn empty_input_returns_empty_input_error() {
        let provider = StubTravelTimeProvider::with_unit_matrix(2);

        let err = provider
            .get_travel_time_matrix(&[])
            .expect_err("should fail");

        assert_eq!(err, TravelTimeError::EmptyInput);
    }

    #[rstest]
    fn with_unit_matrix_creates_correct_pattern() {
        let provider = StubTravelTimeProvider::with_unit_matrix(3);

        let matrix = provider
            .get_travel_time_matrix(&sample_pois(3))
            .expect("should succeed");

        assert_eq!(matrix.len(), 3);
        for (i, row) in matrix.iter().enumerate() {
            assert_eq!(row.len(), 3);
            for (j, &cell) in row.iter().enumerate() {
                if i == j {
                    assert_eq!(cell, Duration::ZERO);
                } else {
                    assert_eq!(cell, Duration::from_secs(1));
                }
            }
        }
    }
}
