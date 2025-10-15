#![forbid(unsafe_code)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
};
use thiserror::Error;
use wildside_data::wikidata::dump::{
    DEFAULT_USER_AGENT, DownloadLog, HttpDumpSource, WikidataDumpError, download_descriptor,
    resolve_latest_descriptor,
};

fn main() {
    match run() {
        Ok(()) => {}
        Err(CliError::Usage) => {
            print_usage();
        }
        Err(error) => {
            eprintln!("wikidata-etl: {error}");
            process::exit(1);
        }
    }
}

fn run() -> Result<(), CliError> {
    let arguments = Arguments::parse(env::args().skip(1))?;
    let Arguments {
        output_dir,
        file_name,
        metadata_db,
        endpoint,
        user_agent,
        overwrite,
    } = arguments;

    let source = HttpDumpSource::new(endpoint).with_user_agent(user_agent);
    let descriptor = resolve_latest_descriptor(&source)?;
    let target_file = match file_name {
        Some(name) => name,
        None => descriptor.file_name.clone(),
    };
    let output_path = output_dir.join(&target_file);
    if output_path.exists() && !overwrite {
        return Err(CliError::OutputExists { path: output_path });
    }

    let log = initialise_log(metadata_db.as_deref())?;
    let report = download_descriptor(&source, descriptor, &output_path, log.as_ref())?;
    println!(
        "Downloaded {} ({} bytes) to {}",
        report.descriptor.file_name,
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

#[derive(Debug)]
struct Arguments {
    output_dir: PathBuf,
    file_name: Option<String>,
    metadata_db: Option<PathBuf>,
    endpoint: String,
    user_agent: String,
    overwrite: bool,
}

impl Arguments {
    fn parse<I>(mut args: I) -> Result<Self, CliError>
    where
        I: Iterator<Item = String>,
    {
        let mut output_dir: Option<PathBuf> = None;
        let mut file_name: Option<String> = None;
        let mut metadata_db: Option<PathBuf> = None;
        let mut endpoint: Option<String> = None;
        let mut user_agent: Option<String> = None;
        let mut overwrite = false;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => return Err(CliError::Usage),
                "--output-dir" | "-o" => {
                    let value = next_value(&mut args, "--output-dir")?;
                    output_dir = Some(PathBuf::from(value));
                }
                "--file-name" | "-f" => {
                    let value = next_value(&mut args, "--file-name")?;
                    file_name = Some(value);
                }
                "--metadata" | "-m" => {
                    let value = next_value(&mut args, "--metadata")?;
                    metadata_db = Some(PathBuf::from(value));
                }
                "--endpoint" => {
                    let value = next_value(&mut args, "--endpoint")?;
                    endpoint = Some(value);
                }
                "--user-agent" => {
                    let value = next_value(&mut args, "--user-agent")?;
                    user_agent = Some(value);
                }
                "--overwrite" => overwrite = true,
                other => return Err(CliError::UnknownArgument(other.to_owned())),
            }
        }

        let output_dir = output_dir.ok_or(CliError::MissingOutputDir)?;
        let endpoint = match endpoint {
            Some(value) => value,
            None => default_endpoint(),
        };
        let user_agent = match user_agent {
            Some(value) => value,
            None => default_user_agent(),
        };

        Ok(Self {
            output_dir,
            file_name,
            metadata_db,
            endpoint,
            user_agent,
            overwrite,
        })
    }
}

fn next_value<I>(args: &mut I, option: &'static str) -> Result<String, CliError>
where
    I: Iterator<Item = String>,
{
    args.next()
        .ok_or(CliError::MissingValue { argument: option })
}

fn default_endpoint() -> String {
    "https://dumps.wikimedia.org".to_owned()
}

fn default_user_agent() -> String {
    DEFAULT_USER_AGENT.to_owned()
}

fn print_usage() {
    println!("Wikidata ETL downloader");
    println!();
    println!("Usage: wikidata-etl [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -o, --output-dir <path>    Directory to store the downloaded dump");
    println!(
        "  -f, --file-name <name>     Override the dump file name (defaults to manifest value)"
    );
    println!("  -m, --metadata <path>      Optional path to a SQLite download log");
    println!("      --endpoint <url>       Override the dumps endpoint (for testing)");
    println!("      --user-agent <agent>   Custom HTTP user agent string");
    println!("      --overwrite            Overwrite the output file if it exists");
    println!("  -h, --help                Show this message");
}

#[derive(Debug, Error)]
enum CliError {
    #[error("usage requested")]
    Usage,
    #[error("missing value for {argument}")]
    MissingValue { argument: &'static str },
    #[error("missing required argument --output-dir")]
    MissingOutputDir,
    #[error("unknown argument {0}")]
    UnknownArgument(String),
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
