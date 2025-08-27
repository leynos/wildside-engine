//! Behavioural tests for `TravelTimeProvider` implementations.

use geo::Coord;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use std::time::Duration;
use wildside_core::{
    PointOfInterest, TravelTimeError, TravelTimeProvider, test_support::UnitTravelTimeProvider,
};

#[fixture]
fn provider() -> UnitTravelTimeProvider {
    UnitTravelTimeProvider
}

#[fixture]
fn result() -> RefCell<Result<Vec<Vec<Duration>>, TravelTimeError>> {
    RefCell::new(Ok(Vec::new()))
}

#[given("a provider returning unit distances")]
fn given_provider(
    #[from(provider)] _provider: &UnitTravelTimeProvider,
    #[from(result)] result: &RefCell<Result<Vec<Vec<Duration>>, TravelTimeError>>,
) {
    *result.borrow_mut() = Ok(Vec::new());
}

#[when("I request travel times for two POIs")]
fn request_two(
    #[from(provider)] provider: &UnitTravelTimeProvider,
    #[from(result)] result: &RefCell<Result<Vec<Vec<Duration>>, TravelTimeError>>,
) {
    let pois = vec![
        PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 }),
        PointOfInterest::with_empty_tags(2, Coord { x: 1.0, y: 1.0 }),
    ];
    *result.borrow_mut() = provider.get_travel_time_matrix(&pois);
}

#[when("I request travel times for no POIs")]
fn request_none(
    #[from(provider)] provider: &UnitTravelTimeProvider,
    #[from(result)] result: &RefCell<Result<Vec<Vec<Duration>>, TravelTimeError>>,
) {
    let pois: Vec<PointOfInterest> = Vec::new();
    *result.borrow_mut() = provider.get_travel_time_matrix(&pois);
}

#[then("a 2x2 matrix is returned")]
fn then_matrix(#[from(result)] result: &RefCell<Result<Vec<Vec<Duration>>, TravelTimeError>>) {
    let borrow = result.borrow();
    let matrix = borrow.as_ref().expect("expected Ok result");
    assert_eq!(matrix.len(), 2);
    assert!(matrix.iter().all(|row| row.len() == 2));
}

#[then("an error is returned")]
fn then_error(#[from(result)] result: &RefCell<Result<Vec<Vec<Duration>>, TravelTimeError>>) {
    assert!(result.borrow().is_err(), "expected an error");
}

#[scenario(path = "tests/features/travel_time_provider.feature", index = 0)]
fn matrix_returned(
    provider: UnitTravelTimeProvider,
    result: RefCell<Result<Vec<Vec<Duration>>, TravelTimeError>>,
) {
    let _ = (provider, result);
}

#[scenario(path = "tests/features/travel_time_provider.feature", index = 1)]
fn error_on_empty(
    provider: UnitTravelTimeProvider,
    result: RefCell<Result<Vec<Vec<Duration>>, TravelTimeError>>,
) {
    let _ = (provider, result);
}
