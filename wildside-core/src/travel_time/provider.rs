//! Travel-time provider trait and adjacency-matrix alias for POI pairs.

use std::time::Duration;

use crate::PointOfInterest;

use super::error::TravelTimeError;

/// Adjacency matrix of travel times.
pub type TravelTimeMatrix = Vec<Vec<Duration>>;

/// Fetch pairwise travel times for a set of POIs.
///
/// Implementers must return a square `n×n` matrix where `n == pois.len()`.
/// `matrix[i][j]` is the travel time from `pois[i]` to `pois[j]`.
///
/// # Examples
///
/// ```rust
/// use std::time::Duration;
/// use geo::Coord;
/// use wildside_core::{PointOfInterest, TravelTimeError, TravelTimeMatrix, TravelTimeProvider};
///
/// struct UnitProvider;
///
/// impl TravelTimeProvider for UnitProvider {
///     fn get_travel_time_matrix(
///         &self,
///         pois: &[PointOfInterest],
///     ) -> Result<TravelTimeMatrix, TravelTimeError> {
///         if pois.is_empty() {
///             return Err(TravelTimeError::EmptyInput);
///         }
///         let n = pois.len();
///         Ok((0..n)
///             .map(|i| {
///                 (0..n)
///                     .map(|j| if i == j { Duration::ZERO } else { Duration::from_secs(1) })
///                     .collect::<Vec<_>>()
///             })
///             .collect())
///     }
/// }
///
/// let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
/// let matrix = UnitProvider.get_travel_time_matrix(&[poi])?;
/// assert_eq!(matrix.len(), 1);
/// # Ok::<(), TravelTimeError>(())
/// ```
pub trait TravelTimeProvider {
    /// Return a matrix of travel times for `pois`.
    ///
    /// Implementations must return `Err(TravelTimeError::EmptyInput)` when
    /// `pois` is empty.
    fn get_travel_time_matrix(
        &self,
        pois: &[PointOfInterest],
    ) -> Result<TravelTimeMatrix, TravelTimeError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::Coord;
    use rstest::rstest;

    use crate::test_support::UnitTravelTimeProvider;

    fn sample_pois() -> Vec<PointOfInterest> {
        vec![
            PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 }),
            PointOfInterest::with_empty_tags(2, Coord { x: 1.0, y: 1.0 }),
        ]
    }

    #[rstest]
    fn returns_square_matrix() {
        let provider = UnitTravelTimeProvider;
        let pois = sample_pois();
        let matrix = provider
            .get_travel_time_matrix(&pois)
            .expect("expected square matrix from UnitTravelTimeProvider");
        assert_eq!(matrix.len(), pois.len());
        assert!(matrix.iter().all(|row| row.len() == pois.len()));
        assert_eq!(matrix[0][0], Duration::ZERO);
        assert_eq!(matrix[0][1], Duration::from_secs(1));
    }

    #[rstest]
    fn errors_on_empty_input() {
        let provider = UnitTravelTimeProvider;
        let err = provider
            .get_travel_time_matrix(&[])
            .expect_err("expected EmptyInput for empty slice");
        assert_eq!(err, TravelTimeError::EmptyInput);
    }
}
