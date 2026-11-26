//! Fixture lookup, fixture decoding and float comparison, shared by the ingest
//! integration tests and the crate's unit tests.

use base64::{Engine as _, engine::general_purpose};
use std::{
    fmt::Display,
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

/// Unwrap a fixture step, failing the test with `what` and the error.
///
/// Fixture helpers are not tests, so this is their single, named panic boundary.
fn or_fail<T, E: Display>(result: Result<T, E>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("failed to {what}: {error}"),
    }
}

/// Decode a Base64-encoded fixture into a temporary `.osm.pbf` file.
pub fn decode_fixture(dir: &Path, stem: &str) -> TempPath {
    let encoded_path = dir.join(format!("{stem}.osm.pbf.b64"));
    let encoded = or_fail(
        wildside_fs::read_to_string(&encoded_path),
        &format!("read base64 fixture {encoded_path:?}"),
    );
    let cleaned: String = encoded
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect();
    let decoded = or_fail(
        general_purpose::STANDARD.decode(cleaned.as_bytes()),
        &format!("decode base64 fixture {encoded_path:?}"),
    );
    let mut tempfile = or_fail(
        Builder::new().prefix(stem).suffix(".osm.pbf").tempfile(),
        &format!("create temporary fixture for {stem}"),
    );
    or_fail(
        tempfile.write_all(&decoded),
        &format!("write decoded fixture for {stem}"),
    );
    or_fail(
        tempfile.flush(),
        &format!("flush decoded fixture for {stem}"),
    );
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
