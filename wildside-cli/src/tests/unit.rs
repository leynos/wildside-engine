//! Focused unit tests covering ingest CLI configuration validation.

use super::*;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8};
use rstest::rstest;
use tempfile::TempDir;

fn write_utf8(path: &Utf8PathBuf, contents: impl AsRef<[u8]>) {
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    let file_name = path
        .file_name()
        .expect("write target should include a file name");
    fs_utf8::Dir::open_ambient_dir(parent, ambient_authority())
        .expect("open ambient dir")
        .write(file_name, contents.as_ref())
        .expect("write file");
}

#[rstest]
#[case(
    None,
    Some(Utf8PathBuf::from("wikidata.json")),
    ARG_OSM_PBF,
    ENV_OSM_PBF
)]
#[case(
    Some(Utf8PathBuf::from("planet.osm.pbf")),
    None,
    ARG_WIKIDATA_DUMP,
    ENV_WIKIDATA_DUMP
)]
fn converting_without_required_fields_errors(
    #[case] osm: Option<Utf8PathBuf>,
    #[case] wiki: Option<Utf8PathBuf>,
    #[case] field: &'static str,
    #[case] env_var: &'static str,
) {
    let args = IngestArgs {
        osm_pbf: osm,
        wikidata_dump: wiki,
        ..IngestArgs::default()
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
    let workspace =
        Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).expect("utf-8 workspace path");
    let config = IngestConfig {
        osm_pbf: workspace.join("missing-osm"),
        wikidata_dump: workspace.join("missing-wiki"),
        output_dir: workspace,
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
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf-8 workspace");
    let file_path = root.join("dump.json");
    write_utf8(&file_path, b"{}\n");
    let config = IngestConfig {
        osm_pbf: root.clone(),
        wikidata_dump: file_path,
        output_dir: root.clone(),
    };
    let err = config
        .validate_sources()
        .expect_err("expected directory rejection");
    match err {
        CliError::MissingSourceFile { field, .. } => assert_eq!(field, ARG_OSM_PBF),
        other => panic!("unexpected error {other:?}"),
    }
}

#[rstest]
fn validate_sources_rejects_output_file() {
    let dir = TempDir::new().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf-8 workspace");
    let osm_path = root.join("planet.osm.pbf");
    let wikidata_path = root.join("dump.json");
    let output_file = root.join("pois.db");
    write_utf8(&osm_path, b"osm");
    write_utf8(&wikidata_path, b"wiki");
    write_utf8(&output_file, b"existing artefact");

    let config = IngestConfig {
        osm_pbf: osm_path,
        wikidata_dump: wikidata_path,
        output_dir: output_file,
    };

    let err = config
        .validate_sources()
        .expect_err("expected output directory validation to fail");
    match err {
        CliError::OutputDirectoryNotDirectory { .. } => {}
        other => panic!("unexpected error {other:?}"),
    }
}
