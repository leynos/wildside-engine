//! Benchmark support utilities for the VRP solver.
//!
//! Provides deterministic POI generation with clustered distributions and
//! distance-based travel time matrices for reproducible benchmarks.

use std::time::Duration;

use geo::Coord;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, Normal, Uniform};
use wildside_core::{InterestProfile, PointOfInterest, SolveRequest, Tags, Theme};

/// Seed for deterministic random number generation in benchmarks.
pub const BENCHMARK_SEED: u64 = 42;

/// Themes to cycle through when assigning POI tags.
const THEMES: [Theme; 4] = [Theme::History, Theme::Art, Theme::Nature, Theme::Culture];

/// Number of cluster centres for POI distribution.
const CLUSTER_COUNT: usize = 5;

/// Standard deviation for POI distribution around cluster centres (in degrees).
/// Approximately 0.005 degrees ~ 500m at the equator.
const CLUSTER_SPREAD: f64 = 0.005;

/// Area size for cluster centre distribution (in degrees).
/// 0.06 degrees ~ 6km at the equator.
const AREA_SIZE: f64 = 0.06;

/// Walking speed in degrees per second (5 km/h ~ 0.000014 deg/s at equator).
const WALKING_SPEED_DEG_PER_SEC: f64 = 0.000_014;

/// Time budget for benchmark solve requests (minutes).
pub const DURATION_MINUTES: u16 = 60;

/// Start position for benchmarks (centre of the POI area).
pub const BENCHMARK_START: Coord<f64> = Coord { x: 0.05, y: 0.05 };

/// Build a standard benchmark solve request.
///
/// Uses a consistent interest profile and deterministic seed for reproducibility.
///
/// # Examples
///
/// ```ignore
/// use wildside_solver_vrp::benches::bench_support::build_benchmark_request;
///
/// let request = build_benchmark_request(42);
/// assert_eq!(request.duration_minutes, 60);
/// ```
#[must_use]
pub fn build_benchmark_request(seed: u64) -> SolveRequest {
    SolveRequest {
        start: BENCHMARK_START,
        end: None,
        duration_minutes: DURATION_MINUTES,
        interests: InterestProfile::new()
            .with_weight(Theme::Art, 0.8)
            .with_weight(Theme::History, 0.5)
            .with_weight(Theme::Nature, 0.3)
            .with_weight(Theme::Culture, 0.2),
        seed,
        max_nodes: None,
    }
}

/// Create a depot POI at the start location for the travel time matrix.
///
/// The depot uses ID 0 and has no tags.
///
/// # Examples
///
/// ```ignore
/// use geo::Coord;
/// use wildside_solver_vrp::benches::bench_support::create_depot;
///
/// let depot = create_depot(Coord { x: 0.05, y: 0.05 });
/// assert_eq!(depot.id, 0);
/// ```
#[must_use]
pub fn create_depot(start: Coord<f64>) -> PointOfInterest {
    PointOfInterest::with_empty_tags(0, start)
}

/// Generate a clustered POI distribution for benchmarks.
///
/// Creates `count` POIs distributed across multiple clusters, each with a
/// Gaussian-like distribution around the cluster centre. Uses a deterministic
/// seeded RNG for reproducibility.
///
/// # Panics
///
/// This function cannot panic under normal usage. The internal `unwrap` on
/// `Normal::new` is safe because the standard deviation is a positive constant.
///
/// # Examples
///
/// ```ignore
/// use wildside_solver_vrp::benches::bench_support::generate_clustered_pois;
///
/// let pois = generate_clustered_pois(50, 42);
/// assert_eq!(pois.len(), 50);
/// ```
#[must_use]
pub fn generate_clustered_pois(count: usize, seed: u64) -> Vec<PointOfInterest> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Centre POI distribution around BENCHMARK_START to ensure all POIs fall
    // within the solver's bounding box. Use separate distributions for x and y
    // in case BENCHMARK_START coordinates ever diverge.
    #[expect(clippy::float_arithmetic, reason = "Centering POI distribution")]
    let half_area = AREA_SIZE / 2.0;
    #[expect(clippy::float_arithmetic, reason = "Centering POI distribution")]
    let (min_x, max_x) = (BENCHMARK_START.x - half_area, BENCHMARK_START.x + half_area);
    #[expect(clippy::float_arithmetic, reason = "Centering POI distribution")]
    let (min_y, max_y) = (BENCHMARK_START.y - half_area, BENCHMARK_START.y + half_area);
    let x_dist = Uniform::new(min_x, max_x);
    let y_dist = Uniform::new(min_y, max_y);

    // Generate cluster centres deterministically.
    let cluster_centres: Vec<Coord<f64>> = (0..CLUSTER_COUNT)
        .map(|_| Coord {
            x: x_dist.sample(&mut rng),
            y: y_dist.sample(&mut rng),
        })
        .collect();

    // Normal distribution for Gaussian spread around cluster centres.
    // unwrap is safe: std_dev > 0
    #[expect(clippy::unwrap_used, reason = "std_dev is a positive constant")]
    let normal = Normal::new(0.0, CLUSTER_SPREAD).unwrap();

    (0..count)
        .map(|i| {
            // Assign to a cluster using round-robin.
            // Indexing is safe: result of modulo is always < CLUSTER_COUNT.
            #[expect(
                clippy::integer_division_remainder_used,
                clippy::indexing_slicing,
                reason = "Modulo for cyclic assignment is intentional and result is bounded"
            )]
            let centre = cluster_centres[i % CLUSTER_COUNT];

            // Generate position with Gaussian spread.
            let dx: f64 = normal.sample(&mut rng);
            let dy: f64 = normal.sample(&mut rng);

            #[expect(clippy::float_arithmetic, reason = "Required for coordinate offset")]
            let location = Coord {
                x: centre.x + dx,
                y: centre.y + dy,
            };

            // Assign theme cyclically.
            // Indexing is safe: result of modulo is always < THEMES.len().
            #[expect(
                clippy::integer_division_remainder_used,
                clippy::indexing_slicing,
                reason = "Modulo for cyclic theme assignment is intentional and result is bounded"
            )]
            let theme = THEMES[i % THEMES.len()].clone();
            let theme_str = theme.to_string().to_lowercase();

            #[expect(clippy::as_conversions, reason = "Safe conversion for small indices")]
            let id = (i + 1) as u64;

            PointOfInterest::new(id, location, Tags::from([(theme_str, String::new())]))
        })
        .collect()
}

/// Compute Euclidean distance between two coordinates.
///
/// Returns the straight-line distance in degrees.
#[expect(
    clippy::float_arithmetic,
    reason = "Euclidean distance calculation requires float arithmetic"
)]
fn euclidean_distance(from: Coord<f64>, to: Coord<f64>) -> f64 {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    (dx * dx + dy * dy).sqrt()
}

/// Compute travel time from distance with noise.
///
/// Converts distance to travel time based on walking speed, then applies
/// a random noise factor (±20%) for realistic variation.
#[expect(
    clippy::float_arithmetic,
    reason = "Travel time calculation requires float arithmetic"
)]
fn travel_time_with_noise(distance: f64, noise_factor: f64) -> Duration {
    let base_time_secs = distance / WALKING_SPEED_DEG_PER_SEC;
    let time_secs = base_time_secs * noise_factor;
    Duration::from_secs_f64(time_secs.max(1.0))
}

/// Generate a distance-based travel time matrix for benchmarks.
///
/// Computes travel times based on Euclidean distance between POI coordinates,
/// scaled by walking speed. The first POI in the slice is treated as the depot
/// (start location). Returns a matrix suitable for `FixedMatrixTravelTimeProvider`.
///
/// # Examples
///
/// ```ignore
/// use wildside_solver_vrp::benches::bench_support::{generate_clustered_pois, generate_travel_time_matrix};
///
/// let pois = generate_clustered_pois(10, 42);
/// let matrix = generate_travel_time_matrix(&pois, 42);
/// assert_eq!(matrix.len(), 10);
/// assert_eq!(matrix[0].len(), 10);
/// ```
#[must_use]
pub fn generate_travel_time_matrix(pois: &[PointOfInterest], seed: u64) -> Vec<Vec<Duration>> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let noise_dist = Uniform::new(0.8, 1.2);
    let n = pois.len();

    let mut matrix = vec![vec![Duration::ZERO; n]; n];

    // Loop indices i and j are bounded by n = pois.len() = matrix.len(), so all
    // slice accesses are within bounds. The #[expect] attributes document this
    // invariant for the linter.
    #[expect(
        clippy::indexing_slicing,
        reason = "Loop indices are bounded by n = pois.len() = matrix.len()"
    )]
    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }

            let loc_i = pois[i].location;
            let loc_j = pois[j].location;

            let distance = euclidean_distance(loc_i, loc_j);
            let noise_factor: f64 = noise_dist.sample(&mut rng);
            let duration = travel_time_with_noise(distance, noise_factor);

            matrix[i][j] = duration;
        }
    }

    matrix
}
