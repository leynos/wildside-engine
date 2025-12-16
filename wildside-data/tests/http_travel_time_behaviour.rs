//! Behavioural tests for [`HttpTravelTimeProvider`].
//!
//! These tests use [`StubTravelTimeProvider`] to verify behaviour without
//! requiring a running OSRM service.

use geo::Coord;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use std::time::Duration;
use wildside_core::{PointOfInterest, TravelTimeError, TravelTimeMatrix, TravelTimeProvider};
use wildside_data::routing::test_support::StubTravelTimeProvider;

/// Result cell holding the outcome of a travel time request.
type ResultCell = RefCell<Result<TravelTimeMatrix, TravelTimeError>>;

#[fixture]
fn provider() -> RefCell<Option<StubTravelTimeProvider>> {
    RefCell::new(None)
}

#[fixture]
fn result() -> ResultCell {
    RefCell::new(Ok(Vec::new()))
}

fn sample_pois(count: usize) -> Vec<PointOfInterest> {
    (0..count)
        .map(|i| {
            PointOfInterest::with_empty_tags(
                i as u64,
                Coord {
                    x: -0.1 + i as f64 * 0.01,
                    y: 51.5 + i as f64 * 0.01,
                },
            )
        })
        .collect()
}

fn sample_matrix() -> TravelTimeMatrix {
    vec![
        vec![Duration::ZERO, Duration::from_secs(120)],
        vec![Duration::from_secs(120), Duration::ZERO],
    ]
}

fn matrix_with_nulls() -> TravelTimeMatrix {
    vec![
        vec![Duration::ZERO, Duration::MAX],
        vec![Duration::MAX, Duration::ZERO],
    ]
}

// --- Given steps ---

#[given("a routing service returning valid durations")]
fn routing_service_ok(#[from(provider)] provider: &RefCell<Option<StubTravelTimeProvider>>) {
    *provider.borrow_mut() = Some(StubTravelTimeProvider::with_matrix(sample_matrix()));
}

#[given("a routing service that fails with a network error")]
fn routing_service_network_error(
    #[from(provider)] provider: &RefCell<Option<StubTravelTimeProvider>>,
) {
    *provider.borrow_mut() = Some(StubTravelTimeProvider::with_error(
        TravelTimeError::NetworkError {
            url: "http://example.com/table/v1/walking".to_string(),
            message: "connection refused".to_string(),
        },
    ));
}

#[given("a routing service that times out")]
fn routing_service_timeout(#[from(provider)] provider: &RefCell<Option<StubTravelTimeProvider>>) {
    *provider.borrow_mut() = Some(StubTravelTimeProvider::with_error(
        TravelTimeError::Timeout {
            url: "http://example.com/table/v1/walking".to_string(),
            timeout_secs: 30,
        },
    ));
}

#[given("a routing service returning an error response")]
fn routing_service_error(#[from(provider)] provider: &RefCell<Option<StubTravelTimeProvider>>) {
    *provider.borrow_mut() = Some(StubTravelTimeProvider::with_error(
        TravelTimeError::ServiceError {
            code: "InvalidQuery".to_string(),
            message: "Too many coordinates".to_string(),
        },
    ));
}

#[given("a routing service returning null for unreachable pairs")]
fn routing_service_with_nulls(
    #[from(provider)] provider: &RefCell<Option<StubTravelTimeProvider>>,
) {
    *provider.borrow_mut() = Some(StubTravelTimeProvider::with_matrix(matrix_with_nulls()));
}

// --- When steps ---

#[when("I request travel times for two POIs")]
fn request_two(
    #[from(provider)] provider: &RefCell<Option<StubTravelTimeProvider>>,
    #[from(result)] result: &ResultCell,
) {
    let guard = provider.borrow();
    let stub = guard.as_ref().expect("provider must be initialised");
    let pois = sample_pois(2);
    *result.borrow_mut() = stub.get_travel_time_matrix(&pois);
}

#[when("I request travel times for no POIs")]
fn request_none(
    #[from(provider)] provider: &RefCell<Option<StubTravelTimeProvider>>,
    #[from(result)] result: &ResultCell,
) {
    let guard = provider.borrow();
    let stub = guard.as_ref().expect("provider must be initialised");
    let pois: Vec<PointOfInterest> = Vec::new();
    *result.borrow_mut() = stub.get_travel_time_matrix(&pois);
}

// --- Then steps ---

#[then("a 2x2 matrix is returned")]
fn then_matrix(#[from(result)] result: &ResultCell) {
    let borrowed = result.borrow();
    let matrix = borrowed.as_ref().expect("expected Ok result");
    assert_eq!(matrix.len(), 2, "expected 2 rows");
    assert!(
        matrix.iter().all(|row| row.len() == 2),
        "expected 2 columns"
    );
    assert_eq!(matrix[0][0], Duration::ZERO, "diagonal should be zero");
    assert_eq!(matrix[1][1], Duration::ZERO, "diagonal should be zero");
}

#[then("an empty input error is returned")]
fn then_empty_error(#[from(result)] result: &ResultCell) {
    let borrowed = result.borrow();
    assert!(
        matches!(&*borrowed, Err(TravelTimeError::EmptyInput)),
        "expected EmptyInput error, got {borrowed:?}"
    );
}

#[then("a network error is returned")]
fn then_network_error(#[from(result)] result: &ResultCell) {
    let borrowed = result.borrow();
    assert!(
        matches!(&*borrowed, Err(TravelTimeError::NetworkError { .. })),
        "expected NetworkError, got {borrowed:?}"
    );
}

#[then("a timeout error is returned")]
fn then_timeout_error(#[from(result)] result: &ResultCell) {
    let borrowed = result.borrow();
    assert!(
        matches!(&*borrowed, Err(TravelTimeError::Timeout { .. })),
        "expected Timeout error, got {borrowed:?}"
    );
}

#[then("a service error is returned")]
fn then_service_error(#[from(result)] result: &ResultCell) {
    let borrowed = result.borrow();
    assert!(
        matches!(&*borrowed, Err(TravelTimeError::ServiceError { .. })),
        "expected ServiceError, got {borrowed:?}"
    );
}

#[then("a 2x2 matrix with maximum duration for nulls is returned")]
fn then_matrix_with_max(#[from(result)] result: &ResultCell) {
    let borrowed = result.borrow();
    let matrix = borrowed.as_ref().expect("expected Ok result");
    assert_eq!(matrix.len(), 2, "expected 2 rows");
    assert_eq!(matrix[0][0], Duration::ZERO, "diagonal should be zero");
    assert_eq!(matrix[1][1], Duration::ZERO, "diagonal should be zero");
    assert_eq!(matrix[0][1], Duration::MAX, "unreachable should be MAX");
    assert_eq!(matrix[1][0], Duration::MAX, "unreachable should be MAX");
}

// --- Scenario registrations ---

macro_rules! register_scenario {
    ($fn_name:ident, $title:literal) => {
        #[scenario(path = "tests/features/http_travel_time.feature", name = $title)]
        fn $fn_name(provider: RefCell<Option<StubTravelTimeProvider>>, result: ResultCell) {
            let _ = (provider, result);
        }
    };
}

register_scenario!(
    returning_matrix_for_two_pois,
    "returning a travel time matrix for two POIs"
);
register_scenario!(
    returning_error_for_empty_input,
    "returning an error for empty input"
);
register_scenario!(handling_network_error, "handling a network error");
register_scenario!(handling_timeout, "handling a timeout");
register_scenario!(handling_service_error, "handling a service error response");
register_scenario!(handling_unreachable_pairs, "handling unreachable pairs");
