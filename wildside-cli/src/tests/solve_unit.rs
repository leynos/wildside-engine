//! Focused unit tests covering solve CLI configuration and request parsing.

use super::helpers::write_utf8;
use super::*;
use camino::Utf8PathBuf;
use geo::Coord;
use rstest::rstest;
use tempfile::TempDir;
use wildside_core::{InterestProfile, SolveRequest};

#[derive(Debug, Copy, Clone)]
enum MissingArtefact {
    Request,
    PoisDb,
    SpatialIndex,
    Popularity,
}

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
#[case::missing_request(ARG_SOLVE_REQUEST, MissingArtefact::Request)]
#[case::missing_db(ARG_SOLVE_POIS_DB, MissingArtefact::PoisDb)]
#[case::missing_index(ARG_SOLVE_SPATIAL_INDEX, MissingArtefact::SpatialIndex)]
#[case::missing_popularity(ARG_SOLVE_POPULARITY, MissingArtefact::Popularity)]
fn validate_sources_reports_missing_artefacts(
    #[case] expected_field: &'static str,
    #[case] missing: MissingArtefact,
) {
    let tmp = TempDir::new().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).expect("utf-8 workspace");

    let request_path = root.join("request.json");
    let db_path = root.join("pois.db");
    let index_path = root.join("pois.rstar");
    let popularity_path = root.join("popularity.bin");

    if !matches!(missing, MissingArtefact::Request) {
        write_utf8(&request_path, b"{}");
    }
    if !matches!(missing, MissingArtefact::PoisDb) {
        write_utf8(&db_path, b"db");
    }
    if !matches!(missing, MissingArtefact::SpatialIndex) {
        write_utf8(&index_path, b"index");
    }
    if !matches!(missing, MissingArtefact::Popularity) {
        write_utf8(&popularity_path, b"popularity");
    }

    let config = SolveConfig {
        request_path,
        pois_db: db_path,
        spatial_index: index_path,
        popularity: popularity_path,
        osrm_base_url: "http://localhost:5000".to_string(),
    };

    let err = config.validate_sources().expect_err("expected failure");
    match err {
        CliError::MissingSourceFile { field, .. } => assert_eq!(field, expected_field),
        other => panic!("expected MissingSourceFile, found {other:?}"),
    }
}

#[rstest]
fn validate_sources_reports_not_file() {
    let tmp = TempDir::new().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).expect("utf-8 workspace");

    let request_path = root.join("request.json");
    std::fs::create_dir(&request_path).expect("request directory");

    let config = SolveConfig {
        request_path: request_path.clone(),
        pois_db: root.join("pois.db"),
        spatial_index: root.join("pois.rstar"),
        popularity: root.join("popularity.bin"),
        osrm_base_url: "http://localhost:5000".to_string(),
    };

    let err = config
        .validate_sources()
        .expect_err("expected directory path to fail validation");
    match err {
        CliError::SourcePathNotFile { field, path } => {
            assert_eq!(field, ARG_SOLVE_REQUEST);
            assert_eq!(path, request_path);
        }
        other => panic!("expected SourcePathNotFile, found {other:?}"),
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

/// Helper to set up a temporary directory and request path for load_solve_request tests.
fn setup_request_test() -> (TempDir, Utf8PathBuf) {
    let tmp = TempDir::new().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).expect("utf-8 workspace");
    let request_path = root.join("request.json");
    (tmp, request_path)
}

#[rstest]
fn load_solve_request_rejects_invalid_json() {
    let (_tmp, request_path) = setup_request_test();
    write_utf8(&request_path, b"{ not valid json");

    let err = load_solve_request(&request_path).expect_err("invalid json should error");
    match err {
        CliError::ParseSolveRequest { path, .. } => assert_eq!(path, request_path),
        other => panic!("unexpected error {other:?}"),
    }
}

#[rstest]
fn load_solve_request_io_error_returns_open_error() {
    let (_tmp, request_path) = setup_request_test();
    // Deliberately don't write the file to trigger IO error

    let err = load_solve_request(&request_path).expect_err("missing request should error");
    match err {
        CliError::OpenSolveRequest { path, .. } => assert_eq!(path, request_path),
        other => panic!("expected OpenSolveRequest, found {other:?}"),
    }
}

#[rstest]
fn merge_layers_maps_configuration_errors() {
    use ortho_config::MergeComposer;
    use serde_json::json;

    let mut composer = MergeComposer::new();
    composer.push_cli(json!({ "request_path": 42 }));

    let err = config_from_layers_for_test(composer.layers())
        .expect_err("invalid config layer should map to CliError::Configuration");
    match err {
        CliError::Configuration(_) => {}
        other => panic!("expected CliError::Configuration, found {other:?}"),
    }
}

#[rstest]
fn merge_layers_honours_precedence_and_defaults_paths() {
    use ortho_config::MergeComposer;
    use serde_json::json;

    let tmp = TempDir::new().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).expect("utf-8 workspace");

    let env_request = root.join("from-env-request.json");
    let cli_dir = root.join("from-cli");
    let mut composer = MergeComposer::new();
    composer.push_file(
        json!({
            "artefacts_dir": root.join("from-file").as_str(),
            "osrm_base_url": "http://from-file:5000",
        }),
        None,
    );
    composer.push_environment(json!({
        "request_path": env_request.as_str(),
        "artefacts_dir": root.join("from-env").as_str(),
    }));
    composer.push_cli(json!({
        "artefacts_dir": cli_dir.as_str(),
    }));

    let config =
        config_from_layers_for_test(composer.layers()).expect("merged config should build");
    assert_eq!(config.request_path, env_request);
    assert_eq!(config.pois_db, cli_dir.join("pois.db"));
    assert_eq!(config.spatial_index, cli_dir.join("pois.rstar"));
    assert_eq!(config.popularity, cli_dir.join("popularity.bin"));
    assert_eq!(config.osrm_base_url, "http://from-file:5000");
}
