//! Entry point for the command-line interface.
#![forbid(unsafe_code)]

fn main() {
    if let Err(err) = wildside_cli::run() {
        eprintln!("wildside: {err}");
        std::process::exit(1);
    }
}
