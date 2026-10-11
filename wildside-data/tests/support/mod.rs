use base64::{Engine as _, engine::general_purpose};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::{Builder, TempPath};

/// Epsilon for floating-point coordinate comparisons in tests
const COORDINATE_EPSILON: f64 = 1.0e-7;

/// Directory containing the encoded fixture blobs.
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Decode a Base64-encoded fixture into a temporary `.osm.pbf` file.
pub fn decode_fixture(dir: &Path, stem: &str) -> TempPath {
    let encoded_path = dir.join(format!("{stem}.osm.pbf.b64"));
    let encoded = fs::read_to_string(&encoded_path).unwrap_or_else(|err| {
        panic!("failed to read base64 fixture {encoded_path:?}: {err}");
    });
    let cleaned: String = encoded
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect();
    let decoded = general_purpose::STANDARD
        .decode(cleaned.as_bytes())
        .unwrap_or_else(|err| {
            panic!("failed to decode base64 fixture {encoded_path:?}: {err}");
        });
    let mut tempfile = Builder::new()
        .prefix(stem)
        .suffix(".osm.pbf")
        .tempfile()
        .unwrap_or_else(|err| {
            panic!("failed to create temporary fixture for {stem}: {err}");
        });
    tempfile.write_all(&decoded).unwrap_or_else(|err| {
        panic!("failed to write decoded fixture for {stem}: {err}");
    });
    tempfile.flush().unwrap_or_else(|err| {
        panic!("failed to flush decoded fixture for {stem}: {err}");
    });
    tempfile.into_temp_path()
}

/// Compare floating-point coordinates within a small epsilon.
#[expect(
    clippy::float_arithmetic,
    reason = "test delta computation requires float maths"
)]
pub fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta <= COORDINATE_EPSILON,
        "expected {expected}, got {actual} (|Δ| = {delta})"
    );
}
