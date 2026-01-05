//! Unit tests covering feature-flag behaviour.

#![cfg(not(feature = "store-sqlite"))]

use super::*;
use camino::Utf8PathBuf;
use rstest::rstest;
use tempfile::TempDir;

#[rstest]
fn ingest_requires_store_sqlite() {
    let tmp = TempDir::new().expect("temp dir");
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).expect("utf-8 workspace");
    let osm_path = root.join("fixture.pbf");
    let wikidata_path = root.join("wikidata.json");

    let args = IngestArgs {
        osm_pbf: Some(osm_path),
        wikidata_dump: Some(wikidata_path),
        output_dir: Some(root.join("artefacts")),
    };

    let err = run_ingest(args).expect_err("missing feature should error");
    match err {
        CliError::MissingFeature { feature, action } => {
            assert_eq!(feature, "store-sqlite");
            assert_eq!(action, "ingest");
        }
        other => panic!("expected MissingFeature, found {other:?}"),
    }
}
