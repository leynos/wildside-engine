//! Attribute and token scanning for the environment-policy source contract.
//!
//! The sources are parsed rather than searched, and macro bodies are walked
//! alongside the parsed attributes. The test root states why each of those
//! choices is load-bearing and records the mutations that prove them.

use crate::support::Failure;
use proc_macro2::{Delimiter, TokenStream, TokenTree, token_stream};
use std::iter::Peekable;
use syn::{
    AttrStyle, Attribute, Macro, Meta, MetaList, Path, Token, ext::IdentExt,
    punctuated::Punctuated, visit::Visit,
};

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
pub const PROTECTED_LINTS: [&str; 4] = [
    "clippy::disallowed_methods",
    "clippy::style",
    "clippy::all",
    "warnings",
];

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
pub fn suppressed_lints(contents: &str) -> Result<Vec<(String, String)>, Failure> {
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
