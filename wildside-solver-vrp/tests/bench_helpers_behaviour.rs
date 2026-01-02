//! Behavioural tests for benchmark helper functions using rstest-bdd.
//!
//! Tests the `generate_clustered_pois` and `generate_travel_time_matrix` functions
//! using BDD-style scenarios.

use std::cell::RefCell;
use std::time::Duration;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use wildside_core::PointOfInterest;

/// Include the benchmark support module from the benches directory.
/// The `dead_code` suppression is narrowly scoped to this module declaration
/// because bench_support exports items used by other test files but not all
/// items are used in this specific file.
#[expect(
    dead_code,
    reason = "bench_support exports items used by other test files"
)]
#[path = "../benches/bench_support.rs"]
mod bench_support;

use bench_support::{generate_clustered_pois, generate_travel_time_matrix};

/// World state for benchmark helper behavioural tests.
#[derive(Debug)]
struct BenchWorld {
    count: RefCell<usize>,
    seed: RefCell<u64>,
    pois: RefCell<Vec<PointOfInterest>>,
    pois_second: RefCell<Vec<PointOfInterest>>,
    matrix: RefCell<Vec<Vec<Duration>>>,
    matrix_second: RefCell<Vec<Vec<Duration>>>,
}

impl BenchWorld {
    #[expect(clippy::missing_const_for_fn, reason = "RefCell::new is not const")]
    fn new() -> Self {
        Self {
            count: RefCell::new(0),
            seed: RefCell::new(42),
            pois: RefCell::new(Vec::new()),
            pois_second: RefCell::new(Vec::new()),
            matrix: RefCell::new(Vec::new()),
            matrix_second: RefCell::new(Vec::new()),
        }
    }
}

#[fixture]
fn world() -> BenchWorld {
    BenchWorld::new()
}

// =============================================================================
// POI generation scenarios
// =============================================================================

#[given("a request for 50 POIs with seed 42")]
fn given_poi_request_50(world: &BenchWorld) {
    world.count.replace(50);
    world.seed.replace(42);
}

#[given("a request for 20 POIs with seed 100")]
fn given_poi_request_20(world: &BenchWorld) {
    world.count.replace(20);
    world.seed.replace(100);
}

#[when("POIs are generated")]
fn when_pois_generated(world: &BenchWorld) {
    let count = *world.count.borrow();
    let seed = *world.seed.borrow();
    let pois = generate_clustered_pois(count, seed);
    world.pois.replace(pois);
}

#[when("POIs are generated twice with the same seed")]
fn when_pois_generated_twice(world: &BenchWorld) {
    let count = *world.count.borrow();
    let seed = *world.seed.borrow();
    let pois1 = generate_clustered_pois(count, seed);
    let pois2 = generate_clustered_pois(count, seed);
    world.pois.replace(pois1);
    world.pois_second.replace(pois2);
}

#[then("50 POIs are returned")]
fn then_50_pois_returned(world: &BenchWorld) {
    assert_eq!(world.pois.borrow().len(), 50);
}

#[then("each POI has a valid ID")]
fn then_pois_have_valid_ids(world: &BenchWorld) {
    let pois = world.pois.borrow();
    for (i, poi) in pois.iter().enumerate() {
        #[expect(clippy::as_conversions, reason = "Safe conversion for test indices")]
        let expected_id = (i + 1) as u64;
        assert_eq!(poi.id, expected_id, "POI {i} has wrong ID");
    }
}

#[then("each POI has a theme tag")]
fn then_pois_have_theme_tags(world: &BenchWorld) {
    let pois = world.pois.borrow();
    let valid_themes = ["history", "art", "nature", "culture"];
    for (i, poi) in pois.iter().enumerate() {
        let has_valid_theme = poi
            .tags
            .iter()
            .any(|(key, _)| valid_themes.contains(&key.as_str()));
        assert!(has_valid_theme, "POI {i} should have a valid theme tag");
    }
}

#[then("both sets of POIs are identical")]
#[expect(
    clippy::float_cmp,
    reason = "Exact equality expected for determinism test"
)]
fn then_both_poi_sets_identical(world: &BenchWorld) {
    let pois1 = world.pois.borrow();
    let pois2 = world.pois_second.borrow();

    assert_eq!(pois1.len(), pois2.len(), "POI counts should match");

    for (i, (p1, p2)) in pois1.iter().zip(pois2.iter()).enumerate() {
        assert_eq!(p1.id, p2.id, "POI {i} IDs should match");
        assert_eq!(
            p1.location.x, p2.location.x,
            "POI {i} x coordinates should match"
        );
        assert_eq!(
            p1.location.y, p2.location.y,
            "POI {i} y coordinates should match"
        );
    }
}

// =============================================================================
// Travel time matrix scenarios
// =============================================================================

#[given("a set of 10 POIs at known locations")]
fn given_10_pois(world: &BenchWorld) {
    world.count.replace(10);
    world.seed.replace(42);
    let pois = generate_clustered_pois(10, 42);
    world.pois.replace(pois);
}

#[given("a set of 5 POIs at known locations")]
fn given_5_pois(world: &BenchWorld) {
    world.count.replace(5);
    world.seed.replace(42);
    let pois = generate_clustered_pois(5, 42);
    world.pois.replace(pois);
}

#[when("a travel time matrix is generated")]
fn when_matrix_generated(world: &BenchWorld) {
    let pois = world.pois.borrow();
    let seed = *world.seed.borrow();
    let matrix = generate_travel_time_matrix(&pois, seed);
    world.matrix.replace(matrix);
}

#[when("a travel time matrix is generated twice with the same seed")]
fn when_matrix_generated_twice(world: &BenchWorld) {
    let pois = world.pois.borrow();
    let seed = *world.seed.borrow();
    let matrix1 = generate_travel_time_matrix(&pois, seed);
    let matrix2 = generate_travel_time_matrix(&pois, seed);
    world.matrix.replace(matrix1);
    world.matrix_second.replace(matrix2);
}

#[then("the matrix is square")]
fn then_matrix_is_square(world: &BenchWorld) {
    let matrix = world.matrix.borrow();
    let n = world.pois.borrow().len();
    assert_eq!(matrix.len(), n, "Matrix should have {n} rows");
    for (i, row) in matrix.iter().enumerate() {
        assert_eq!(row.len(), n, "Row {i} should have {n} columns");
    }
}

#[then("diagonal entries are zero")]
#[expect(
    clippy::indexing_slicing,
    reason = "Index bounded by matrix.len() in loop"
)]
fn then_diagonal_is_zero(world: &BenchWorld) {
    let matrix = world.matrix.borrow();
    for i in 0..matrix.len() {
        assert_eq!(
            matrix[i][i],
            Duration::ZERO,
            "Diagonal entry [{i}][{i}] should be zero"
        );
    }
}

#[then("off-diagonal entries are positive")]
#[expect(
    clippy::indexing_slicing,
    reason = "Indices bounded by matrix.len() in nested loops"
)]
fn then_off_diagonal_positive(world: &BenchWorld) {
    let matrix = world.matrix.borrow();
    for i in 0..matrix.len() {
        for j in 0..matrix.len() {
            if i != j {
                assert!(
                    matrix[i][j] > Duration::ZERO,
                    "Off-diagonal entry [{i}][{j}] should be positive"
                );
            }
        }
    }
}

#[then("both matrices are identical")]
#[expect(
    clippy::indexing_slicing,
    reason = "Indices bounded by matrix.len() in nested loops"
)]
fn then_both_matrices_identical(world: &BenchWorld) {
    let matrix1 = world.matrix.borrow();
    let matrix2 = world.matrix_second.borrow();

    assert_eq!(
        matrix1.len(),
        matrix2.len(),
        "Matrix dimensions should match"
    );

    for i in 0..matrix1.len() {
        for j in 0..matrix1.len() {
            assert_eq!(
                matrix1[i][j], matrix2[i][j],
                "Matrix entry [{i}][{j}] should match"
            );
        }
    }
}

// =============================================================================
// Scenario definitions
// =============================================================================

#[scenario(path = "tests/features/bench_helpers.feature", index = 0)]
fn generating_pois_produces_clustered_distribution(world: BenchWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/bench_helpers.feature", index = 1)]
fn generating_travel_time_matrix_produces_valid_routing_data(world: BenchWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/bench_helpers.feature", index = 2)]
fn same_seed_produces_identical_pois(world: BenchWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/bench_helpers.feature", index = 3)]
fn same_seed_produces_identical_travel_time_matrix(world: BenchWorld) {
    let _ = world;
}
