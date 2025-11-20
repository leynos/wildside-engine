//! Behavioural coverage for the Wikidata dump downloader.

use crate::wikidata::dump::test_support::{StubSource, block_on_for_tests};
use crate::wikidata::dump::{DownloadLog, DownloadReport, WikidataDumpError, download_latest_dump};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::{cell::RefCell, fs, path::PathBuf};
use tempfile::TempDir;

const SAMPLE_ARCHIVE: &[u8] = b"sample";

type DownloadResultCell = RefCell<Option<Result<DownloadReport, WikidataDumpError>>>;

#[fixture]
fn working_dir() -> TempDir {
    match TempDir::new() {
        Ok(dir) => dir,
        Err(err) => panic!("failed to create temporary directory: {err}"),
    }
}

#[derive(Debug, Default)]
struct DumpScenarioContext {
    stub_source: RefCell<Option<StubSource>>,
    download_result: DownloadResultCell,
    output_path: RefCell<Option<PathBuf>>,
    log_handle: RefCell<Option<DownloadLog>>,
}

impl DumpScenarioContext {
    fn stub_source(&self) -> &RefCell<Option<StubSource>> {
        &self.stub_source
    }

    fn download_result(&self) -> &DownloadResultCell {
        &self.download_result
    }

    fn output_path(&self) -> &RefCell<Option<PathBuf>> {
        &self.output_path
    }

    fn log_handle(&self) -> &RefCell<Option<DownloadLog>> {
        &self.log_handle
    }
}

#[fixture]
fn dump_context() -> DumpScenarioContext {
    DumpScenarioContext::default()
}

fn build_manifest_with_dump() -> Vec<u8> {
    format!(
        r#"{{
            "jobs": {{
                "json": {{
                    "status": "done",
                    "files": {{
                        "wikidatawiki-20240909-all.json.bz2": {{
                            "url": "/wikidatawiki/entities/20240909/wikidatawiki-20240909-all.json.bz2",
                            "size": {size},
                            "sha1": "abc123"
                        }}
                    }}
                }}
            }}
        }}"#,
        size = SAMPLE_ARCHIVE.len()
    )
    .into_bytes()
}

fn build_manifest_without_dump() -> Vec<u8> {
    r#"{
        "jobs": {
            "json": {
                "status": "failed",
                "files": {}
            }
        }
    }"#
    .as_bytes()
    .to_vec()
}

#[given("a dump status manifest containing a JSON dump")]
fn manifest_with_dump(#[from(dump_context)] ctx: &DumpScenarioContext) {
    *ctx.stub_source().borrow_mut() = Some(StubSource::with_manifest(
        build_manifest_with_dump(),
        SAMPLE_ARCHIVE.to_vec(),
    ));
}

#[given("a dump status manifest missing the JSON dump")]
fn manifest_without_dump(#[from(dump_context)] ctx: &DumpScenarioContext) {
    *ctx.stub_source().borrow_mut() = Some(StubSource::with_manifest(
        build_manifest_without_dump(),
        SAMPLE_ARCHIVE.to_vec(),
    ));
}

#[given("a writable output directory")]
fn writable_output(
    #[from(working_dir)] dir: &TempDir,
    #[from(dump_context)] ctx: &DumpScenarioContext,
) {
    *ctx.output_path().borrow_mut() = Some(dir.path().join("wikidata-latest.json.bz2"));
}

#[given("a download log target")]
fn download_log_target(
    #[from(working_dir)] dir: &TempDir,
    #[from(dump_context)] ctx: &DumpScenarioContext,
) {
    let path = dir.path().join("downloads.sqlite");
    let log = match DownloadLog::initialise(&path) {
        Ok(log) => log,
        Err(err) => panic!("log initialisation failed: {err}"),
    };
    *ctx.log_handle().borrow_mut() = Some(log);
}

#[when("I download the latest dump")]
fn download_latest(
    #[from(dump_context)] ctx: &DumpScenarioContext,
    #[from(working_dir)] _dir: &TempDir,
) {
    let source_borrow = ctx.stub_source().borrow();
    let stub = source_borrow
        .as_ref()
        .unwrap_or_else(|| panic!("stub source must be initialised"));
    let output_path = {
        let borrowed = ctx.output_path().borrow();
        borrowed
            .as_ref()
            .cloned()
            .unwrap_or_else(|| panic!("output path must be prepared"))
    };
    let log_borrow = ctx.log_handle().borrow();
    let log_ref = log_borrow.as_ref();
    let outcome = block_on_for_tests(download_latest_dump(stub, &output_path, log_ref, false));
    *ctx.download_result().borrow_mut() = Some(outcome);
}

#[then("the archive is written to disk")]
fn archive_written(#[from(dump_context)] ctx: &DumpScenarioContext) {
    let source_borrow = ctx.stub_source().borrow();
    let expected = source_borrow
        .as_ref()
        .map(StubSource::archive)
        .unwrap_or_else(|| panic!("stub source must be initialised"));
    let result_borrow = ctx.download_result().borrow();
    let outcome = result_borrow
        .as_ref()
        .unwrap_or_else(|| panic!("download result must be captured"));
    let report = match outcome {
        Ok(report) => report,
        Err(err) => panic!("download should succeed: {err}"),
    };
    let output_path = ctx
        .output_path()
        .borrow()
        .as_ref()
        .cloned()
        .unwrap_or_else(|| panic!("output path must be prepared"));
    let contents = match fs::read(&output_path) {
        Ok(bytes) => bytes,
        Err(err) => panic!("failed to read downloaded archive: {err}"),
    };
    assert_eq!(contents, expected);
    assert_eq!(report.output_path, output_path);
}

#[then("the download log records an entry")]
fn log_records_entry(#[from(dump_context)] ctx: &DumpScenarioContext) {
    let log_borrow = ctx.log_handle().borrow();
    let log = log_borrow
        .as_ref()
        .unwrap_or_else(|| panic!("download log should be initialised"));
    let count: i64 = match log
        .connection()
        .query_row("SELECT COUNT(*) FROM downloads", [], |row| row.get(0))
    {
        Ok(value) => value,
        Err(err) => panic!("failed to query download log: {err}"),
    };
    assert_eq!(count, 1);
}

#[then("an error about the missing dump is returned")]
fn missing_dump_error(#[from(dump_context)] ctx: &DumpScenarioContext) {
    let result_borrow = ctx.download_result().borrow();
    let outcome = result_borrow
        .as_ref()
        .unwrap_or_else(|| panic!("download result must be captured"));
    match outcome {
        Ok(_) => panic!("expected an error for the missing dump"),
        Err(WikidataDumpError::MissingDump) => {}
        Err(err) => panic!("unexpected error variant: {err}"),
    }
}

#[test]
fn scenario_indices_follow_feature_order() {
    let feature_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/features/download_wikidata_dump.feature");
    let contents = match fs::read_to_string(&feature_path) {
        Ok(data) => data,
        Err(err) => panic!("failed to read feature file {feature_path:?}: {err}"),
    };
    let titles: Vec<String> = contents
        .lines()
        .filter_map(|line| line.trim().strip_prefix("Scenario: "))
        .map(|title| title.to_owned())
        .collect();
    let expected = [
        "downloading the latest dump descriptor",
        "reporting a missing dump",
    ];
    assert_eq!(
        titles.len(),
        expected.len(),
        "scenario count changed in feature file: {titles:?}"
    );
    for (index, expected_title) in expected.iter().enumerate() {
        let actual = titles.get(index).map(String::as_str);
        assert_eq!(
            actual,
            Some(*expected_title),
            "scenario at index {index} does not match feature order"
        );
    }
}

macro_rules! register_scenario {
    ($name:ident, $index:literal) => {
        #[scenario(path = "tests/features/download_wikidata_dump.feature", index = $index)]
        fn $name(#[from(dump_context)] context: DumpScenarioContext, working_dir: TempDir) {
            let _ = (context, working_dir);
        }
    };
}

register_scenario!(downloading_the_latest_dump_descriptor, 0);
register_scenario!(reporting_a_missing_dump, 1);
