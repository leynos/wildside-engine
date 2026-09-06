//! Contract coverage for the environment-access lint policy (issue #126).
//!
//! The policy has four moving parts, and removing any one of them silently
//! reopens ambient process-environment access: the `disallowed-methods` entries
//! in the root `clippy.toml`, the workspace `disallowed_methods` deny that makes
//! Clippy act on them, each package's enforcement of that deny, and the Make
//! target that runs Clippy over every workspace target and feature.
//!
//! Mutation proof (run 2026-09-06): deleting the `std::env::set_var` entry from
//! `clippy.toml` failed `clippy_config_disallows_every_environment_method`;
//! changing the workspace `disallowed_methods` deny to `"warn"` failed
//! `workspace_lint_table_denies_disallowed_methods`; removing
//! `[lints.clippy] disallowed_methods` from `wildside-fs/Cargo.toml` failed
//! `every_workspace_package_enforces_the_environment_policy`; and dropping
//! `--all-targets` from `CLIPPY_FLAGS` failed
//! `clippy_gate_covers_every_workspace_target_and_feature`. Each mutation broke
//! only its own test.

use camino::Utf8Path;
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

/// Return `Err` carrying `message` when `condition` does not hold.
fn ensure_that(condition: bool, message: String) -> Result<(), Failure> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

/// Open a capability handle on the workspace root.
fn workspace_root() -> Result<Dir, Failure> {
    let root = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    Dir::open_ambient_dir(root, ambient_authority())
        .map_err(|err| -> Failure { format!("open workspace root {root}: {err}").into() })
}

/// Read and parse a TOML document held under the workspace root.
fn read_toml(root: &Dir, relative: &str) -> Result<Value, Failure> {
    let text = root
        .read_to_string(relative)
        .map_err(|err| -> Failure { format!("read {relative}: {err}").into() })?;
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

/// Return the workspace member directories named by the root manifest.
fn workspace_members(root: &Dir) -> Result<Vec<String>, Failure> {
    let manifest = read_toml(root, "Cargo.toml")?;
    let members = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(Value::as_array)
        .ok_or_else(|| -> Failure { "Cargo.toml must declare workspace members".into() })?;
    Ok(members
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect())
}

/// Report whether a package manifest enforces the environment-access rule.
///
/// A package qualifies either by inheriting the workspace lint table or, until
/// issue #124 completes that migration, by denying `disallowed_methods` itself.
fn enforces_environment_policy(manifest: &Value) -> bool {
    let Some(lints) = manifest.get("lints") else {
        return false;
    };
    if lints.get("workspace").and_then(Value::as_bool) == Some(true) {
        return true;
    }
    lints
        .get("clippy")
        .and_then(|clippy| clippy.get("disallowed_methods"))
        .and_then(Value::as_str)
        == Some("deny")
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

/// Scenario: a workspace package opts out of the shared lint configuration.
///
/// Invariant: every member enforces `disallowed_methods`, whether by inheriting
/// the workspace table or by denying the single rule in its own manifest.
#[test]
fn every_workspace_package_enforces_the_environment_policy() -> Result<(), Failure> {
    let root = workspace_root()?;
    for member in workspace_members(&root)? {
        let relative = format!("{member}/Cargo.toml");
        let manifest = read_toml(&root, &relative)?;
        ensure_that(
            enforces_environment_policy(&manifest),
            format!(
                "{relative} must inherit the workspace lints or deny disallowed_methods itself"
            ),
        )?;
    }
    let root_manifest = read_toml(&root, "Cargo.toml")?;
    ensure_that(
        enforces_environment_policy(&root_manifest),
        "the root package must inherit the workspace lints".to_owned(),
    )
}

/// Scenario: the Clippy gate is narrowed to the default target and features.
///
/// Invariant: the lint target runs Clippy across every workspace package,
/// target kind and feature with warnings denied, so test code is covered too.
#[test]
fn clippy_gate_covers_every_workspace_target_and_feature() -> Result<(), Failure> {
    let root = workspace_root()?;
    let makefile = root
        .read_to_string("Makefile")
        .map_err(|err| -> Failure { format!("read Makefile: {err}").into() })?;
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
