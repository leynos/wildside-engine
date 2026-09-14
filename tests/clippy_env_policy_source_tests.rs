//! Source-scan contract for the environment-access policy (issue #126).
//!
//! `clippy_env_policy_tests.rs` guards the configuration and
//! `clippy_env_policy_ui_tests.rs` proves the lint fires. Neither sees a source
//! file that switches the lint off for itself, and `clippy::allow_attributes`
//! does not fire on inner attributes, so a crate root can disarm the policy
//! with every other gate green.
//!
//! Measured on `main` at 4fd0c1640db6256df9634c13f29f9e9b485ccbe0: with
//! `#![allow(clippy::disallowed_methods, reason = "..")]` in `wildside-fs`'s
//! crate root and an unannotated `std::env::var("HOME")` beside it, `make lint`
//! exited 0 with no findings, and all ten configuration and compile-time
//! contract tests passed.
//!
//! `expect` is judged by scope rather than waved through. An item-scoped
//! `#[expect]` is the sanctioned form at a composition root precisely because
//! it covers one item, still reports the lint everywhere else, and warns once
//! that site grows a seam. A crate-scoped `#![expect]` is not the same thing:
//! one matching call fulfils it for the whole crate, silencing every other call
//! without even an unfulfilled-expectation warning.
//!
//! The sources are parsed rather than searched. A text scan cannot follow
//! `cfg_attr`, cannot tell an attribute from attribute-shaped text in a string
//! literal, breaks on a parenthesis inside a `reason`, and misses a space
//! before the parenthesis. Naming the lint alone is also not enough: Clippy
//! places `disallowed_methods` in the `style` group, so `clippy::style`,
//! `clippy::all` and `warnings` each switch it off without ever naming it.
//!
//! Mutation proof, recorded 2026-09-08. Each was applied alone to a real
//! source file, run through the build, and reverted:
//!
//! - `#![allow(clippy::disallowed_methods)]` in a crate root fails
//!   `no_source_allows_a_policy_lint`;
//! - `#![allow(clippy::style)]`, naming the group rather than the lint, fails
//!   it;
//! - `#![cfg_attr(all(), allow(clippy::disallowed_methods))]` fails it;
//! - `#![allow(clippy::all)]` wrapped across several lines fails it;
//! - `#[allow(warnings)]` on an item fails it;
//! - an `#![allow(clippy::disallowed_methods)]` emitted from a `macro_rules!`
//!   arm fails it, at one macro depth and at two, because `syn` keeps an arm's
//!   body an opaque token stream that never reaches `visit_attribute` while
//!   Clippy expands and honours it;
//! - a crate-scoped `#![expect(clippy::disallowed_methods, reason = "..")]`
//!   fails it. Measured with Clippy: with two `std::env::var` calls in the
//!   crate it reports nothing and exits 0, because one call fulfils the
//!   expectation crate-wide and no unfulfilled-expectation warning is raised.
//!   The item-scoped form on one of the two functions still reports the other
//!   and exits 101, which is why `expect` is judged by scope rather than
//!   ignored;
//! - `#![allow(clippy::r#style)]`, spelt with a raw identifier, fails it;
//! - `#[expect(clippy::disallowed_methods, reason = "..")]` on one function
//!   must, and does, keep passing;
//! - `#[allow(clippy::allow_attributes)]` on an item must, and does, keep
//!   passing. That case keeps this contract honest: the text scan this replaced
//!   compared names by substring and would have reported it as suppressing
//!   `clippy::all`, whose name it contains.
//!
//! The contract is split across four files to stay within the 400-line limit.
//! `env_policy_scan/support.rs` owns the source set and the workspace handle,
//! `env_policy_scan/scan.rs` owns the attribute and token walk,
//! `env_policy_scan/properties.rs` states the scan's judgement as properties
//! over generated lint names, forms and reason strings, and the contracts below
//! own the example-based judgements.
//! The sample sources live in `tests/fixtures/env_policy_samples` and are
//! loaded with `include_str!`. Their `.rs.txt` extension is deliberate: a
//! tracked `.rs` sample would be scanned by `no_source_allows_a_policy_lint`
//! itself and would force a second exemption.
//!
//! The split was re-proved on 2026-09-14, because a mutation proof is only
//! evidence about the mechanism it was run against. Each was applied alone and
//! run through the build: `#![allow(clippy::disallowed_methods)]` at the top of
//! `wildside-fs/src/lib.rs` fails `no_source_allows_a_policy_lint`, and
//! `#![allow(clippy::style)]` at the top of the extracted scan module fails it
//! too, which is what shows the moved files are still in the scanned set.

// An integration test's crate root resolves `mod` against `tests/`, not against
// a directory of its own, so each module names its path explicitly. The
// directory is `env_policy_scan` rather than the test's own name because
// `clippy::self_named_module_files` rejects a `foo.rs` beside a `foo/`. Files
// under a `tests/` subdirectory are not built as test binaries of their own,
// which is what keeps this contract in one binary.
#[path = "env_policy_scan/properties.rs"]
mod properties;
#[path = "env_policy_scan/scan.rs"]
mod scan;
#[path = "env_policy_scan/support.rs"]
mod support;

use rstest::rstest;
use scan::suppressed_lints;
use support::{EXEMPT_SOURCES, Failure, ensure_that, scannable_sources, workspace_root};

/// Return `Ok` when the scan reports exactly one finding in `source`.
fn exactly_one_finding(source: &str) -> Result<(), Failure> {
    let found = suppressed_lints(source)?;
    ensure_that(
        found.len() == 1,
        format!("{source:?} must yield one finding, got {found:?}"),
    )
}

/// Return `Ok` when the scan reports nothing in `source`.
fn no_findings(source: &str) -> Result<(), Failure> {
    let found = suppressed_lints(source)?;
    ensure_that(
        found.is_empty(),
        format!("{source:?} must yield no finding, got {found:?}"),
    )
}

/// Scenario: a source file switches the policy lint off for itself.
///
/// Invariant: no tracked source allows a protected lint. An inner attribute is
/// the case that matters, because `clippy::allow_attributes` cannot see one, so
/// nothing else in the repository would notice.
#[test]
fn no_source_allows_a_policy_lint() -> Result<(), Failure> {
    let root = workspace_root()?;
    let sources = scannable_sources()?;
    ensure_that(
        sources.len() > 20,
        format!(
            "the scan should cover the workspace, found {} sources",
            sources.len()
        ),
    )?;
    let mut offences = Vec::new();
    for path in sources {
        let contents = root
            .read_to_string(path.as_str())
            .map_err(|err| -> Failure { format!("read {path}: {err}").into() })?;
        let found = suppressed_lints(&contents)
            .map_err(|err| -> Failure { format!("{path} should parse as Rust: {err}").into() })?;
        for (lint, attribute) in found {
            offences.push(format!("{path} allows {lint} via {attribute}"));
        }
    }
    ensure_that(
        offences.is_empty(),
        format!(
            "no source may allow a protected lint; use an item-scoped \
             #[expect(..., reason = \"...\")] at a composition root instead:\n{}",
            offences.join("\n")
        ),
    )
}

/// Scenario: the scan is pointed at sources that no longer exist.
///
/// Invariant: it reaches each package's root module, so it fails loudly rather
/// than passing by finding nothing, which is how a source scan usually rots.
#[rstest]
#[case("src/lib.rs")]
#[case("wildside-cli/src/main.rs")]
#[case("wildside-core/src/lib.rs")]
#[case("wildside-data/src/lib.rs")]
#[case("wildside-fs/src/lib.rs")]
#[case("wildside-scorer/src/lib.rs")]
#[case("wildside-solver-ortools/src/lib.rs")]
#[case("wildside-solver-vrp/src/lib.rs")]
fn the_scan_reaches_every_package_root(#[case] package: &str) -> Result<(), Failure> {
    let sources = scannable_sources()?;
    ensure_that(
        sources.iter().any(|path| path.as_str() == package),
        format!("the scan should reach {package}"),
    )
}

/// Scenario: a contributor adds their own file to the exemption list.
///
/// Invariant: the list names only the compile-time fixture. An exemption that
/// can grow silently is a hole with a comment beside it.
#[test]
fn only_the_ui_fixture_is_exempt() -> Result<(), Failure> {
    ensure_that(
        EXEMPT_SOURCES == ["tests/fixtures/env_policy_probe/src/lib.rs"],
        format!("only the fixture may be exempt, found {EXEMPT_SOURCES:?}"),
    )
}

/// Scenario: an item-scoped `expect` at a composition root, plain and
/// conditional.
///
/// Invariant: neither is an offence. It covers one item, still reports the lint
/// everywhere else, and warns once that item grows a seam, which is what makes
/// it the sanctioned form. `cfg_attr` carries the outer item scope down, so the
/// conditional spelling is judged the same way. Rejecting either would push
/// contributors towards `allow`, the attribute that never warns.
#[rstest]
#[case::plain(include_str!("fixtures/env_policy_samples/sanctioned_item_expect.rs.txt"))]
#[case::conditional(include_str!(
    "fixtures/env_policy_samples/sanctioned_conditional_expect.rs.txt"
))]
fn an_item_scoped_expect_remains_sanctioned(#[case] source: &str) -> Result<(), Failure> {
    no_findings(source)
}

/// Scenario: the suppression is nested inside a `cfg_attr`.
///
/// Invariant: it is reported. Clippy honours the nested `allow` and
/// `clippy::allow_attributes` does not report it, so a scan looking for a line
/// beginning `#![allow(` would miss it entirely.
#[test]
fn a_suppression_nested_in_cfg_attr_is_an_offence() -> Result<(), Failure> {
    exactly_one_finding("#![cfg_attr(all(), allow(clippy::disallowed_methods, reason = \"x\"))]\n")
}

/// Scenario: the suppression names a group rather than the lint.
///
/// Invariant: each is reported. `disallowed_methods` sits in `style`, so
/// `clippy::style` switches the policy off while never naming it, and the wider
/// spellings do the same. Each group is its own case, so a spelling that stops
/// being caught is named in the failure rather than hidden behind an earlier
/// one.
#[rstest]
#[case::style("clippy::style")]
#[case::all("clippy::all")]
#[case::warnings("warnings")]
fn a_suppression_of_a_lints_group_is_an_offence(#[case] group: &str) -> Result<(), Failure> {
    exactly_one_finding(&format!("#![allow({group})]\n"))
}

/// Scenario: awkward spacing, and a parenthesis inside the reason string.
///
/// Invariant: both are handled. A scan matching a fixed opener would miss the
/// spaced form, and one counting raw brackets would end the attribute early at
/// the parenthesis inside the string.
#[test]
fn spacing_and_a_parenthesis_in_the_reason_do_not_hide_a_suppression() -> Result<(), Failure> {
    exactly_one_finding("#![allow (warnings, reason = \"see the note (below)\")]\n")
}

/// Scenario: attribute-shaped text in a string literal or a doc comment.
///
/// Invariant: neither is an offence. This is the false positive a text scan
/// cannot avoid, and a contract that reports one gets disabled.
#[test]
fn attribute_shaped_text_is_not_an_attribute() -> Result<(), Failure> {
    no_findings(include_str!(
        "fixtures/env_policy_samples/attribute_shaped_prose.rs.txt"
    ))
}

/// Scenario: the suppression wears the sanctioned form at crate scope.
///
/// Invariant: a crate-scoped `#![expect]` of a protected lint is an offence,
/// written plainly or nested inside a `cfg_attr` that carries the crate scope
/// down. Measured with Clippy on `wildside-fs`: with two `std::env::var` calls
/// and `#![expect(clippy::disallowed_methods, reason = "..")]` at the crate
/// root, Clippy reports nothing and exits 0, because one call fulfils the
/// expectation for the whole crate and no unfulfilled-expectation warning is
/// raised. The item-scoped form on one of the two functions still reports the
/// other call and exits 101.
#[rstest]
#[case::plain("#![expect(clippy::disallowed_methods, reason = \"probe\")]\n")]
#[case::nested("#![cfg_attr(all(), expect(clippy::disallowed_methods))]\n")]
fn a_crate_scoped_expect_is_an_offence(#[case] source: &str) -> Result<(), Failure> {
    exactly_one_finding(source)
}

/// Scenario: the suppression is written with raw identifiers.
///
/// Invariant: it is reported. `r#allow` is `allow` and `clippy::r#style` is
/// `clippy::style` to the compiler, so each silences the lint while reading
/// differently. Paths are unwrapped before comparison.
#[rstest]
#[case::raw_allow("#![r#allow(clippy::disallowed_methods)]\n")]
#[case::raw_group("#![allow(clippy::r#style)]\n")]
#[case::raw_both("#![r#allow(clippy::r#all)]\n")]
#[case::raw_expect("#![r#expect(clippy::r#disallowed_methods)]\n")]
fn raw_identifiers_do_not_hide_a_suppression(#[case] source: &str) -> Result<(), Failure> {
    exactly_one_finding(source)
}

/// Scenario: the suppression is emitted from a `macro_rules!` arm.
///
/// Invariant: it is reported, at one macro depth and at two. `syn` keeps an
/// arm's body an opaque token stream, so the attribute never reaches
/// `visit_attribute`, yet Clippy expands and honours it at every call site.
/// Every group is recursed into, so a macro that writes another macro is
/// reached as well.
#[rstest]
#[case::one_deep(include_str!("fixtures/env_policy_samples/one_macro_deep.rs.txt"))]
#[case::two_deep(include_str!("fixtures/env_policy_samples/two_macros_deep.rs.txt"))]
fn a_suppression_emitted_from_a_macro_is_an_offence(#[case] source: &str) -> Result<(), Failure> {
    exactly_one_finding(source)
}

/// Scenario: a macro emits an `allow` of a lint the policy does not cover.
///
/// Invariant: it is not reported. Walking macro bodies must not turn every
/// generated `#[allow(dead_code)]` into a finding, or the contract becomes
/// noise and gets switched off.
#[test]
fn a_macro_emitting_an_unrelated_allow_is_not_an_offence() -> Result<(), Failure> {
    no_findings(include_str!(
        "fixtures/env_policy_samples/benign_macro.rs.txt"
    ))
}

/// Scenario: a lint whose name merely contains a protected one.
///
/// Invariant: neither `clippy::allow_attributes` nor
/// `clippy::alloc_instead_of_core` is read as `clippy::all`, whose name each
/// contains. Paths are compared, not substrings, so a contributor can tell a
/// real finding from a false one.
#[rstest]
#[case::allow_attributes("clippy::allow_attributes")]
#[case::alloc_instead_of_core("clippy::alloc_instead_of_core")]
fn a_longer_lint_name_containing_a_protected_one_is_not_an_offence(
    #[case] lint: &str,
) -> Result<(), Failure> {
    no_findings(&format!("#[allow({lint})]\nfn documented() {{}}\n"))
}

/// Scenario: a policy suppression beneath a sanctioned enclosing `expect`.
///
/// Invariant: the inner `allow` is still reported, and it is the policy lint
/// that is named. An `#[expect(clippy::allow_attributes, ..)]` on an enclosing
/// item stops Clippy objecting to the `#[allow]` beneath it, which is how a
/// suppression of the policy lint could sit in plain sight. The enclosing
/// `expect` is not itself an offence; the `allow` under it is.
#[test]
fn an_allow_beneath_a_sanctioned_expect_is_still_an_offence() -> Result<(), Failure> {
    let found = suppressed_lints(include_str!(
        "fixtures/env_policy_samples/shielded_allow.rs.txt"
    ))?;
    ensure_that(
        found.len() == 1
            && found.first().map(|(lint, _)| lint.as_str()) == Some("clippy::disallowed_methods"),
        format!("the shielded allow must still be found, got {found:?}"),
    )
}
