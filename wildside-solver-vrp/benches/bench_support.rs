//! Benchmark support utilities for the VRP solver.
//!
//! Provides deterministic POI generation with clustered distributions and
//! distance-based travel time matrices for reproducible benchmarks.

use std::time::Duration;

use geo::Coord;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, Normal, Uniform};
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
    let area_dist = Uniform::new(0.0, AREA_SIZE);

    // Generate cluster centres deterministically.
    let cluster_centres: Vec<Coord<f64>> = (0..CLUSTER_COUNT)
        .map(|_| Coord {
            x: area_dist.sample(&mut rng),
            y: area_dist.sample(&mut rng),
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

    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }

            #[expect(
                clippy::indexing_slicing,
                reason = "Indices are bounded by loop over pois.len()"
            )]
            let loc_i = pois[i].location;
            #[expect(
                clippy::indexing_slicing,
                reason = "Indices are bounded by loop over pois.len()"
            )]
            let loc_j = pois[j].location;

            // Euclidean distance in degrees.
            #[expect(clippy::float_arithmetic, reason = "Required for distance calculation")]
            let dx = loc_j.x - loc_i.x;

            #[expect(clippy::float_arithmetic, reason = "Required for distance calculation")]
            let dy = loc_j.y - loc_i.y;

            #[expect(clippy::float_arithmetic, reason = "Required for distance calculation")]
            let distance = (dx * dx + dy * dy).sqrt();

            // Convert to travel time with some noise (+-20%).
            #[expect(clippy::float_arithmetic, reason = "Required for time calculation")]
            let base_time_secs = distance / WALKING_SPEED_DEG_PER_SEC;

            let noise_factor: f64 = noise_dist.sample(&mut rng);

            #[expect(clippy::float_arithmetic, reason = "Required for noise application")]
            let time_secs = base_time_secs * noise_factor;

            let duration = Duration::from_secs_f64(time_secs.max(1.0));

            #[expect(clippy::indexing_slicing, reason = "Indices are bounded by loop")]
            {
                matrix[i][j] = duration;
            }
        }
    }

    matrix
}
