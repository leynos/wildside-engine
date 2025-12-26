//! Benchmark support utilities for the VRP solver.
//!
//! Provides deterministic POI generation with clustered distributions and
//! distance-based travel time matrices for reproducible benchmarks.

use std::time::Duration;

use geo::Coord;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use wildside_core::{PointOfInterest, Tags, Theme};

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
/// 0.1 degrees ~ 10km at the equator.
const AREA_SIZE: f64 = 0.1;

/// Walking speed in degrees per second (5 km/h ~ 0.0014 deg/s at equator).
const WALKING_SPEED_DEG_PER_SEC: f64 = 0.000_014;

/// Generate a clustered POI distribution for benchmarks.
///
/// Creates `count` POIs distributed across multiple clusters, each with a
/// Gaussian-like distribution around the cluster centre. Uses a deterministic
/// seeded RNG for reproducibility.
///
/// # Examples
///
/// ```
/// use wildside_solver_vrp::benches::bench_support::generate_clustered_pois;
///
/// let pois = generate_clustered_pois(50, 42);
/// assert_eq!(pois.len(), 50);
/// ```
#[must_use]
pub fn generate_clustered_pois(count: usize, seed: u64) -> Vec<PointOfInterest> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Generate cluster centres deterministically.
    let cluster_centres: Vec<Coord<f64>> = (0..CLUSTER_COUNT)
        .map(|_| Coord {
            x: rng.gen_range(0.0..AREA_SIZE),
            y: rng.gen_range(0.0..AREA_SIZE),
        })
        .collect();

    (0..count)
        .map(|i| {
            // Assign to a cluster using round-robin.
            #[expect(
                clippy::integer_division_remainder_used,
                reason = "Modulo for cyclic assignment is intentional"
            )]
            let cluster_idx = i % CLUSTER_COUNT;
            let centre = cluster_centres
                .get(cluster_idx)
                .copied()
                .unwrap_or(Coord { x: 0.0, y: 0.0 });

            // Generate position with Gaussian-like spread using Box-Muller.
            let (dx, dy) = box_muller(&mut rng, CLUSTER_SPREAD);

            #[expect(clippy::float_arithmetic, reason = "Required for coordinate offset")]
            let location = Coord {
                x: centre.x + dx,
                y: centre.y + dy,
            };

            // Assign theme cyclically.
            #[expect(
                clippy::integer_division_remainder_used,
                reason = "Modulo for cyclic theme assignment is intentional"
            )]
            let theme_idx = i % THEMES.len();
            let theme = THEMES.get(theme_idx).cloned().unwrap_or(Theme::History);
            let theme_str = theme.to_string().to_lowercase();

            #[expect(clippy::as_conversions, reason = "Safe conversion for small indices")]
            let id = (i + 1) as u64;

            PointOfInterest::new(id, location, Tags::from([(theme_str, String::new())]))
        })
        .collect()
}

/// Generate a distance-based travel time matrix for benchmarks.
///
/// Computes travel times based on Euclidean distance between POI coordinates,
/// scaled by walking speed. The first POI in the slice is treated as the depot
/// (start location). Returns a matrix suitable for `FixedMatrixTravelTimeProvider`.
///
/// # Examples
///
/// ```
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
    let n = pois.len();

    let mut matrix = vec![vec![Duration::ZERO; n]; n];

    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }

            let poi_i = pois.get(i);
            let poi_j = pois.get(j);

            let (loc_i, loc_j) = match (poi_i, poi_j) {
                (Some(a), Some(b)) => (a.location, b.location),
                _ => continue,
            };

            // Euclidean distance in degrees.
            #[expect(
                clippy::float_arithmetic,
                reason = "Required for coordinate difference"
            )]
            let dx = loc_j.x - loc_i.x;

            #[expect(
                clippy::float_arithmetic,
                reason = "Required for coordinate difference"
            )]
            let dy = loc_j.y - loc_i.y;

            #[expect(clippy::float_arithmetic, reason = "Required for distance calculation")]
            let distance = (dx * dx + dy * dy).sqrt();

            // Convert to travel time with some noise (+-20%).
            #[expect(clippy::float_arithmetic, reason = "Required for time calculation")]
            let base_time_secs = distance / WALKING_SPEED_DEG_PER_SEC;

            let noise_factor: f64 = rng.gen_range(0.8..1.2);

            #[expect(clippy::float_arithmetic, reason = "Required for noise application")]
            let time_secs = base_time_secs * noise_factor;

            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "Travel times are bounded and positive"
            )]
            let duration = Duration::from_secs(time_secs.max(1.0) as u64);

            set_matrix_cell(&mut matrix, i, j, duration);
        }
    }

    matrix
}

/// Set a cell in the travel time matrix, avoiding excessive nesting.
fn set_matrix_cell(matrix: &mut [Vec<Duration>], i: usize, j: usize, duration: Duration) {
    if let Some(row) = matrix.get_mut(i)
        && let Some(cell) = row.get_mut(j)
    {
        *cell = duration;
    }
}

/// Box-Muller transform to generate Gaussian-distributed values.
///
/// Returns a pair of independent standard normal variates scaled by `std_dev`.
fn box_muller<R: Rng>(rng: &mut R, std_dev: f64) -> (f64, f64) {
    let u1: f64 = rng.gen_range(0.0001..1.0);
    let u2: f64 = rng.gen_range(0.0..1.0);

    #[expect(clippy::float_arithmetic, reason = "Required for Box-Muller transform")]
    let r = (-2.0 * u1.ln()).sqrt();

    #[expect(clippy::float_arithmetic, reason = "Required for Box-Muller transform")]
    let theta = 2.0 * std::f64::consts::PI * u2;

    #[expect(clippy::float_arithmetic, reason = "Required for Box-Muller transform")]
    let z0 = r * theta.cos() * std_dev;

    #[expect(clippy::float_arithmetic, reason = "Required for Box-Muller transform")]
    let z1 = r * theta.sin() * std_dev;

    (z0, z1)
}
