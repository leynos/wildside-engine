//! Property tests for the environment-policy source scan.
//!
//! The example-based contracts beside these name the routes that were found by
//! measurement: inner attributes, groups, `cfg_attr`, macro arms, raw
//! identifiers, crate-scoped `expect`. Each is one sample of a wider claim, and
//! a sample cannot say whether the scan's judgement holds for a lint name or a
//! reason string nobody thought to write down.
//!
//! These state the claim itself over generated inputs: what the scan reports is
//! decided by the lint named and by the scope the attribute takes, and by
//! nothing else. The reason string in particular is generated with spaces,
//! commas and parentheses, because a parenthesis inside a reason defeated the
//! text scan this contract replaced.
//!
//! Mutation-proved on 2026-09-14, each applied alone to `scan.rs` and run
//! through the build:
//!
//! - dropping the `"expect" if inner` arm from `suppressed_by_meta` fails
//!   `a_protected_lint_is_reported_in_every_offending_form`;
//! - scanning an empty token stream in place of each macro body, so every
//!   helper stays used and the mutant compiles, fails it too;
//! - comparing lint names with `contains` rather than by path fails
//!   `an_unprotected_lint_is_never_reported`, on the two generated names that
//!   begin with the text `clippy::all`.

use crate::scan::{PROTECTED_LINTS, suppressed_lints};
use proptest::prelude::*;

/// Lints outside the policy's protected set.
///
/// `clippy::allow_attributes` and `clippy::alloc_instead_of_core` are here on
/// purpose: each begins with the text `clippy::all`, so a scan comparing
/// substrings rather than paths would report both.
const UNPROTECTED_LINTS: [&str; 5] = [
    "dead_code",
    "unused_variables",
    "clippy::allow_attributes",
    "clippy::alloc_instead_of_core",
    "clippy::pedantic",
];

/// A way of writing a suppression the policy refuses.
///
/// Most of these reach past the item they are written on. `ItemAllow` does
/// not: it lowers the deny for one item, and is refused because the policy's
/// rule is `expect`, never `allow`, precisely so that a suppression warns once
/// its site stops needing it.
#[derive(Debug, Clone, Copy)]
enum OffendingForm {
    /// A crate-level `allow`, which `clippy::allow_attributes` cannot see.
    InnerAllow,
    /// An item-level `allow`, which lowers the deny for that item.
    ItemAllow,
    /// A crate-level `expect`, fulfilled crate-wide by one matching call.
    CrateExpect,
    /// An `allow` nested inside a `cfg_attr`, which Clippy honours.
    NestedInCfgAttr,
    /// An `allow` emitted from a `macro_rules!` arm, opaque to `syn`.
    FromMacroArm,
    /// A crate-level `allow` with a space before the parenthesis.
    SpacedInnerAllow,
    /// A crate-level `allow` spelt with raw identifiers.
    RawInnerAllow,
}

/// Spell a path with each segment as a raw identifier.
fn as_raw(lint: &str) -> String {
    lint.split("::")
        .map(|segment| format!("r#{segment}"))
        .collect::<Vec<_>>()
        .join("::")
}

impl OffendingForm {
    /// Render this form as a source file suppressing `lint`.
    fn render(self, lint: &str, reason: &str) -> String {
        match self {
            Self::InnerAllow => format!("#![allow({lint}, reason = \"{reason}\")]\n"),
            Self::ItemAllow => {
                format!("#[allow({lint}, reason = \"{reason}\")]\nfn probe() {{}}\n")
            }
            Self::CrateExpect => format!("#![expect({lint}, reason = \"{reason}\")]\n"),
            Self::NestedInCfgAttr => {
                format!("#![cfg_attr(all(), allow({lint}, reason = \"{reason}\"))]\n")
            }
            Self::FromMacroArm => format!(
                "macro_rules! disarm {{\n    () => {{\n        \
                 #![allow({lint}, reason = \"{reason}\")]\n    }};\n}}\n"
            ),
            Self::SpacedInnerAllow => format!("#![allow ({lint}, reason = \"{reason}\")]\n"),
            Self::RawInnerAllow => {
                format!("#![r#allow({}, reason = \"{reason}\")]\n", as_raw(lint))
            }
        }
    }
}

/// Every offending form, for a strategy to select from.
const OFFENDING_FORMS: [OffendingForm; 7] = [
    OffendingForm::InnerAllow,
    OffendingForm::ItemAllow,
    OffendingForm::CrateExpect,
    OffendingForm::NestedInCfgAttr,
    OffendingForm::FromMacroArm,
    OffendingForm::SpacedInnerAllow,
    OffendingForm::RawInnerAllow,
];

/// Run the scan, turning a parse failure into a test-case failure.
fn scan(source: &str) -> Result<Vec<(String, String)>, TestCaseError> {
    suppressed_lints(source)
        .map_err(|err| TestCaseError::fail(format!("{source:?} should parse as Rust: {err}")))
}

/// A lint name the policy protects.
fn protected_lint() -> impl Strategy<Value = &'static str> {
    proptest::sample::select(PROTECTED_LINTS.to_vec())
}

/// A lint name the policy does not protect.
fn unprotected_lint() -> impl Strategy<Value = &'static str> {
    proptest::sample::select(UNPROTECTED_LINTS.to_vec())
}

/// Any lint name, protected or not.
fn any_lint() -> impl Strategy<Value = &'static str> {
    prop_oneof![protected_lint(), unprotected_lint()]
}

/// A way of writing a suppression the policy refuses.
fn offending_form() -> impl Strategy<Value = OffendingForm> {
    proptest::sample::select(OFFENDING_FORMS.to_vec())
}

/// A reason string, with the punctuation that defeats a text scan.
fn reason() -> impl Strategy<Value = String> {
    "[a-z ,()]{0,24}".prop_map(String::from)
}

proptest! {
    /// Scenario: a protected lint is suppressed in any form the policy
    /// refuses.
    ///
    /// Invariant: the scan reports exactly that lint, once, whatever the form
    /// and whatever the reason string says. The example-based tests each pin
    /// one form; this says the judgement is the lint's, not the spelling's.
    #[test]
    fn a_protected_lint_is_reported_in_every_offending_form(
        lint in protected_lint(),
        form in offending_form(),
        reason in reason(),
    ) {
        let found = scan(&form.render(lint, &reason))?;
        let names: Vec<&str> = found.iter().map(|(name, _)| name.as_str()).collect();
        prop_assert_eq!(names, vec![lint], "form {:?} reason {:?}", form, reason);
    }

    /// Scenario: a lint outside the protected set is suppressed in the same
    /// forms.
    ///
    /// Invariant: nothing is reported. A contract that fires on every
    /// `#[allow(dead_code)]` becomes noise and gets switched off, and two of
    /// the generated names begin with the text `clippy::all`, so this also says
    /// paths are compared rather than substrings.
    #[test]
    fn an_unprotected_lint_is_never_reported(
        lint in unprotected_lint(),
        form in offending_form(),
        reason in reason(),
    ) {
        let found = scan(&form.render(lint, &reason))?;
        prop_assert!(found.is_empty(), "form {:?} found {:?}", form, found);
    }

    /// Scenario: the sanctioned form, an item-scoped `expect` with a reason.
    ///
    /// Invariant: it is never an offence, for any lint. Rejecting it would push
    /// contributors towards `allow`, the attribute that never warns.
    #[test]
    fn an_item_scoped_expect_is_never_an_offence(
        lint in any_lint(),
        reason in reason(),
    ) {
        let sanctioned = format!(
            "#[expect({lint}, reason = \"{reason}\")]\nfn probe() {{}}\n"
        );
        let found = scan(&sanctioned)?;
        prop_assert!(found.is_empty(), "lint {:?} found {:?}", lint, found);
    }

    /// Scenario: attribute-shaped text in a string literal and in a doc
    /// comment.
    ///
    /// Invariant: neither is ever an offence. This is the false positive a text
    /// scan cannot avoid, and a contract that reports one gets disabled.
    #[test]
    fn attribute_shaped_text_is_never_an_offence(lint in any_lint()) {
        let prose = format!(
            "//! Never write #![allow({lint})] in a crate root.\n\
             const EXAMPLE: &str = \"#[allow({lint})]\";\n"
        );
        let found = scan(&prose)?;
        prop_assert!(found.is_empty(), "lint {:?} found {:?}", lint, found);
    }
}
