//! Shared plumbing for the environment-policy source-scan contract.
//!
//! The source set, the workspace handle and the failure type live here so that
//! the scanning helpers in [`crate::scan`] and the contracts in the test root
//! each read as one concern.

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use std::process::Command;

/// Error type carried by the helpers and tests.
pub type Failure = Box<dyn std::error::Error>;

/// Sources permitted to allow a protected lint.
///
/// Only the compile-time test's fixture, whose whole purpose is to be rejected
/// by Clippy, and which is its own workspace root so no workspace build
/// compiles it. The list is closed: `only_the_ui_fixture_is_exempt` fails if it
/// gains an entry.
pub const EXEMPT_SOURCES: [&str; 1] = ["tests/fixtures/env_policy_probe/src/lib.rs"];

/// Return `Err` carrying `message` when `condition` does not hold.
pub fn ensure_that(condition: bool, message: String) -> Result<(), Failure> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

/// Open a capability handle on the workspace root.
pub fn workspace_root() -> Result<Dir, Failure> {
    let root = env!("CARGO_MANIFEST_DIR");
    Dir::open_ambient_dir(root, ambient_authority())
        .map_err(|err| -> Failure { format!("open workspace root {root}: {err}").into() })
}

/// Return the tracked Rust sources this contract scans, exemptions removed.
///
/// Tracked files are the right set: an untracked file is not part of the
/// sources, and a scan of the working tree would flag scratch files.
pub fn scannable_sources() -> Result<Vec<Utf8PathBuf>, Failure> {
    let output = Command::new("git")
        .args(["ls-files", "-z", "--", "*.rs"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .map_err(|err| -> Failure { format!("list tracked sources: {err}").into() })?;
    ensure_that(
        output.status.success(),
        format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    let listing = String::from_utf8(output.stdout)
        .map_err(|err| -> Failure { format!("decode the source listing: {err}").into() })?;
    Ok(listing
        .split('\0')
        .filter(|path| !path.is_empty() && !EXEMPT_SOURCES.contains(path))
        .map(Utf8PathBuf::from)
        .collect())
}
