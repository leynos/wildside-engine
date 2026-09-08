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

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use proc_macro2::{Delimiter, TokenStream, TokenTree, token_stream};
use std::iter::Peekable;
use std::process::Command;
use syn::{
    AttrStyle, Attribute, Macro, Meta, MetaList, Path, Token, ext::IdentExt,
    punctuated::Punctuated, visit::Visit,
};

/// Error type carried by the helpers and tests.
type Failure = Box<dyn std::error::Error>;

/// Lints whose suppression disarms the environment-access policy.
///
/// Naming the lint alone is not enough. Clippy places `disallowed_methods` in
/// the `style` group, so `clippy::style` and the wider `clippy::all` each
/// switch it off, and `warnings` takes down everything. Each was confirmed
/// against Clippy before being listed; extend this if the lint's group ever
/// changes.
///
/// The hygiene lints `clippy::allow_attributes` and
/// `clippy::allow_attributes_without_reason` are deliberately absent. They stop
/// an item-scoped `#[allow]` lowering the deny, which mattered while nothing
/// read the sources. This scan reports an `allow` of the policy lint wherever
/// it sits, so suppressing the hygiene lints buys an author nothing, and
/// listing them would reject reasoned suppressions of unrelated lints that have
/// no bearing on the policy.
const PROTECTED_LINTS: [&str; 4] = [
    "clippy::disallowed_methods",
    "clippy::style",
    "clippy::all",
    "warnings",
];

/// Sources permitted to allow a protected lint.
///
/// Only the compile-time test's fixture, whose whole purpose is to be rejected
/// by Clippy, and which is its own workspace root so no workspace build
/// compiles it.
const EXEMPT_SOURCES: [&str; 1] = ["tests/fixtures/env_policy_probe/src/lib.rs"];

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
    let root = env!("CARGO_MANIFEST_DIR");
    Dir::open_ambient_dir(root, ambient_authority())
        .map_err(|err| -> Failure { format!("open workspace root {root}: {err}").into() })
}

/// Return the tracked Rust sources this contract scans, exemptions removed.
///
/// Tracked files are the right set: an untracked file is not part of the
/// sources, and a scan of the working tree would flag scratch files.
fn scannable_sources() -> Result<Vec<Utf8PathBuf>, Failure> {
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

/// Collect every attribute in a parsed file, wherever it sits.
///
/// A visitor is used rather than a hand-rolled walk so that attributes on
/// nested items, on function-local items and on expressions are all reached.
///
/// Macro bodies are collected separately. A `macro_rules!` arm is an opaque
/// token stream to `syn`, so an `allow` written inside one never reaches
/// `visit_attribute`, yet it is expanded and honoured by Clippy at every call
/// site. The tokens are walked afterwards.
#[derive(Default)]
struct AttributeCollector {
    /// Every attribute the visitor has seen.
    attributes: Vec<Attribute>,
    /// The body of every macro definition and invocation seen.
    macro_bodies: Vec<TokenStream>,
}

impl<'ast> Visit<'ast> for AttributeCollector {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        self.attributes.push(attribute.clone());
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        self.macro_bodies.push(mac.tokens.clone());
        syn::visit::visit_macro(self, mac);
    }
}

/// Render a lint path in its plain form, for comparison.
///
/// Raw identifiers are unwrapped, because `r#allow` is `allow` and
/// `clippy::r#style` is `clippy::style` as far as the compiler is concerned.
/// Comparing the written form would let either spelling through.
fn render_path(path: &Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.unraw().to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// Parse a meta-list's comma-separated arguments.
///
/// An argument list that does not parse yields nothing, which is the safe
/// reading: this contract reports suppressions it can see, and an unparsable
/// attribute is not one Clippy would honour either.
fn nested_metas(list: &MetaList) -> Vec<Meta> {
    list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        .map(|nested| nested.into_iter().collect())
        .unwrap_or_default()
}

/// Return the lint names an `allow` meta-list suppresses.
///
/// Key-value arguments such as `reason = "..."` are not lint names and are
/// skipped.
fn allowed_lints(list: &MetaList) -> Vec<String> {
    nested_metas(list)
        .iter()
        .filter_map(|meta| match meta {
            Meta::Path(path) => Some(render_path(path)),
            Meta::List(_) | Meta::NameValue(_) => None,
        })
        .collect()
}

/// Return the lint names nested inside a `cfg_attr`.
///
/// The leading element is the condition and is skipped; a nested `cfg_attr` is
/// followed in turn. The condition is not evaluated: a suppression that applies
/// under some configuration is still a suppression, and deciding which
/// configurations are reachable is not this contract's job.
///
/// `inner` is the style of the outermost attribute and is carried down
/// unchanged, because that is what decides the scope the nested suppression
/// finally takes.
fn suppressed_by_cfg_attr(list: &MetaList, inner: bool) -> Vec<String> {
    nested_metas(list)
        .iter()
        .skip(1)
        .filter(|meta| matches!(meta, Meta::List(_)))
        .flat_map(|meta| suppressed_by_meta(meta, inner))
        .collect()
}

/// Return the lint names one `Meta` suppresses, following `cfg_attr`.
///
/// `expect` is judged by scope, `allow` is not. An item-scoped `#[expect]` is
/// the sanctioned form at a composition root: it covers one item and still
/// reports the lint everywhere else, and it warns once that item grows a seam.
/// A crate-scoped `#![expect]` behaves quite differently. One matching call
/// anywhere in the crate fulfils it, so every other call is silenced and no
/// unfulfilled-expectation warning is raised. That is a suppression wearing the
/// sanctioned form's clothes.
fn suppressed_by_meta(meta: &Meta, inner: bool) -> Vec<String> {
    let Ok(list) = meta.require_list() else {
        return Vec::new();
    };
    match render_path(meta.path()).as_str() {
        "allow" => allowed_lints(list),
        "expect" if inner => allowed_lints(list),
        "cfg_attr" => suppressed_by_cfg_attr(list, inner),
        _ => Vec::new(),
    }
}

/// Return the lint names one attribute suppresses, following `cfg_attr`.
fn suppressed_by(attribute: &Attribute) -> Vec<String> {
    suppressed_by_meta(
        &attribute.meta,
        matches!(attribute.style, AttrStyle::Inner(_)),
    )
}

/// Read an attribute when the token stream sits just after its `#`.
///
/// Returns the parsed `Meta` and the attribute as written, or `None` when the
/// `#` began something that is not an attribute.
fn attribute_at(trees: &mut Peekable<token_stream::IntoIter>) -> Option<(Meta, String, bool)> {
    let inner_style =
        matches!(trees.peek(), Some(TokenTree::Punct(punct)) if punct.as_char() == '!');
    if inner_style {
        trees.next();
    }
    let TokenTree::Group(group) = trees.peek()? else {
        return None;
    };
    if group.delimiter() != Delimiter::Bracket {
        return None;
    }
    let body = group.stream();
    let meta = syn::parse2::<Meta>(body.clone()).ok()?;
    let bang = if inner_style { "!" } else { "" };
    Some((meta, format!("#{bang}[{body}]"), inner_style))
}

/// Return the lints suppressed by an attribute beginning at the current `#`.
///
/// Empty when the `#` began something that is not an attribute, or when the
/// attribute suppresses nothing.
fn suppression_at(trees: &mut Peekable<token_stream::IntoIter>) -> Vec<(String, String)> {
    let Some((meta, rendered, inner)) = attribute_at(trees) else {
        return Vec::new();
    };
    suppressed_by_meta(&meta, inner)
        .into_iter()
        .map(|lint| (lint, rendered.clone()))
        .collect()
}

/// Return every lint suppressed inside a macro's token stream.
///
/// Every group is recursed into, so a suppression nested in a macro that
/// defines another macro is reached at any depth. The same judgement is applied
/// as to a parsed attribute, because by the time Clippy sees the expansion it
/// is one.
fn suppressed_in_tokens(tokens: TokenStream) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut trees = tokens.into_iter().peekable();
    while let Some(tree) = trees.next() {
        match tree {
            TokenTree::Punct(punct) if punct.as_char() == '#' => {
                found.extend(suppression_at(&mut trees));
            }
            TokenTree::Group(group) => found.extend(suppressed_in_tokens(group.stream())),
            TokenTree::Punct(_) | TokenTree::Ident(_) | TokenTree::Literal(_) => {}
        }
    }
    found
}

/// Render an attribute roughly as written, for a failure message.
fn render_attribute(attribute: &Attribute) -> String {
    let bang = match attribute.style {
        AttrStyle::Inner(_) => "!",
        AttrStyle::Outer => "",
    };
    let path = render_path(attribute.path());
    attribute.meta.require_list().map_or_else(
        |_| format!("#{bang}[{path}]"),
        |list| format!("#{bang}[{path}({})]", list.tokens),
    )
}

/// Return every protected lint suppressed in one source, with its attribute.
fn suppressed_lints(contents: &str) -> Result<Vec<(String, String)>, Failure> {
    let parsed =
        syn::parse_file(contents).map_err(|err| -> Failure { format!("parse: {err}").into() })?;
    let mut collector = AttributeCollector::default();
    collector.visit_file(&parsed);
    let mut found = Vec::new();
    for attribute in &collector.attributes {
        for lint in suppressed_by(attribute) {
            found.push((lint, render_attribute(attribute)));
        }
    }
    for body in collector.macro_bodies {
        found.extend(suppressed_in_tokens(body));
    }
    found.retain(|(lint, _)| PROTECTED_LINTS.contains(&lint.as_str()));
    Ok(found)
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
#[test]
fn the_scan_reaches_every_package_root() -> Result<(), Failure> {
    let sources = scannable_sources()?;
    for package in [
        "src/lib.rs",
        "wildside-cli/src/main.rs",
        "wildside-core/src/lib.rs",
        "wildside-data/src/lib.rs",
        "wildside-fs/src/lib.rs",
        "wildside-scorer/src/lib.rs",
        "wildside-solver-ortools/src/lib.rs",
        "wildside-solver-vrp/src/lib.rs",
    ] {
        ensure_that(
            sources.iter().any(|path| path.as_str() == package),
            format!("the scan should reach {package}"),
        )?;
    }
    Ok(())
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

/// Scenario: an `expect` at a sanctioned composition root.
///
/// Invariant: the scan leaves it alone. Rejecting `expect` would push
/// contributors towards `allow`, the attribute that never warns.
#[test]
fn a_sanctioned_expect_is_not_an_offence() -> Result<(), Failure> {
    let sanctioned = "#[expect(clippy::disallowed_methods, reason = \"composition root\")]\n\
         fn read() -> Option<String> { std::env::var(\"HOME\").ok() }\n";
    let found = suppressed_lints(sanctioned)?;
    ensure_that(
        found.is_empty(),
        format!("expect must be left alone, got {found:?}"),
    )
}

/// Scenario: the suppression is nested inside a `cfg_attr`.
///
/// Invariant: it is reported. Clippy honours the nested `allow` and
/// `clippy::allow_attributes` does not report it, so a scan looking for a line
/// beginning `#![allow(` would miss it entirely.
#[test]
fn a_suppression_nested_in_cfg_attr_is_an_offence() -> Result<(), Failure> {
    let nested = "#![cfg_attr(all(), allow(clippy::disallowed_methods, reason = \"x\"))]\n";
    let found = suppressed_lints(nested)?;
    ensure_that(
        found.len() == 1,
        format!("a nested suppression must be found, got {found:?}"),
    )
}

/// Scenario: the suppression names a group rather than the lint.
///
/// Invariant: it is reported. `disallowed_methods` sits in `style`, so
/// `clippy::style` switches the policy off while never naming it.
#[test]
fn a_suppression_of_a_lints_group_is_an_offence() -> Result<(), Failure> {
    for group in ["clippy::style", "clippy::all", "warnings"] {
        let source = format!("#![allow({group})]\n");
        let found = suppressed_lints(&source)?;
        ensure_that(
            found.len() == 1,
            format!("allowing {group} must be found, got {found:?}"),
        )?;
    }
    Ok(())
}

/// Scenario: awkward spacing, and a parenthesis inside the reason string.
///
/// Invariant: both are handled. A scan matching a fixed opener would miss the
/// spaced form, and one counting raw brackets would end the attribute early at
/// the parenthesis inside the string.
#[test]
fn spacing_and_a_parenthesis_in_the_reason_do_not_hide_a_suppression() -> Result<(), Failure> {
    let awkward = "#![allow (warnings, reason = \"see the note (below)\")]\n";
    let found = suppressed_lints(awkward)?;
    ensure_that(
        found.len() == 1,
        format!("the spaced form must be found, got {found:?}"),
    )
}

/// Scenario: attribute-shaped text in a string literal or a doc comment.
///
/// Invariant: neither is an offence. This is the false positive a text scan
/// cannot avoid, and a contract that reports one gets disabled.
#[test]
fn attribute_shaped_text_is_not_an_attribute() -> Result<(), Failure> {
    let prose = "//! Never write #![allow(clippy::disallowed_methods)] in a crate root.\n\
         const EXAMPLE: &str = \"#[allow(warnings)]\";\n";
    let found = suppressed_lints(prose)?;
    ensure_that(
        found.is_empty(),
        format!("prose is not an attribute, got {found:?}"),
    )
}

/// Scenario: the suppression wears the sanctioned form at crate scope.
///
/// Invariant: a crate-scoped `#![expect]` of a protected lint is an offence.
/// Measured with Clippy on `wildside-fs`: with two `std::env::var` calls and
/// `#![expect(clippy::disallowed_methods, reason = "..")]` at the crate root,
/// Clippy reports nothing and exits 0, because one call fulfils the
/// expectation for the whole crate and no unfulfilled-expectation warning is
/// raised. The item-scoped form on one of the two functions still reports the
/// other call and exits 101.
#[test]
fn a_crate_scoped_expect_is_an_offence() -> Result<(), Failure> {
    let crate_scoped = "#![expect(clippy::disallowed_methods, reason = \"probe\")]\n";
    let found = suppressed_lints(crate_scoped)?;
    ensure_that(
        found.len() == 1,
        format!("a crate-scoped expect must be found, got {found:?}"),
    )?;
    let nested = "#![cfg_attr(all(), expect(clippy::disallowed_methods))]\n";
    let carried = suppressed_lints(nested)?;
    ensure_that(
        carried.len() == 1,
        format!("cfg_attr must carry the outer scope, got {carried:?}"),
    )
}

/// Scenario: an item-scoped `expect` at a composition root.
///
/// Invariant: it is not an offence, whether written plainly or nested inside a
/// `cfg_attr`. It covers one item, still reports the lint everywhere else, and
/// warns once that item grows a seam, which is what makes it the sanctioned
/// form. Rejecting it would push contributors towards `allow`, which never
/// warns.
#[test]
fn an_item_scoped_expect_remains_sanctioned() -> Result<(), Failure> {
    let plain = "#[expect(clippy::disallowed_methods, reason = \"composition root\")]\n\
         fn read() -> Option<String> { std::env::var(\"HOME\").ok() }\n";
    let found = suppressed_lints(plain)?;
    ensure_that(
        found.is_empty(),
        format!("an item-scoped expect is sanctioned, got {found:?}"),
    )?;
    let conditional = "#[cfg_attr(unix, expect(clippy::disallowed_methods, reason = \"root\"))]\n\
         fn read() -> Option<String> { std::env::var(\"HOME\").ok() }\n";
    let nested = suppressed_lints(conditional)?;
    ensure_that(
        nested.is_empty(),
        format!("cfg_attr carries the outer scope here too, got {nested:?}"),
    )
}

/// Scenario: the suppression is written with raw identifiers.
///
/// Invariant: it is reported. `r#allow` is `allow` and `clippy::r#style` is
/// `clippy::style` to the compiler, so each silences the lint while reading
/// differently. Paths are unwrapped before comparison.
#[test]
fn raw_identifiers_do_not_hide_a_suppression() -> Result<(), Failure> {
    for source in [
        "#![r#allow(clippy::disallowed_methods)]\n",
        "#![allow(clippy::r#style)]\n",
        "#![r#allow(clippy::r#all)]\n",
        "#![r#expect(clippy::r#disallowed_methods)]\n",
    ] {
        let found = suppressed_lints(source)?;
        ensure_that(
            found.len() == 1,
            format!("a raw-identifier suppression must be found in {source:?}, got {found:?}"),
        )?;
    }
    Ok(())
}

/// Scenario: the suppression is emitted from a `macro_rules!` arm.
///
/// Invariant: it is reported, at one macro depth and at two. `syn` keeps an
/// arm's body an opaque token stream, so the attribute never reaches
/// `visit_attribute`, yet Clippy expands and honours it at every call site.
/// Every group is recursed into, so a macro that writes another macro is
/// reached as well.
#[test]
fn a_suppression_emitted_from_a_macro_is_an_offence() -> Result<(), Failure> {
    let one_deep = concat!(
        "macro_rules! disarm {\n",
        "    () => {\n",
        "        #![allow(clippy::disallowed_methods)]\n",
        "    };\n",
        "}\n",
    );
    let shallow = suppressed_lints(one_deep)?;
    ensure_that(
        shallow.len() == 1,
        format!("a suppression one macro deep must be found, got {shallow:?}"),
    )?;
    let two_deep = concat!(
        "macro_rules! outer {\n",
        "    () => {\n",
        "        macro_rules! inner {\n",
        "            () => {\n",
        "                #![allow(clippy::style)]\n",
        "            };\n",
        "        }\n",
        "    };\n",
        "}\n",
    );
    let nested = suppressed_lints(two_deep)?;
    ensure_that(
        nested.len() == 1,
        format!("a suppression two macros deep must be found, got {nested:?}"),
    )
}

/// Scenario: a macro emits an `allow` of a lint the policy does not cover.
///
/// Invariant: it is not reported. Walking macro bodies must not turn every
/// generated `#[allow(dead_code)]` into a finding, or the contract becomes
/// noise and gets switched off.
#[test]
fn a_macro_emitting_an_unrelated_allow_is_not_an_offence() -> Result<(), Failure> {
    let benign = concat!(
        "macro_rules! define {\n",
        "    ($name:ident) => {\n",
        "        #[allow(dead_code, reason = \"generated\")]\n",
        "        fn $name() {}\n",
        "    };\n",
        "}\n",
    );
    let found = suppressed_lints(benign)?;
    ensure_that(
        found.is_empty(),
        format!("an unrelated generated allow is not a finding, got {found:?}"),
    )
}

/// Scenario: a lint whose name merely contains a protected one.
///
/// Invariant: neither `clippy::allow_attributes` nor
/// `clippy::alloc_instead_of_core` is read as `clippy::all`, whose name each
/// contains. Paths are compared, not substrings, so a contributor can tell a
/// real finding from a false one.
#[test]
fn a_longer_lint_name_containing_a_protected_one_is_not_an_offence() -> Result<(), Failure> {
    for lint in ["clippy::allow_attributes", "clippy::alloc_instead_of_core"] {
        let source = format!("#[allow({lint})]\nfn documented() {{}}\n");
        let found = suppressed_lints(&source)?;
        ensure_that(
            found.is_empty(),
            format!("{lint} is not a protected lint, got {found:?}"),
        )?;
    }
    Ok(())
}

/// Scenario: a policy suppression beneath a sanctioned enclosing `expect`.
///
/// Invariant: the inner `allow` is still reported. An
/// `#[expect(clippy::allow_attributes, ..)]` on an enclosing item stops Clippy
/// objecting to the `#[allow]` beneath it, which is how a suppression of the
/// policy lint could sit in plain sight. The enclosing `expect` is not itself
/// an offence; the `allow` under it is.
#[test]
fn an_allow_beneath_a_sanctioned_expect_is_still_an_offence() -> Result<(), Failure> {
    let shielded = concat!(
        "#[expect(clippy::allow_attributes, reason = \"fields vary by binary\")]\n",
        "struct Config {\n",
        "    #[allow(clippy::disallowed_methods)]\n",
        "    home: String,\n",
        "}\n",
    );
    let found = suppressed_lints(shielded)?;
    ensure_that(
        found.len() == 1
            && found.first().map(|(lint, _)| lint.as_str()) == Some("clippy::disallowed_methods"),
        format!("the shielded allow must still be found, got {found:?}"),
    )
}
