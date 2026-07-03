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

[^1]: <https://github.com/leynos/shared-actions/tree/main/.github/actions/setup-rust>
