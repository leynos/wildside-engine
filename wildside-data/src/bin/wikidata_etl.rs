//! CLI entrypoint for the Wikidata ETL downloader.
#![forbid(unsafe_code)]

use clap::Parser;
use std::{
    fs,
    path::{Path, PathBuf},
    process,
};
use thiserror::Error;
use wildside_data::wikidata::dump::{
    DEFAULT_USER_AGENT, DownloadLog, DownloadOptions, DumpSource, HttpDumpSource,
    WikidataDumpError, download_descriptor, resolve_latest_descriptor,
};

#[tokio::main]
async fn main() {
    let args = Arguments::parse();
    if let Err(error) = run(args).await {
        eprintln!("wikidata-etl: {error}");
        process::exit(1);
    }
}

async fn run(arguments: Arguments) -> Result<(), CliError> {
    let endpoint = arguments.endpoint.clone();
    let user_agent = arguments.user_agent.clone();
    let source = HttpDumpSource::new(endpoint).with_user_agent(user_agent);
    execute(arguments, source).await
}

async fn execute<S: DumpSource>(arguments: Arguments, source: S) -> Result<(), CliError> {
    let Arguments {
        output_dir,
        file_name,
        metadata_db,
        overwrite,
        ..
    } = arguments;

    let descriptor = resolve_latest_descriptor(&source).await?;
    let target_file = file_name.unwrap_or_else(|| descriptor.file_name.clone().into_inner());
    let output_path = output_dir.join(&target_file);
    if output_path.exists() && !overwrite {
        return Err(CliError::OutputExists { path: output_path });
    }

    let log = initialise_log(metadata_db.as_deref())?;
    let options = log
        .as_ref()
        .map_or_else(
            || DownloadOptions::new(output_path.as_path()),
            |entry| DownloadOptions::new(output_path.as_path()).with_log(entry),
        )
        .with_overwrite(overwrite);
    let report = download_descriptor(&source, descriptor, options).await?;
    println!(
        "Downloaded {} ({} bytes) to {}",
        report.descriptor.file_name.as_ref(),
        report.bytes_written,
        report.output_path.display()
    );
    Ok(())
}

fn initialise_log(path: Option<&Path>) -> Result<Option<DownloadLog>, CliError> {
    let Some(path) = path else {
        return Ok(None);
    };
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|source| CliError::CreateLogDirectory {
            source,
            path: parent.to_path_buf(),
        })?;
    }
    let log = DownloadLog::initialise(path)?;
    Ok(Some(log))
}

#[derive(Debug, Parser)]
#[command(name = "wikidata-etl", about = "Wikidata ETL downloader")]
struct Arguments {
    /// Directory to store the downloaded dump
    #[arg(short, long, value_name = "path")]
    output_dir: PathBuf,
    /// Override the dump file name (defaults to manifest value)
    #[arg(short = 'f', long, value_name = "name")]
    file_name: Option<String>,
    /// Optional path to a SQLite download log
    #[arg(short = 'm', long = "metadata", value_name = "path")]
    metadata_db: Option<PathBuf>,
    /// Override the dumps endpoint (for testing)
    #[arg(
        long,
        value_name = "url",
        default_value = "https://dumps.wikimedia.org"
    )]
    endpoint: String,
    /// Custom HTTP user agent string
    #[arg(long, value_name = "agent", default_value = DEFAULT_USER_AGENT)]
    user_agent: String,
    /// Overwrite the output file if it exists
    #[arg(long)]
    overwrite: bool,
}

#[derive(Debug, Error)]
enum CliError {
    #[error("output file {path:?} already exists (pass --overwrite)")]
    OutputExists { path: PathBuf },
    #[error("failed to create log directory {path:?}: {source}")]
    CreateLogDirectory {
        source: std::io::Error,
        path: PathBuf,
    },
    #[error(transparent)]
    Pipeline(#[from] WikidataDumpError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};
    use std::fs;
    use tempfile::TempDir;
    use wildside_data::wikidata::dump::BaseUrl;
    use wildside_data::wikidata::dump::test_support::{StubSource, block_on_for_tests};

    #[fixture]
    fn base_url() -> BaseUrl {
        BaseUrl::from("https://example.org")
    }

    #[fixture]
    fn manifest() -> Vec<u8> {
        let json = r#"{
            "jobs": {
                "json": {
                    "status": "done",
                    "files": {
                        "wikidatawiki-20240909-all.json.bz2": {
                            "url": "/wikidatawiki/entities/20240909/wikidatawiki-20240909-all.json.bz2",
                            "size": 5
                        }
                    }
                }
            }
        }"#;
        json.as_bytes().to_vec()
    }

    #[fixture]
    fn archive() -> Vec<u8> {
        b"hello".to_vec()
    }

    #[fixture]
    fn tmp() -> TempDir {
        TempDir::new().expect("failed to create temporary directory")
    }

    #[rstest]
    fn parses_minimum_arguments(tmp: TempDir) {
        let output = tmp.path().join("dump");
        let args =
            Arguments::try_parse_from(["wikidata-etl", "--output-dir", output.to_str().unwrap()])
                .expect("arguments should parse");
        assert_eq!(args.output_dir, output);
        assert_eq!(args.file_name, None);
        assert_eq!(args.metadata_db, None);
        assert_eq!(args.endpoint, "https://dumps.wikimedia.org");
        assert_eq!(args.user_agent, DEFAULT_USER_AGENT);
        assert!(!args.overwrite);
    }

    #[rstest]
    fn parses_overrides(tmp: TempDir) {
        let output = tmp.path().join("dump");
        let metadata = tmp.path().join("log");
        let args = Arguments::try_parse_from([
            "wikidata-etl",
            "--output-dir",
            output.to_str().unwrap(),
            "--file-name",
            "custom.bz2",
            "--metadata",
            metadata.to_str().unwrap(),
            "--endpoint",
            "https://mirror.local",
            "--user-agent",
            "agent/1.0",
            "--overwrite",
        ])
        .expect("arguments should parse");
        assert_eq!(args.file_name.as_deref(), Some("custom.bz2"));
        assert_eq!(args.metadata_db.as_deref(), Some(metadata.as_path()));
        assert_eq!(args.endpoint, "https://mirror.local");
        assert_eq!(args.user_agent, "agent/1.0");
        assert!(args.overwrite);
    }

    #[rstest]
    fn rejects_missing_output_dir() {
        let outcome = Arguments::try_parse_from(["wikidata-etl"]);
        assert!(outcome.is_err(), "parser should require --output-dir");
    }

    #[rstest]
    fn execute_errors_when_output_exists(
        tmp: TempDir,
        base_url: BaseUrl,
        manifest: Vec<u8>,
        archive: Vec<u8>,
    ) {
        let output_dir = tmp.path().join("out");
        fs::create_dir_all(&output_dir).expect("failed to create output dir");
        let output_file = output_dir.join("wikidatawiki-20240909-all.json.bz2");
        fs::write(&output_file, b"existing").expect("failed to create existing file");
        let args = Arguments {
            output_dir: output_dir.clone(),
            file_name: None,
            metadata_db: None,
            endpoint: base_url.clone().into_inner(),
            user_agent: DEFAULT_USER_AGENT.to_owned(),
            overwrite: false,
        };
        let source = StubSource::new(base_url, manifest, archive);
        let outcome = block_on_for_tests(execute(args, source));
        assert!(matches!(outcome, Err(CliError::OutputExists { path }) if path == output_file));
    }

    #[rstest]
    fn initialise_log_creates_parent(tmp: TempDir) {
        let nested = tmp.path().join("nested").join("downloads.sqlite");
        let outcome = initialise_log(Some(nested.as_path()))
            .expect("initialisation should succeed")
            .expect("log should be created");
        assert!(nested.exists());
        assert_eq!(outcome.path(), nested.as_path());
    }

    #[rstest]
    fn initialise_log_skips_when_absent() {
        let log = initialise_log(None).expect("initialise_log should succeed");
        assert!(log.is_none());
    }
}
