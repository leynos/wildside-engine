//! Compile-time coverage for the environment-access lint policy (issue #126).
//!
//! `clippy_env_policy_tests.rs` asserts that the configuration says the right
//! thing. This file asserts that the configuration *does* the right thing: it
//! runs Clippy over the fixture crate in `tests/fixtures/env_policy_probe` and
//! reads the diagnostics. The fixture is excluded from the workspace, so its
//! deliberately offending probes never reach `cargo clippy --workspace`, and
//! Clippy finds the repository's own `clippy.toml` by walking up from the
//! fixture directory. The reason strings asserted here are therefore the ones a
//! contributor sees.
//!
//! Each probe sits behind its own feature so one invocation reports one
//! diagnostic, and the no-feature control proves a failure is attributable to
//! the probe under test rather than to the fixture itself.
//!
//! Mutation proof (run 2026-09-07). Each mutation failed only the test named
//! beside it:
//!
//! - change the `std::env::var` entry's reason string in `clippy.toml` —
//!   `every_prohibited_method_is_rejected_with_its_guidance`, on the guidance
//!   assertion. An earlier draft searched the whole output for the reason and
//!   survived this mutation, because the three sibling entries share that
//!   string. Slicing the diagnostic block per method is what caught it;
//! - remove `allow_attributes` from the fixture manifest —
//!   `an_allow_attribute_cannot_lower_the_deny`;
//! - remove the `reason` from the fixture's `#[expect]` —
//!   `a_reasoned_expect_at_a_composition_root_is_accepted`.
//!
//! Deleting the `std::env::var` entry outright fails
//! `every_prohibited_method_is_rejected_with_its_guidance` and also
//! `a_reasoned_expect_at_a_composition_root_is_accepted`, because the fixture's
//! `#[expect]` then goes unfulfilled and warns. That coupling is inherent: the
//! sanctioned escape only means anything while the method is disallowed.

use std::process::{Command, Output};

/// Error type carried by the helpers and tests.
type Failure = Box<dyn std::error::Error>;

/// Return `Err` carrying `message` when `condition` does not hold.
fn ensure_that(condition: bool, message: String) -> Result<(), Failure> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

/// Lint the fixture crate with `features` enabled and return Clippy's output.
///
/// The build lands in this test target's own temporary directory, so it shares
/// nothing with the workspace build that is running this test.
fn lint_probe(features: &str) -> Result<Output, Failure> {
    let manifest = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/env_policy_probe/Cargo.toml"
    );
    let target_dir = concat!(env!("CARGO_TARGET_TMPDIR"), "/env-policy-ui");
    let mut command = Command::new(env!("CARGO"));
    // Name the repository's configuration outright rather than relying on
    // Clippy's walk up from the fixture directory.
    command.env("CLIPPY_CONF_DIR", env!("CARGO_MANIFEST_DIR"));
    command.args([
        "clippy",
        "--manifest-path",
        manifest,
        "--target-dir",
        target_dir,
    ]);
    if !features.is_empty() {
        command.args(["--features", features]);
    }
    command
        .args(["--", "-D", "warnings"])
        .output()
        .map_err(|err| -> Failure { format!("run clippy on the {features} probe: {err}").into() })
}

/// Lint a probe and require that Clippy rejected it, returning the diagnostics.
fn rejected_probe(features: &str) -> Result<String, Failure> {
    let output = lint_probe(features)?;
    let diagnostics = String::from_utf8_lossy(&output.stderr).into_owned();
    ensure_that(
        !output.status.success(),
        format!("Clippy accepted the {features} probe; it must reject it\n{diagnostics}"),
    )?;
    Ok(diagnostics)
}

/// Lint a probe and require that Clippy accepted it.
fn accepted_probe(features: &str) -> Result<(), Failure> {
    let output = lint_probe(features)?;
    let label = if features.is_empty() {
        "no-feature"
    } else {
        features
    };
    ensure_that(
        output.status.success(),
        format!(
            "Clippy rejected the {label} probe; it must accept it\n{}",
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

/// Require that `diagnostics` mentions `needle`.
fn require_diagnostic(diagnostics: &str, needle: &str) -> Result<(), Failure> {
    ensure_that(
        diagnostics.contains(needle),
        format!("Clippy should have reported {needle:?}, got:\n{diagnostics}"),
    )
}

/// Return the single diagnostic that starts with `header`, up to the next one.
///
/// Clippy prints the guidance as a `= note:` line inside the block belonging to
/// the offending call. Slicing the block is what ties a reason string to its
/// method: asserting against the whole output would let any one entry's reason
/// satisfy every other entry.
fn diagnostic_block<'a>(diagnostics: &'a str, header: &str) -> Option<&'a str> {
    let start = diagnostics.find(header)?;
    let block = diagnostics.get(start..)?;
    block
        .get(header.len()..)
        .and_then(|rest| rest.find("\nerror"))
        .map_or(Some(block), |offset| block.get(..header.len() + offset))
}

/// Scenario: the fixture crate with none of its probes enabled.
///
/// Invariant: the fixture lints clean, so a failure in the tests below is
/// attributable to the probe under test and not to the fixture.
#[test]
fn the_fixture_without_a_probe_lints_clean() -> Result<(), Failure> {
    accepted_probe("")
}

/// Every prohibited method, paired with the guidance `clippy.toml` gives.
const PROHIBITED_METHODS: [(&str, &str); 6] = [
    ("std::env::var", "inject an environment reader"),
    ("std::env::var_os", "inject an environment reader"),
    ("std::env::vars", "inject an environment reader"),
    ("std::env::vars_os", "inject an environment reader"),
    ("std::env::set_var", "use a stub environment in tests"),
    ("std::env::remove_var", "use a stub environment in tests"),
];

/// Scenario: a contributor reads or mutates the process environment directly.
///
/// Invariant: Clippy rejects every one of the six calls and names the seam to
/// reach for instead, so the guidance in `clippy.toml` reaches the contributor
/// rather than sitting unread in a configuration file.
#[test]
fn every_prohibited_method_is_rejected_with_its_guidance() -> Result<(), Failure> {
    let diagnostics = rejected_probe("bare-read")?;
    for (method, guidance) in PROHIBITED_METHODS {
        let header = format!("error: use of a disallowed method `{method}`");
        let block = diagnostic_block(&diagnostics, &header).ok_or_else(|| -> Failure {
            format!("Clippy should have reported {header:?}, got:\n{diagnostics}").into()
        })?;
        ensure_that(
            block.contains(guidance),
            format!("the {method} diagnostic should carry {guidance:?}, got:\n{block}"),
        )?;
    }
    Ok(())
}

/// Scenario: a contributor silences the deny with `#[allow]`.
///
/// Invariant: the attribute is itself rejected, so the policy cannot be lowered
/// at an ordinary call site.
#[test]
fn an_allow_attribute_cannot_lower_the_deny() -> Result<(), Failure> {
    let diagnostics = rejected_probe("allow-bypass")?;
    require_diagnostic(&diagnostics, "#[allow] attribute found")
}

/// Scenario: a genuine composition root reads the environment.
///
/// Invariant: an item-scoped `#[expect]` carrying a reason compiles, so the
/// sanctioned escape works and the policy stays usable.
#[test]
fn a_reasoned_expect_at_a_composition_root_is_accepted() -> Result<(), Failure> {
    accepted_probe("expect-escape")
}
