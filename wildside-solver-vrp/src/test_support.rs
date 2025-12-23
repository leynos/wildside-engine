//! Test-only utilities for `wildside-solver-vrp`.
//!
//! The helpers in this module are available to unit tests and behavioural
//! tests. They are gated behind the `test-support` feature (and `cfg(test)`).

use std::time::Duration;

use geo::Coord;
use wildside_core::{PointOfInterest, Tags, TravelTimeError, TravelTimeMatrix, TravelTimeProvider};

/// Construct a `PointOfInterest` tagged with a theme key.
///
/// # Examples
/// ```rust
/// use wildside_solver_vrp::test_support::poi;
///
/// let poi = poi(1, 0.0, 0.0, "art");
/// assert_eq!(poi.id, 1);
/// assert!(poi.tags.contains_key("art"));
/// ```
#[must_use]
pub fn poi(id: u64, x: f64, y: f64, theme: &str) -> PointOfInterest {
    PointOfInterest::new(
        id,
        Coord { x, y },
        Tags::from([(theme.to_owned(), String::new())]),
    )
}

/// A [`TravelTimeProvider`] returning a fixed, pre-defined matrix.
///
/// This provider enables fully deterministic golden route tests by returning
/// a caller-supplied travel time matrix verbatim. The matrix must match the
/// number of POIs passed to [`get_travel_time_matrix`]; dimension mismatches
/// produce a [`TravelTimeError::ServiceError`] with code `DIMENSION_MISMATCH`.
///
/// # Examples
///
/// ```rust
/// use std::time::Duration;
/// use geo::Coord;
/// use wildside_core::{PointOfInterest, TravelTimeProvider};
/// use wildside_solver_vrp::test_support::FixedMatrixTravelTimeProvider;
///
/// let matrix = vec![
///     vec![Duration::ZERO, Duration::from_secs(60)],
///     vec![Duration::from_secs(60), Duration::ZERO],
/// ];
/// let provider = FixedMatrixTravelTimeProvider::new(matrix);
///
/// let pois = vec![
///     PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 }),
///     PointOfInterest::with_empty_tags(2, Coord { x: 1.0, y: 0.0 }),
/// ];
/// let result = provider.get_travel_time_matrix(&pois);
/// assert!(result.is_ok());
/// ```
#[derive(Debug, Clone)]
pub struct FixedMatrixTravelTimeProvider {
    matrix: TravelTimeMatrix,
}

impl FixedMatrixTravelTimeProvider {
    /// Construct a provider from a pre-built travel time matrix.
    #[must_use]
    pub const fn new(matrix: TravelTimeMatrix) -> Self {
        Self { matrix }
    }

    /// Build from integer seconds for convenience in test fixtures.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wildside_solver_vrp::test_support::FixedMatrixTravelTimeProvider;
    ///
    /// let provider = FixedMatrixTravelTimeProvider::from_seconds(vec![
    ///     vec![0, 30, 60],
    ///     vec![30, 0, 45],
    ///     vec![60, 45, 0],
    /// ]);
    /// ```
    #[must_use]
    pub fn from_seconds(seconds: Vec<Vec<u64>>) -> Self {
        let matrix = seconds
            .into_iter()
            .map(|row| row.into_iter().map(Duration::from_secs).collect())
            .collect();
        Self { matrix }
    }
}

impl TravelTimeProvider for FixedMatrixTravelTimeProvider {
    fn get_travel_time_matrix(
        &self,
        pois: &[PointOfInterest],
    ) -> Result<TravelTimeMatrix, TravelTimeError> {
        if pois.is_empty() {
            return Err(TravelTimeError::EmptyInput);
        }
        let expected_dim = pois.len();
        if self.matrix.len() != expected_dim {
            return Err(TravelTimeError::ServiceError {
                code: "DIMENSION_MISMATCH".to_owned(),
                message: format!(
                    "matrix has {} rows but {} POIs provided",
                    self.matrix.len(),
                    expected_dim
                ),
            });
        }
        // Validate each row has the correct number of columns.
        for (row_idx, row) in self.matrix.iter().enumerate() {
            if row.len() != expected_dim {
                return Err(TravelTimeError::ServiceError {
                    code: "DIMENSION_MISMATCH".to_owned(),
                    message: format!(
                        "row {} has {} columns but {} expected (matrix must be square)",
                        row_idx,
                        row.len(),
                        expected_dim
                    ),
                });
            }
        }
        Ok(self.matrix.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[expect(clippy::indexing_slicing, reason = "test matrix has known dimensions")]
    fn from_seconds_creates_duration_matrix() {
        let provider = FixedMatrixTravelTimeProvider::from_seconds(vec![vec![0, 30], vec![30, 0]]);
        let pois = vec![
            PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 }),
            PointOfInterest::with_empty_tags(2, Coord { x: 1.0, y: 0.0 }),
        ];
        let matrix = provider
            .get_travel_time_matrix(&pois)
            .expect("matrix should be returned");
        assert_eq!(matrix[0][1], Duration::from_secs(30));
        assert_eq!(matrix[1][0], Duration::from_secs(30));
    }

    #[rstest]
    fn errors_on_empty_input() {
        let provider = FixedMatrixTravelTimeProvider::from_seconds(vec![vec![0]]);
        let err = provider
            .get_travel_time_matrix(&[])
            .expect_err("expected EmptyInput for empty slice");
        assert_eq!(err, TravelTimeError::EmptyInput);
    }

    #[rstest]
    fn errors_on_dimension_mismatch() {
        let provider = FixedMatrixTravelTimeProvider::from_seconds(vec![vec![0, 30], vec![30, 0]]);
        let pois = vec![
            PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 }),
            PointOfInterest::with_empty_tags(2, Coord { x: 1.0, y: 0.0 }),
            PointOfInterest::with_empty_tags(3, Coord { x: 2.0, y: 0.0 }),
        ];
        let err = provider
            .get_travel_time_matrix(&pois)
            .expect_err("expected dimension mismatch error");
        match err {
            TravelTimeError::ServiceError { code, .. } => {
                assert_eq!(code, "DIMENSION_MISMATCH");
            }
            _ => panic!("expected ServiceError with DIMENSION_MISMATCH"),
        }
    }

    #[rstest]
    fn errors_on_jagged_matrix() {
        // Matrix with mismatched row lengths (first row has 2 cols, second has 1).
        let provider = FixedMatrixTravelTimeProvider::from_seconds(vec![vec![0, 30], vec![30]]);
        let pois = vec![
            PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 }),
            PointOfInterest::with_empty_tags(2, Coord { x: 1.0, y: 0.0 }),
        ];
        let err = provider
            .get_travel_time_matrix(&pois)
            .expect_err("expected dimension mismatch for jagged matrix");
        match err {
            TravelTimeError::ServiceError { code, message } => {
                assert_eq!(code, "DIMENSION_MISMATCH");
                assert!(
                    message.contains("row 1"),
                    "error should identify the problematic row"
                );
            }
            _ => panic!("expected ServiceError with DIMENSION_MISMATCH"),
        }
    }

    #[rstest]
    fn errors_on_non_square_matrix() {
        // Matrix has correct row count but wrong column count in all rows.
        let provider =
            FixedMatrixTravelTimeProvider::from_seconds(vec![vec![0, 30, 60], vec![30, 0, 45]]);
        let pois = vec![
            PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 }),
            PointOfInterest::with_empty_tags(2, Coord { x: 1.0, y: 0.0 }),
        ];
        let err = provider
            .get_travel_time_matrix(&pois)
            .expect_err("expected dimension mismatch for non-square matrix");
        match err {
            TravelTimeError::ServiceError { code, message } => {
                assert_eq!(code, "DIMENSION_MISMATCH");
                assert!(
                    message.contains("must be square"),
                    "error should mention square requirement"
                );
            }
            _ => panic!("expected ServiceError with DIMENSION_MISMATCH"),
        }
    }
}
