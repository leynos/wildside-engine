//! Focused unit tests covering solve CLI configuration and request parsing.

use super::helpers::write_utf8;
use super::*;
use camino::Utf8PathBuf;
use geo::Coord;
use rstest::rstest;
use tempfile::TempDir;
use wildside_core::{InterestProfile, SolveRequest};

#[rstest]
fn converting_solve_without_request_errors() {
    let args = SolveArgs {
        request_path: None,
        ..SolveArgs::default()
    };

    let err = SolveConfig::try_from(args).expect_err("missing request should error");
    match err {
        CliError::MissingArgument { field, env } => {
            assert_eq!(field, ARG_SOLVE_REQUEST);
            assert_eq!(env, ENV_SOLVE_REQUEST);
        }
        other => panic!("expected MissingArgument, found {other:?}"),
    }
}

#[rstest]
fn solve_config_derives_default_artefact_paths() {
    let tmp = TempDir::new().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).expect("utf-8 workspace");

    let request_path = root.join("request.json");
    let db_path = root.join("pois.db");
    let index_path = root.join("pois.rstar");
    let popularity_path = root.join("popularity.bin");

    write_utf8(&request_path, b"{}");
    write_utf8(&db_path, b"db");
    write_utf8(&index_path, b"index");
    write_utf8(&popularity_path, b"popularity");

    let args = SolveArgs {
        request_path: Some(request_path.clone()),
        artefacts_dir: Some(root.clone()),
        pois_db: None,
        spatial_index: None,
        popularity: None,
        osrm_base_url: None,
    };

    let config = SolveConfig::try_from(args).expect("config should build");
    assert_eq!(config.request_path, request_path);
    assert_eq!(config.pois_db, db_path);
    assert_eq!(config.spatial_index, index_path);
    assert_eq!(config.popularity, popularity_path);
    assert_eq!(config.osrm_base_url, "http://localhost:5000");
}

#[rstest]
fn validate_sources_reports_missing_artefacts() {
    let tmp = TempDir::new().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).expect("utf-8 workspace");

    let config = SolveConfig {
        request_path: root.join("missing-request.json"),
        pois_db: root.join("missing-pois.db"),
        spatial_index: root.join("missing-pois.rstar"),
        popularity: root.join("missing-popularity.bin"),
        osrm_base_url: "http://localhost:5000".to_string(),
    };

    let err = config.validate_sources().expect_err("expected failure");
    match err {
        CliError::MissingSourceFile { field, .. } => assert_eq!(field, ARG_SOLVE_REQUEST),
        other => panic!("unexpected error {other:?}"),
    }
}

#[rstest]
fn load_solve_request_decodes_json() {
    let tmp = TempDir::new().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).expect("utf-8 workspace");
    let request_path = root.join("request.json");

    let request = SolveRequest {
        start: Coord { x: 0.1, y: 51.5 },
        end: None,
        duration_minutes: 30,
        interests: InterestProfile::new(),
        seed: 42,
        max_nodes: Some(10),
    };
    let payload = serde_json::to_string_pretty(&request).expect("serialize request");
    write_utf8(&request_path, payload.as_bytes());

    let decoded = load_solve_request(&request_path).expect("request should decode");
    assert_eq!(decoded, request);
}

#[rstest]
fn load_solve_request_rejects_invalid_json() {
    let tmp = TempDir::new().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).expect("utf-8 workspace");
    let request_path = root.join("request.json");

    write_utf8(&request_path, b"{ not valid json");

    let err = load_solve_request(&request_path).expect_err("invalid json should error");
    match err {
        CliError::ParseSolveRequest { path, .. } => assert_eq!(path, request_path),
        other => panic!("unexpected error {other:?}"),
    }
}
