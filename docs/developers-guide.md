# Developers guide

This guide records the local tooling expected by the Continuous Integration
(CI) workflow.

## Rust toolchain

The repository pins Rust in [`rust-toolchain.toml`](../rust-toolchain.toml).
Developers should let `rustup` install that toolchain automatically when
entering the workspace or running Cargo commands.

The required components are:

- `rustfmt`, for formatting checks.
- `clippy`, for lint checks with warnings denied.
- `llvm-tools-preview`, for tools that need LLVM coverage support.
- `rust-analyzer`, for language-server support in editors and agent tooling.

The CI workflow uses the shared `setup-rust` action[^1] without an explicit
`toolchain` input. The action reads `rust-toolchain.toml`, so local and CI
builds use the same nightly and component list.

## Diagram tooling

Markdown diagram validation uses Mermaid CLI, `nixie-cli`, and `merman-cli`.
The CI workflow installs them before running `make nixie`:

- Mermaid CLI is installed with Bun and verified with `mmdc --version`.
- `nixie-cli` 1.1.0 is installed from PyPI for Python 3.14.
- `merman-cli` 0.7.0 is installed with Rust 1.95.0.

Local validation can use the same Make target:

```sh
make nixie
```

## Spelling policy

Run the spelling gate with:

```sh
make spelling
```

The gate enforces en-GB-oxendict spelling across tracked text, including hidden
files, and also enforces the shared phrase corrections Typos cannot express.
`make markdownlint` depends on the same gate.

The tracked `typos.toml` is regenerated on every run from the live shared
dictionary and the repository-specific `typos.local.toml` overlay. Never edit
generated entries by hand; add only narrow repository terminology to the
overlay. Because the dictionary is live, `typos.toml` must never be drift
checked in continuous integration.

The shared `typos-config-builder` CLI refreshes the estate dictionary into the
untracked `.typos-oxendict-base.toml` cache only when the authoritative copy is
newer; `.typos-oxendict-base.json` records refresh metadata. A valid cache
remains usable when the network is unavailable. Keep repository exceptions
narrow: preserve external APIs, formal names, wire values and immutable
fixtures without adding ordinary bare-word exceptions.

Continuous integration installs Nixie 1.1.0 and Merman CLI 0.7.0 before
validating the repository's Mermaid diagrams with `make nixie`.

## Environment access policy

No code in this workspace reads or mutates the process environment ambiently.
`std::env::var`, `var_os`, `vars`, `vars_os`, `set_var` and `remove_var` are
listed under `disallowed-methods` in [`clippy.toml`](../clippy.toml), and
`clippy::disallowed_methods` is denied, so `make lint` rejects a direct call in
any package, target or feature.

The rule exists for test throughput as much as for design. A test that mutates
the parent process environment forces the whole suite that touches that
variable to run serially, which wastes cores in CI and hides ordering bugs.
Injected seams keep tests hermetic and parallel.

### Choosing a seam

Pick the lightest shape that fits the boundary, judged by how many call sites
it has and whether it is expected to grow.

- **Explicit value.** One variable read by one caller: pass the resolved value
  as an argument. Nothing else is warranted.
- **Narrow reader closure.** A small reusable boundary: the module owns a
  private function taking an `FnOnce(&str) -> Result<String, VarError>` (or the
  `OsString`-typed equivalent) instead of reading the process itself.
- **Shared environment trait.** Only when several variables feed one boundary,
  or many tests must mock it. Production supplies the real reader and tests
  supply a stub. A trait for a single-variable, single-caller site recreates
  the ambient coupling one layer down, so reviewers should reject it.

### Composition roots

A genuine executable composition root, such as a binary's `main`, may read the
environment directly. It carries an item-scoped attribute naming the reason:

```rust
#[expect(clippy::disallowed_methods, reason = "composition root for WILDSIDE_DB")]
fn database_url() -> Option<String> {
    std::env::var("WILDSIDE_DB").ok()
}
```

Use `expect` rather than `allow`. When the site later gains a seam, the
expectation goes unfulfilled and warns, so the exception removes itself instead
of rotting.

### Tests and child processes

Tests never mutate the parent process environment. A test that needs a
controlled environment for a spawned binary builds the child's environment
explicitly with `Command::env_clear`, `Command::env` and `Command::env_remove`,
forwarding only what the scenario needs. An in-process test supplies a stub
through the seam instead. Because nothing mutates shared process state, no test
needs a serialization group for environment reasons; a nextest group is
justified only by a documented structural constraint such as a fixed port or a
shared on-disk fixture.

### Contract

Two test files hold the policy in place, and every assertion in both was proved
by mutation. Each file's module documentation records the mutations and the
test each one broke.

[`tests/clippy_env_policy_ui_tests.rs`](../tests/clippy_env_policy_ui_tests.rs)
proves the lint fires. It runs Clippy over the fixture crate in
`tests/fixtures/env_policy_probe` and reads the diagnostics: all six methods
are rejected, each carrying its own guidance string; an `#[allow]` is itself
rejected; and a reasoned `#[expect]` compiles. The fixture is excluded from the
workspace so its deliberately offending probes never reach
`cargo clippy --workspace`, and the test points Clippy at this repository's
`clippy.toml`, so the reason strings it asserts are the ones a contributor sees.

[`tests/clippy_env_policy_tests.rs`](../tests/clippy_env_policy_tests.rs)
guards the configuration that makes the lint fire. It fails if any of the six
entries leaves `clippy.toml`, if the deny is downgraded, if a package joins or
leaves the workspace, if a package stops enforcing the rule, if the Make lint
target stops covering every target and feature, or if the CI lint step
overrides the Clippy flags. It embeds each file with `include_str!`, so moving
or deleting one is a compile failure rather than a runtime error. The package
list comes from `cargo metadata` rather than `[workspace].members`, because
Cargo also promotes an in-tree path dependency to a member without an entry in
that array, and is then compared with the eight names it expects.

A third file, [`clippy_env_policy_source_tests.rs`][source-scan], parses the
sources themselves and rejects any `allow` of `clippy::disallowed_methods`, of
its group `clippy::style`, or of `clippy::all` or `warnings`.
`clippy::allow_attributes` does not fire on inner attributes, so a crate-level
`#![allow(clippy::disallowed_methods)]` would otherwise switch the policy off
for a whole crate with every other contract and the lint gate green. Naming a
group is enough to do the same, because Clippy places `disallowed_methods` in
`style`.

[source-scan]: ../tests/clippy_env_policy_source_tests.rs

It scans every tracked Rust source. One file is exempt,
`tests/fixtures/env_policy_probe/src/lib.rs`, whose whole purpose is to be
rejected by Clippy; the list is closed, and `only_the_ui_fixture_is_exempt`
fails if it gains a second entry. The contract is split across
`tests/env_policy_scan/support.rs` for the source set,
`tests/env_policy_scan/scan.rs` for the attribute and token walk,
`tests/env_policy_scan/properties.rs` for the judgement stated as properties
over generated lint names, forms and reason strings, and the test root for the
example-based judgements. The sample sources are held as `.rs.txt` files under
`tests/fixtures/env_policy_samples`, so the scan does not read its own fixtures
as offences.

The example-based tests and the property tests answer different questions. Each
example pins a route that measurement found, such as a `cfg_attr` or a
`macro_rules!` arm; the properties say that what the scan reports is decided by
the lint named and by the scope the attribute takes, and by nothing else, over
generated lint names, forms and reason strings. Reason strings are generated
with spaces, commas and parentheses, because a parenthesis inside a reason
defeated the text scan this contract replaced.

The hygiene lints `clippy::allow_attributes` and
`clippy::allow_attributes_without_reason` are not protected. They stop an
item-scoped `#[allow]` lowering the deny, which mattered while nothing read the
sources; now that an `allow` of the policy lint is reported wherever it sits,
suppressing them buys nothing, and protecting them would reject reasoned
suppressions of unrelated lints. An `#[allow(clippy::disallowed_methods)]`
beneath an enclosing `#[expect(clippy::allow_attributes, ..)]` is still
reported.

An attribute whose body is a macro metavariable is refused rather than
resolved. A `macro_rules!` arm writing `#[$attr]`, invoked as
`forward!(allow(clippy::disallowed_methods))`, silences the policy lint with no
diagnostic of any kind, and neither half is visible to a scan: `#[$attr]` does
not parse as an attribute body, and the invocation carries no `#`. Only the
shapes that could bear on the policy are refused, those whose body begins with
`$`, or with `allow`, `expect` or `cfg_attr`; `#[doc = $doc]` and
`#[derive($trait)]` are left alone. Write the lint into the attribute rather
than passing it in.

The sources are parsed with `syn` rather than searched. A text scan cannot
follow `#[cfg_attr(<any condition>, allow(...))]`, which Clippy honours, cannot
tell an attribute from attribute-shaped text in a string literal or a doc
comment, and is defeated by a space before the parenthesis or a parenthesis
inside a `reason`. Lint paths are compared rather than matched as substrings, so
`clippy::alloc_instead_of_core` is not read as `clippy::all`, and raw
identifiers are unwrapped first, so `clippy::r#style` cannot slip past as a
different name.

`expect` is judged by scope rather than waved through. An item-scoped
`#[expect(..., reason = "...")]` at a composition root is the sanctioned form:
it covers one item, still reports the lint everywhere else, and warns once that
site grows a seam. A crate-scoped `#![expect(...)]` is a different thing
wearing the same clothes. One matching call fulfils it for the whole crate, so
every other call is silenced and no unfulfilled-expectation warning is raised.
Write the item-scoped form.

Four packages do not yet inherit the workspace lint table, so they deny
`disallowed_methods` in their own manifests, together with `allow_attributes`
and `allow_attributes_without_reason`. Those two matter: an item-scoped
`#[allow(clippy::disallowed_methods)]` lowers a deny, so without them the
policy would be bypassable in exactly the packages that carry it locally. Issue
124 folds all three into `[lints] workspace = true`.

## Workflow pins and Dependabot

Dependabot owns the upgrade of GitHub Actions and reusable workflows, including
calls into `leynos/shared-actions`. Contract tests that assert a caller's exact
commit SHA create a lockstep dependency: every time Dependabot opens a bump PR,
the test fails until a human edits the pinned constant to match. That defeats
the purpose of automated dependency updates and turns a routine bump into a
manual chore.

Contract tests may still verify the *shape* of a reusable-workflow caller. They
must not verify the specific SHA value.

- Do assert the workflow references the correct reusable workflow path.
- Do assert the ref is pinned to a full 40-character commit SHA, not a
  mutable branch such as `main` or `rolling`.
- Do assert the expected `on:` triggers, least-privilege `permissions:`, and
  the inputs the caller relies on.
- Do not hard-code the current SHA value as an expected string. Match it with
  a pattern instead.
- Do not fail a test purely because Dependabot bumped the pinned SHA.

```python
import re

SHA_RE = re.compile(r"^[0-9a-f]{40}$")


def test_uses_pinned_full_sha(caller_step):
    ref = caller_step["uses"].split("@")[-1]
    assert SHA_RE.match(ref), f"expected a 40-hex commit SHA, got {ref!r}"
```

If a workflow's behaviour genuinely depends on a feature only present from a
particular commit onwards, express that as a comment or a changelog note, not
as a test assertion on the SHA string.

## Mutation-testing workflow contract tests

This repository runs scheduled, informational mutation testing through a thin
caller workflow, [`.github/workflows/mutation-testing.yml`](../.github/workflows/mutation-testing.yml),
which delegates to the shared reusable workflow
`leynos/shared-actions/.github/workflows/mutation-cargo.yml`. The heavy lifting
— running `cargo-mutants`, sharding, and summarizing survivors — lives in
`shared-actions`; this repository carries only declarative configuration. The
run is **informational only**: it never gates a pull request. Survivors are
reported through the job summary and downloadable artefacts so they can be
triaged into tests, not enforced as a blocking check.

The workflow runs in two modes. A **daily schedule** fires a change-scoped run
that mutates only the source files touched within the detection window, so
quiet days are cheap no-ops. A **manual dispatch** (the Actions "Run workflow"
control) mutates the whole workspace, fanned out across shards; select a
branch in that control to exercise a feature branch.

The caller passes a small set of configuration inputs, each carrying intent:

- `paths` — the change-detection globs that decide whether a scheduled run has
  anything to mutate. Because the workspace members (`wildside-cli`,
  `wildside-core`, `wildside-data`, `wildside-fs`, `wildside-scorer`,
  `wildside-solver-ortools`, `wildside-solver-vrp`) live beside the root crate
  rather than under a shared `crates/` directory, each member directory is
  listed explicitly alongside `src/`.
- `exclude-globs` — feature-gated test-support and benchmark scaffolding
  (`test_support.rs`, `bench_support.rs`) whose surviving mutants are noise
  rather than genuine test gaps, kept out of the survivors table.
- `extra-args` — arguments forwarded to `cargo-mutants` (here
  `--features test-support --test-workspace=true`) so the mutation run
  matches the CI baseline. `--features test-support` matches `make test`,
  which compiles the shared test-double scaffolding; `--test-workspace=true`
  runs the whole workspace's test suite against each mutant rather than only
  the mutated package's own tests, so crates that are covered indirectly by a
  dependent crate's tests do not report false survivors.

The `uses:` reference pins the shared workflow to a full 40-character commit
SHA rather than a branch or tag, so a force-push upstream cannot silently
change what runs here. The contract test asserts only that the pin is a full
commit SHA, not a particular value, so Dependabot bumps it automatically
without any accompanying test edit. The `with:` block is checked differently:
it is compared for exact equality against a hard-coded expected mapping, so
adding, removing, or editing any input requires updating the test alongside
the workflow in the same change.

Because the caller is configuration rather than code, a contract test pins
the shape it must uphold, failing the pull request when the caller drifts —
repointing the pin at a branch, widening the token scope, or dropping a
configuration input — rather than letting the breakage surface only in a
scheduled run. Run it locally with `make test-workflow-contracts`. The test,
[`tests/workflow_contracts/mutation_testing_test.py`](../tests/workflow_contracts/mutation_testing_test.py),
validates:

- the `uses:` reference targets `mutation-cargo.yml` pinned to a full commit
  SHA;
- job permissions are exactly `contents: read` and `id-token: write`, and the
  workflow-level default token scope is empty;
- `concurrency` serializes runs per ref (`mutation-testing-${{ github.ref }}`)
  without cancelling one in progress;
- the triggers keep the daily 04:20 UTC schedule and a plain
  `workflow_dispatch` with no legacy branch input; and
- the `with:` block carries exactly the expected `paths`, `exclude-globs`,
  and `extra-args` values described above.

[^1]: <https://github.com/leynos/shared-actions/tree/main/.github/actions/setup-rust>
