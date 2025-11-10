//! Focused unit tests covering ingest CLI configuration validation.

use super::*;
use rstest::rstest;
use std::{fs, path::PathBuf};
use tempfile::TempDir;

#[rstest]
#[case(None, Some(PathBuf::from("wikidata.json")), ARG_OSM_PBF, ENV_OSM_PBF)]
#[case(
    Some(PathBuf::from("planet.osm.pbf")),
    None,
    ARG_WIKIDATA_DUMP,
    ENV_WIKIDATA_DUMP
)]
fn converting_without_required_fields_errors(
    #[case] osm: Option<PathBuf>,
    #[case] wiki: Option<PathBuf>,
    #[case] field: &'static str,
    #[case] env_var: &'static str,
) {
    let args = IngestArgs {
        osm_pbf: osm,
        wikidata_dump: wiki,
    };
    let err = IngestConfig::try_from(args).expect_err("missing field should error");
    match err {
        CliError::MissingArgument {
            field: missing,
            env,
        } => {
            assert_eq!(missing, field);
            assert_eq!(env, env_var);
        }
        other => panic!("expected MissingArgument, found {other:?}"),
    }
}

#[rstest]
fn validate_sources_reports_missing_files() {
    let tmp = TempDir::new().expect("tempdir");
    let config = IngestConfig {
        osm_pbf: tmp.path().join("missing-osm"),
        wikidata_dump: tmp.path().join("missing-wiki"),
    };
    let err = config.validate_sources().expect_err("expected failure");
    match err {
        CliError::MissingSourceFile { field, .. } => {
            assert_eq!(field, ARG_OSM_PBF);
        }
        other => panic!("unexpected error {other:?}"),
    }
}

#[rstest]
fn validate_sources_rejects_directories() {
    let dir = TempDir::new().expect("tempdir");
    let file_path = dir.path().join("dump.json");
    fs::write(&file_path, b"{}\n").expect("write dump");
    let config = IngestConfig {
        osm_pbf: dir.path().to_path_buf(),
        wikidata_dump: file_path,
    };
    let err = config
        .validate_sources()
        .expect_err("expected directory rejection");
    match err {
        CliError::MissingSourceFile { field, .. } => assert_eq!(field, ARG_OSM_PBF),
        other => panic!("unexpected error {other:?}"),
    }
}
