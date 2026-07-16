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

[^1]: <https://github.com/leynos/shared-actions/tree/main/.github/actions/setup-rust>
