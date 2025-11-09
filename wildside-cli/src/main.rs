//! Entry point for the command-line interface.
#![forbid(unsafe_code)]

use eyre::Result;

fn main() -> Result<()> {
    wildside_cli::run()?;
    Ok(())
}
