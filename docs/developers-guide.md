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
- `nixie-cli` is installed from PyPI with `uv tool install nixie-cli`.
- `merman-cli` is installed with
  `cargo binstall --locked --no-confirm merman-cli`.

Local validation can use the same Make target:

```sh
make nixie
```

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
