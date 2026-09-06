//! Contract coverage for the environment-access lint policy (issue #126).
//!
//! The policy has several moving parts, and removing any one of them silently
//! reopens ambient process-environment access: the `disallowed-methods` entries
//! in the root `clippy.toml`, the workspace `disallowed_methods` deny that makes
//! Clippy act on them, each package's enforcement of that deny, the hygiene
//! lints that stop an item-scoped `#[allow]` lowering it, and the gate that runs
//! Clippy over every workspace target and feature in CI.
//!
//! The package list comes from `cargo metadata`, not from `[workspace].members`,
//! because Cargo also promotes an in-tree path dependency to a workspace member
//! without an entry in that array. The resolved set is then compared with the
//! eight package names this workspace is known to contain, so both a removed
//! member and an unenforced new one fail here.
//!
//! Mutation proof (run 2026-09-06). Each mutation failed only the test named
//! beside it:
//!
//! - delete the `std::env::set_var` entry from `clippy.toml` —
//!   `clippy_config_disallows_every_environment_method`;
//! - change the workspace `disallowed_methods` deny to `"warn"` —
//!   `workspace_lint_table_denies_disallowed_methods`;
//! - remove `[lints.clippy] disallowed_methods` from `wildside-fs/Cargo.toml` —
//!   `every_workspace_package_enforces_the_environment_policy`;
//! - remove `allow_attributes` from `wildside-fs/Cargo.toml` — the same test;
//! - drop a name from `EXPECTED_PACKAGES` — `workspace_contains_the_expected_packages`;
//! - drop `--all-targets` from `CLIPPY_FLAGS` —
//!   `clippy_gate_covers_every_workspace_target_and_feature`;
//! - give the CI lint step `make lint CLIPPY_FLAGS=--workspace` —
//!   `ci_runs_the_lint_gate_without_overriding_the_clippy_flags`.

use std::collections::BTreeSet;
use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use toml::Value;

/// Error type carried by the policy helpers and tests.
type Failure = Box<dyn std::error::Error>;

/// Environment methods that no package may call outside a composition root.
const FORBIDDEN_ENVIRONMENT_METHODS: [&str; 6] = [
    "std::env::var",
    "std::env::var_os",
    "std::env::vars",
    "std::env::vars_os",
    "std::env::set_var",
    "std::env::remove_var",
];

/// Every package this workspace contains, enumerated rather than sampled.
const EXPECTED_PACKAGES: [&str; 8] = [
    "wildside-cli",
    "wildside-core",
    "wildside-data",
    "wildside-engine",
    "wildside-fs",
    "wildside-scorer",
    "wildside-solver-ortools",
    "wildside-solver-vrp",
];

/// Hygiene lints a package must deny alongside `disallowed_methods`.
///
/// An item-scoped `#[allow(clippy::disallowed_methods)]` lowers a deny. These
/// two reject that attribute, leaving the reasoned `#[expect(..)]` at a
/// composition root as the only escape.
const REQUIRED_HYGIENE_LINTS: [&str; 2] = ["allow_attributes", "allow_attributes_without_reason"];

/// Return `Err` carrying `message` when `condition` does not hold.
fn ensure_that(condition: bool, message: String) -> Result<(), Failure> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

/// Return the workspace root as a Camino path.
fn workspace_root_path() -> &'static Utf8Path {
    Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Open a capability handle on the workspace root.
fn workspace_root() -> Result<Dir, Failure> {
    let root = workspace_root_path();
    Dir::open_ambient_dir(root, ambient_authority())
        .map_err(|err| -> Failure { format!("open workspace root {root}: {err}").into() })
}

/// Read a file held under the workspace root.
fn read_file(root: &Dir, relative: &str) -> Result<String, Failure> {
    root.read_to_string(relative)
        .map_err(|err| -> Failure { format!("read {relative}: {err}").into() })
}

/// Read and parse a TOML document held under the workspace root.
fn read_toml(root: &Dir, relative: &str) -> Result<Value, Failure> {
    let text = read_file(root, relative)?;
    toml::from_str::<Value>(&text)
        .map_err(|err| -> Failure { format!("parse {relative}: {err}").into() })
}

/// Return the `disallowed-methods` paths declared by the Clippy configuration.
fn disallowed_method_paths(root: &Dir) -> Result<Vec<String>, Failure> {
    let policy = read_toml(root, "clippy.toml")?;
    let methods = policy
        .get("disallowed-methods")
        .and_then(Value::as_array)
        .ok_or_else(|| -> Failure {
            "clippy.toml must declare a disallowed-methods array".into()
        })?;
    Ok(methods
        .iter()
        .filter_map(|method| method.get("path").and_then(Value::as_str))
        .map(str::to_owned)
        .collect())
}

/// One workspace package as Cargo resolved it.
struct PackageEntry {
    /// The package name.
    name: String,
    /// The package manifest, relative to the workspace root.
    manifest: Utf8PathBuf,
}

/// Ask Cargo for the workspace packages, including implicit path members.
fn workspace_packages() -> Result<Vec<PackageEntry>, Failure> {
    let root = workspace_root_path();
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(root)
        .output()
        .map_err(|err| -> Failure { format!("run cargo metadata: {err}").into() })?;
    ensure_that(
        output.status.success(),
        format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    parse_package_entries(&output.stdout, root)
}

/// Turn `cargo metadata` output into workspace-relative package entries.
fn parse_package_entries(stdout: &[u8], root: &Utf8Path) -> Result<Vec<PackageEntry>, Failure> {
    let metadata: serde_json::Value = serde_json::from_slice(stdout)
        .map_err(|err| -> Failure { format!("parse cargo metadata: {err}").into() })?;
    let packages = metadata
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| -> Failure { "cargo metadata must report packages".into() })?;
    packages
        .iter()
        .map(|package| package_entry(package, root))
        .collect()
}

/// Convert one `cargo metadata` package object into a `PackageEntry`.
fn package_entry(package: &serde_json::Value, root: &Utf8Path) -> Result<PackageEntry, Failure> {
    let name = package
        .get("name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| -> Failure { "a package entry must carry a name".into() })?;
    let manifest = package
        .get("manifest_path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| -> Failure {
            format!("package {name} must carry a manifest_path").into()
        })?;
    let relative = Utf8Path::new(manifest)
        .strip_prefix(root)
        .map_err(|_| -> Failure { format!("{manifest} lies outside {root}").into() })?;
    Ok(PackageEntry {
        name: name.to_owned(),
        manifest: relative.to_owned(),
    })
}

/// Return the level a lint table assigns to `lint`, if any.
fn lint_level<'a>(clippy: &'a Value, lint: &str) -> Option<&'a str> {
    clippy.get(lint).and_then(Value::as_str)
}

/// Explain why a package manifest fails to enforce the policy, or return `None`.
///
/// A package qualifies by inheriting the workspace lint table or, until issue
/// #124 completes that migration, by denying `disallowed_methods` and the two
/// hygiene lints itself.
fn policy_gap(manifest: &Value) -> Option<String> {
    let lints = manifest.get("lints")?;
    if lints.get("workspace").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let clippy = lints.get("clippy")?;
    for lint in std::iter::once("disallowed_methods").chain(REQUIRED_HYGIENE_LINTS) {
        if lint_level(clippy, lint) != Some("deny") {
            return Some(format!("{lint} is not denied"));
        }
    }
    None
}

/// Scenario: a contributor edits the Clippy configuration.
///
/// Invariant: every prohibited environment method keeps its entry, so Clippy
/// still has something to act on.
#[test]
fn clippy_config_disallows_every_environment_method() -> Result<(), Failure> {
    let root = workspace_root()?;
    let paths = disallowed_method_paths(&root)?;
    for required in FORBIDDEN_ENVIRONMENT_METHODS {
        ensure_that(
            paths.iter().any(|path| path == required),
            format!("clippy.toml must disallow {required}, found {paths:?}"),
        )?;
    }
    Ok(())
}

/// Scenario: the Clippy configuration is intact but the lint is not denied.
///
/// Invariant: the workspace lint table denies `disallowed_methods`, so the
/// entries are enforced rather than merely declared.
#[test]
fn workspace_lint_table_denies_disallowed_methods() -> Result<(), Failure> {
    let root = workspace_root()?;
    let manifest = read_toml(&root, "Cargo.toml")?;
    let level = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("lints"))
        .and_then(|lints| lints.get("clippy"))
        .and_then(|clippy| clippy.get("disallowed_methods"))
        .and_then(Value::as_str);
    ensure_that(
        level == Some("deny"),
        format!("workspace.lints.clippy.disallowed_methods must be \"deny\", found {level:?}"),
    )
}

/// Scenario: a package is added to or removed from the workspace.
///
/// Invariant: Cargo resolves exactly the eight packages this contract knows
/// about, so a new member cannot slip past the enforcement test unexamined.
#[test]
fn workspace_contains_the_expected_packages() -> Result<(), Failure> {
    let resolved: BTreeSet<String> = workspace_packages()?
        .into_iter()
        .map(|package| package.name)
        .collect();
    let expected: BTreeSet<String> = EXPECTED_PACKAGES
        .iter()
        .map(|&name| name.to_owned())
        .collect();
    ensure_that(
        resolved == expected,
        format!("cargo resolved {resolved:?}, this contract expects {expected:?}"),
    )
}

/// Scenario: a workspace package opts out of the shared lint configuration.
///
/// Invariant: every member Cargo resolves enforces `disallowed_methods`, either
/// by inheriting the workspace table or by denying that rule and the two
/// hygiene lints that stop an `#[allow]` lowering it.
#[test]
fn every_workspace_package_enforces_the_environment_policy() -> Result<(), Failure> {
    let root = workspace_root()?;
    for package in workspace_packages()? {
        let manifest = read_toml(&root, package.manifest.as_str())?;
        if let Some(gap) = policy_gap(&manifest) {
            let name = package.name;
            let path = package.manifest;
            return Err(format!("{name} ({path}) does not enforce the policy: {gap}").into());
        }
    }
    Ok(())
}

/// Scenario: the Clippy gate is narrowed to the default target and features.
///
/// Invariant: the lint target runs Clippy across every workspace package,
/// target kind and feature with warnings denied, so test code is covered too.
#[test]
fn clippy_gate_covers_every_workspace_target_and_feature() -> Result<(), Failure> {
    let root = workspace_root()?;
    let makefile = read_file(&root, "Makefile")?;
    ensure_that(
        makefile
            .contains("CLIPPY_FLAGS ?= --workspace --all-targets --all-features -- -D warnings"),
        "CLIPPY_FLAGS must cover every workspace target and feature with warnings denied"
            .to_owned(),
    )?;
    ensure_that(
        makefile.contains("$(CARGO) clippy $(CLIPPY_FLAGS)"),
        "the lint target must invoke Cargo Clippy with the workspace-wide contract".to_owned(),
    )
}

/// Scenario: the CI lint step passes its own Clippy flags to Make.
///
/// Invariant: CI runs `make lint` bare, so the flags asserted above are the
/// flags CI actually uses. `CLIPPY_FLAGS ?=` is overridable by design for local
/// work; the gate that blocks a merge must not exercise that.
#[test]
fn ci_runs_the_lint_gate_without_overriding_the_clippy_flags() -> Result<(), Failure> {
    let root = workspace_root()?;
    let workflow = read_file(&root, ".github/workflows/ci.yml")?;
    let lint_steps: Vec<&str> = workflow
        .lines()
        .map(str::trim)
        .filter(|line| line.contains("make lint"))
        .collect();
    ensure_that(
        lint_steps.contains(&"run: make lint"),
        format!("ci.yml must run `make lint` bare, found {lint_steps:?}"),
    )?;
    ensure_that(
        !lint_steps.iter().any(|line| line.contains("CLIPPY_FLAGS")),
        format!("no CI lint step may override CLIPPY_FLAGS, found {lint_steps:?}"),
    )
}
