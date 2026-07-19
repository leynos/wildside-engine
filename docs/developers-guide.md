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

The `make spelling` gate enforces en-GB-oxendict spelling across tracked text.
It runs Typos 1.48.0 and a phrase checker that rejects the hyphenated form in
favour of `handwritten`. `make markdownlint` depends on the same spelling gate.

The tracked `typos.toml` is generated from the shared Oxford dictionary and the
repository-specific `typos.local.toml` overlay. The generator is the focused
`typos-config-builder` command pinned to commit
`b604f198797fdd36a567dd0f8f07b13f9539b241`. It refreshes the untracked
`.typos-oxendict-base.toml` cache only when the authority is newer than the
local copy; `.typos-oxendict-base.json` records refresh metadata.

Use `make spelling-config-write` after changing `typos.local.toml`, and use
`make spelling-config` to check deterministic output. Never edit `typos.toml`
directly. Keep repository exceptions narrow: preserve external APIs, formal
names, wire values and immutable fixtures without adding ordinary bare-word
exceptions.

The standalone phrase helper and its tests use Python 3.14 at runtime,
Pathspec 1.1.1 and a Python 3.13 Ruff compatibility target. Continuous
integration installs Nixie 1.1.0 and Merman CLI 0.7.0 before validating the
repository's Mermaid diagrams with `make nixie`.

## Workflow pins and Dependabot

Dependabot owns the upgrade of GitHub Actions and reusable workflows,
including calls into `leynos/shared-actions`. Contract tests that assert a
caller's exact commit SHA create a lockstep dependency: every time Dependabot
opens a bump PR, the test fails until a human edits the pinned constant to
match. That defeats the purpose of automated dependency updates and turns a
routine bump into a manual chore.

Contract tests may still verify the *shape* of a reusable-workflow caller.
They must not verify the specific SHA value.

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
