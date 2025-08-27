//! Compute travel times between points of interest.
//!
//! The `TravelTimeProvider` trait abstracts the retrieval of pairwise travel
//! times between [`PointOfInterest`] instances. Callers supply a slice of POIs
//! and receive an adjacency matrix of [`Duration`] values.
//!
//! Errors are returned when inputs are invalid, e.g. an empty slice.

use std::time::Duration;
use thiserror::Error;

use crate::PointOfInterest;

/// Errors from [`TravelTimeProvider::get_travel_time_matrix`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TravelTimeError {
    /// No points of interest were provided.
    ///
    /// The provider requires at least one POI to compute a matrix. Callers
    /// should pre-filter input to avoid this condition.
    #[error("at least one point of interest is required")]
    EmptyInput,
}

/// Fetch pairwise travel times for a set of POIs.
///
/// Implementers are expected to return a square `n x n` matrix where `n` equals
/// the number of input POIs. `matrix[i][j]` represents the travel time from
/// `pois[i]` to `pois[j]`.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use geo::Coord;
/// use wildside_core::{PointOfInterest, TravelTimeProvider};
/// use wildside_core::travel_time::{TravelTimeError, TravelTimeProvider as _};
///
/// struct UnitProvider;
///
/// impl TravelTimeProvider for UnitProvider {
///     fn get_travel_time_matrix(
///         &self,
///         pois: &[PointOfInterest],
///     ) -> Result<Vec<Vec<Duration>>, TravelTimeError> {
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
///             .collect::<Vec<_>>())
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
    fn get_travel_time_matrix(
        &self,
        pois: &[PointOfInterest],
    ) -> Result<Vec<Vec<Duration>>, TravelTimeError>;
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
        let matrix = provider.get_travel_time_matrix(&pois).unwrap();
        assert_eq!(matrix.len(), pois.len());
        assert!(matrix.iter().all(|row| row.len() == pois.len()));
        assert_eq!(matrix[0][0], Duration::ZERO);
        assert_eq!(matrix[0][1], Duration::from_secs(1));
    }

    #[rstest]
    fn errors_on_empty_input() {
        let provider = UnitTravelTimeProvider;
        let err = provider.get_travel_time_matrix(&[]).unwrap_err();
        assert_eq!(err, TravelTimeError::EmptyInput);
    }
}
