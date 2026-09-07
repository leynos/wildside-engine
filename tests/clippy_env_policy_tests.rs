//! Contract coverage for the environment-access lint policy (issue #126).
//!
//! `clippy_env_policy_ui_tests.rs` proves the lint fires. This file guards the
//! configuration that makes it fire: the `disallowed-methods` entries in the
//! root `clippy.toml`, the workspace `disallowed_methods` deny, each package's
//! enforcement of that deny, the hygiene lints that stop an item-scoped
//! `#[allow]` lowering it, and the Make target that runs Clippy over every
//! workspace target and feature.
//!
//! Every file read here is embedded with `include_str!`, so moving or deleting
//! one is a compile failure rather than a runtime error, and the test needs no
//! filesystem access at all. The eight manifests can be named statically
//! because `workspace_contains_the_expected_packages` holds the resolved
//! package set to exactly those eight names.
//!
//! That set comes from `cargo metadata`, not from `[workspace].members`,
//! because Cargo also promotes an in-tree path dependency to a workspace member
//! without an entry in that array. A member added, removed, or known only to
//! Cargo therefore fails this contract instead of escaping it.
//!
//! Mutation proof, re-run in full against this mechanism on 2026-09-07:
//!
//! - delete the `std::env::set_var` entry from `clippy.toml` —
//!   `clippy_config_disallows_every_environment_method`, and also
//!   `every_prohibited_method_is_rejected_with_its_guidance` in the UI tests,
//!   because the two files guard the same six entries from different angles.
//!   Deleting an entry is the one mutation that legitimately fails both;
//! - change the workspace `disallowed_methods` deny to `"warn"` —
//!   `workspace_lint_table_denies_disallowed_methods`;
//! - remove `[lints.clippy] disallowed_methods` from `wildside-fs/Cargo.toml` —
//!   `every_workspace_package_enforces_the_environment_policy`;
//! - remove `allow_attributes` from `wildside-fs/Cargo.toml` — the same test;
//! - rename an entry of `EXPECTED_PACKAGES` —
//!   `workspace_contains_the_expected_packages`;
//! - drop `--all-targets` from `CLIPPY_FLAGS` —
//!   `clippy_gate_covers_every_workspace_target_and_feature`;
//! - comment the assignment out and add a weaker one elsewhere — the same
//!   test, which an earlier whole-file `contains` survived;
//! - prefix either recipe line with `-` or `@-`, or append `|| true`,
//!   `; true`, or `| tee` to either — `no_lint_command_discards_its_exit_status`
//!   in every case, plus the coverage test where the Clippy line's text also
//!   changes. Before this test judged each command separately, the three
//!   appended forms and the `@-` prefix all passed on the Whitaker line;
//! - remove the `[lints]` table from `wildside-fs/Cargo.toml` —
//!   `every_workspace_package_enforces_the_environment_policy` again.
//!
//! Every mutation but the first fails exactly one test, and the unmutated tree
//! fails none.
//!
//! `tests/workflow_contracts/lint_gate_test.py` carries the matching assertion
//! about CI, which needs to read every `env` scope in the workflow and so
//! belongs with the other `PyYAML` workflow contracts.

use std::collections::BTreeSet;
use std::process::Command;

use toml::Value;

/// Error type carried by the helpers and tests.
type Failure = Box<dyn std::error::Error>;

/// The Clippy configuration this policy lives in.
const CLIPPY_CONFIG: &str = include_str!("../clippy.toml");

/// The root manifest, which carries the workspace lint table.
const ROOT_MANIFEST: &str = include_str!("../Cargo.toml");

/// The Make targets that run the lint gate.
const MAKEFILE: &str = include_str!("../Makefile");

/// Environment methods that no package may call outside a composition root.
const FORBIDDEN_ENVIRONMENT_METHODS: [&str; 6] = [
    "std::env::var",
    "std::env::var_os",
    "std::env::vars",
    "std::env::vars_os",
    "std::env::set_var",
    "std::env::remove_var",
];

/// Every package in this workspace, with its manifest embedded at compile time.
const EXPECTED_PACKAGES: [(&str, &str); 8] = [
    ("wildside-engine", ROOT_MANIFEST),
    ("wildside-cli", include_str!("../wildside-cli/Cargo.toml")),
    ("wildside-core", include_str!("../wildside-core/Cargo.toml")),
    ("wildside-data", include_str!("../wildside-data/Cargo.toml")),
    ("wildside-fs", include_str!("../wildside-fs/Cargo.toml")),
    (
        "wildside-scorer",
        include_str!("../wildside-scorer/Cargo.toml"),
    ),
    (
        "wildside-solver-ortools",
        include_str!("../wildside-solver-ortools/Cargo.toml"),
    ),
    (
        "wildside-solver-vrp",
        include_str!("../wildside-solver-vrp/Cargo.toml"),
    ),
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

/// Parse one of the embedded TOML documents.
fn parse_toml(label: &str, text: &str) -> Result<Value, Failure> {
    toml::from_str::<Value>(text)
        .map_err(|err| -> Failure { format!("parse {label}: {err}").into() })
}

/// Return the `disallowed-methods` paths declared by the Clippy configuration.
fn disallowed_method_paths() -> Result<Vec<String>, Failure> {
    let policy = parse_toml("clippy.toml", CLIPPY_CONFIG)?;
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

/// Ask Cargo for the workspace package names, including implicit path members.
fn resolved_package_names() -> Result<BTreeSet<String>, Failure> {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .map_err(|err| -> Failure { format!("run cargo metadata: {err}").into() })?;
    ensure_that(
        output.status.success(),
        format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    parse_package_names(&output.stdout)
}

/// Turn `cargo metadata` output into a set of package names.
fn parse_package_names(stdout: &[u8]) -> Result<BTreeSet<String>, Failure> {
    let metadata: serde_json::Value = serde_json::from_slice(stdout)
        .map_err(|err| -> Failure { format!("parse cargo metadata: {err}").into() })?;
    let packages = metadata
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| -> Failure { "cargo metadata must report packages".into() })?;
    packages
        .iter()
        .map(|package| {
            package
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| -> Failure { "a package entry must carry a name".into() })
        })
        .collect()
}

/// Explain why a package manifest fails to enforce the policy, or return `None`.
///
/// A package qualifies by inheriting the workspace lint table or, until issue
/// #124 completes that migration, by denying `disallowed_methods` and the two
/// hygiene lints itself.
fn policy_gap(manifest: &Value) -> Option<String> {
    let Some(lints) = manifest.get("lints") else {
        return Some("it declares no [lints] table".to_owned());
    };
    if lints.get("workspace").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let Some(clippy) = lints.get("clippy") else {
        return Some(
            "it neither inherits the workspace lints nor declares [lints.clippy]".to_owned(),
        );
    };
    for lint in std::iter::once("disallowed_methods").chain(REQUIRED_HYGIENE_LINTS) {
        if clippy.get(lint).and_then(Value::as_str) != Some("deny") {
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
    let paths = disallowed_method_paths()?;
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
    let manifest = parse_toml("Cargo.toml", ROOT_MANIFEST)?;
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
/// Invariant: Cargo resolves exactly the eight packages whose manifests this
/// contract embeds, so a new member cannot slip past the enforcement test.
#[test]
fn workspace_contains_the_expected_packages() -> Result<(), Failure> {
    let resolved = resolved_package_names()?;
    let expected: BTreeSet<String> = EXPECTED_PACKAGES
        .iter()
        .map(|&(name, _)| name.to_owned())
        .collect();
    ensure_that(
        resolved == expected,
        format!("cargo resolved {resolved:?}, this contract expects {expected:?}"),
    )
}

/// Scenario: a workspace package opts out of the shared lint configuration.
///
/// Invariant: every package enforces `disallowed_methods`, either by inheriting
/// the workspace table or by denying that rule and the two hygiene lints that
/// stop an `#[allow]` lowering it.
#[test]
fn every_workspace_package_enforces_the_environment_policy() -> Result<(), Failure> {
    for (name, text) in EXPECTED_PACKAGES {
        let manifest = parse_toml(name, text)?;
        if let Some(gap) = policy_gap(&manifest) {
            return Err(format!("{name} does not enforce the policy: {gap}").into());
        }
    }
    Ok(())
}

/// The flag assignment the lint gate must carry, as a whole line.
const CLIPPY_FLAGS_ASSIGNMENT: &str =
    "CLIPPY_FLAGS ?= --workspace --all-targets --all-features -- -D warnings";

/// The Clippy invocation the lint recipe must carry, as a whole command.
const CLIPPY_INVOCATION: &str = "$(CARGO) clippy $(CLIPPY_FLAGS)";

/// Return the command lines of a Makefile target's recipe.
///
/// Recipe lines are the tab-indented lines following the target line, up to the
/// first line that is neither indented nor blank.
fn recipe_lines(target: &str) -> Vec<&'static str> {
    let mut lines = MAKEFILE
        .lines()
        .skip_while(|line| !line.starts_with(target));
    lines.next();
    lines
        .take_while(|line| line.starts_with('\t') || line.trim().is_empty())
        .filter_map(|line| line.strip_prefix('\t'))
        .map(str::trim_end)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// Shell operators that hand a command's exit status to something else.
///
/// `||` substitutes a fallback's status, `|` substitutes the last stage's, and
/// `;` substitutes the next command's. Each leaves a failing gate reporting
/// success, so the lint recipe uses none of them.
const STATUS_DISCARDING_OPERATORS: [&str; 3] = ["||", "|", ";"];

/// Split a recipe line into make's prefix characters and the command itself.
///
/// Make accepts `@`, `-` and `+` in any order and combination. Reading the run
/// as a whole matters: an assertion that only rejects a leading `-` accepts
/// `@-`, which silences the command and ignores its status together.
fn split_prefix(line: &str) -> (&str, &str) {
    let command = line.trim_start_matches(['@', '-', '+']);
    let prefix = line.get(..line.len() - command.len()).unwrap_or_default();
    (prefix, command)
}

/// Scenario: the Clippy gate is narrowed to the default target and features.
///
/// Invariant: the lint recipe runs Clippy across every workspace package,
/// target kind and feature with warnings denied, so test code is covered too.
///
/// Both assertions judge whole lines rather than substrings. A substring match
/// on the invocation passes when the recipe reads `-$(CARGO) clippy ...`, which
/// tells make to ignore the exit status: the gate then prints a real finding
/// and exits 0. Measured on this repository before this test was tightened.
#[test]
fn clippy_gate_covers_every_workspace_target_and_feature() -> Result<(), Failure> {
    ensure_that(
        MAKEFILE
            .lines()
            .map(str::trim_end)
            .any(|line| line == CLIPPY_FLAGS_ASSIGNMENT),
        format!("the Makefile must assign exactly {CLIPPY_FLAGS_ASSIGNMENT:?}"),
    )?;
    let recipe = recipe_lines("lint:");
    ensure_that(
        recipe
            .iter()
            .any(|line| split_prefix(line).1 == CLIPPY_INVOCATION),
        format!("the lint recipe must run exactly {CLIPPY_INVOCATION:?}, found {recipe:?}"),
    )
}

/// Scenario: a lint command's failure is made not to fail the target.
///
/// Invariant: every command in the lint recipe carries its own exit status to
/// make. This is checked per command, not on the recipe as a whole, because a
/// gate is only as strong as its weakest line: appending `|| true` to the
/// Whitaker command leaves the Clippy command untouched and every whole-recipe
/// assertion satisfied, while the suite it disables reports success.
#[test]
fn no_lint_command_discards_its_exit_status() -> Result<(), Failure> {
    for line in recipe_lines("lint:") {
        let (prefix, command) = split_prefix(line);
        ensure_that(
            !prefix.contains('-'),
            format!("a lint command must not tell make to ignore its status: {line:?}"),
        )?;
        for operator in STATUS_DISCARDING_OPERATORS {
            ensure_that(
                !command.contains(operator),
                format!("a lint command must not hand its status to {operator:?}: {line:?}"),
            )?;
        }
    }
    Ok(())
}
