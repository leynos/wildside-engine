//! Entry point for the command-line interface.
#![forbid(unsafe_code)]

fn main() {
    if let Err(err) = run() {
        eprintln!("wildside: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: parse CLI arguments and dispatch commands.
    Ok(())
}
