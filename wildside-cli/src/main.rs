//! Entry point for the command-line interface.
#![forbid(unsafe_code)]

use eyre::Report;

fn main() {
    if let Err(err) = wildside_cli::run() {
        let report = Report::from(err);
        eprintln!("wildside: {report}");
        std::process::exit(1);
    }
}
