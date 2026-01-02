# Phase 4 Feature Flags

This ExecPlan is a living document. The sections `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must
be kept up to date as work proceeds.

No `PLANS.md` file exists in this repository. If one is added later, this plan
must be updated to follow it.

## Purpose / Big Picture

Introduce feature flags that let consumers choose which solver and store
implementations are compiled, while keeping the default build stable and
fully-tested. Success is observable when the workspace builds with the default
features, the CLI uses the selected solver and store at runtime, and optional
features compile cleanly under `--all-features` while tests cover both enabled
and disabled paths.

## Progress

- [x] (2026-01-02 05:46Z) Review existing Cargo manifests and feature usage.
- [x] (2026-01-02 05:46Z) Decide on the root package strategy and implement
  root feature definitions.
- [x] (2026-01-02 05:46Z) Add or stub the `wildside-solver-ortools` crate so
  `--all-features` succeeds.
- [x] (2026-01-02 05:46Z) Gate the SQLite store in `wildside-core` behind a
  feature and adjust exports/tests.
- [x] (2026-01-02 05:46Z) Gate solver and store wiring in `wildside-cli` and
  update errors.
- [x] (2026-01-02 05:46Z) Add rstest unit tests and rstest-bdd behavioural
  scenarios for feature combinations.
- [x] (2026-01-02 05:46Z) Update design and testing docs and mark the roadmap
  item as done.
- [x] (2026-01-02 07:20Z) Run `make check-fmt`, `make lint`, `make test`, plus
  the `--no-default-features` CLI checks and capture logs.

## Surprises & Discoveries

- Observation: `cargo new` failed after introducing a root package because the
  root dependency tried to override `default-features` on a workspace
  dependency. Evidence: Cargo reported that `default-features = false` cannot
  override the workspace dependency definition.

## Decision Log

- Decision: Treat the workspace root as a package so it can own the canonical
  `solver-vrp`, `solver-ortools`, and `store-sqlite` features. Rationale: Cargo
  features are package-scoped, and the requirement explicitly calls for feature
  definitions in the root manifest. Date/Author: 2026-01-02 / plan author.

- Decision: Use a direct path dependency for `wildside-core` in the root
  package to disable default features without conflicting with workspace
  dependency definitions. Rationale: Workspace dependency inheritance forbids
  overriding `default-features`. Date/Author: 2026-01-02 / plan author.

- Decision: Gate the SQLite store and spatial index helpers behind the
  `store-sqlite` feature in `wildside-core` rather than introducing a no-op
  feature in `wildside-data`. Rationale: The SQLite store lives in
  `wildside-core`; `wildside-data` still requires SQLite for ETL regardless of
  store configuration. Date/Author: 2026-01-02 / plan author.

- Decision: Prefer the VRP solver when both solver features are enabled and
  ship a placeholder OR-Tools solver that returns `SolveError::InvalidRequest`.
  Rationale: Maintains current behaviour while reserving the API surface for a
  future OR-Tools integration. Date/Author: 2026-01-02 / plan author.

## Outcomes & Retrospective

Feature flags now guard solver and store implementations across the workspace,
including CLI behaviour for missing capabilities. Default builds preserve the
VRP solver and SQLite store, while `--no-default-features` builds exercise the
missing-feature paths. Tests and docs were updated accordingly, and the roadmap
entry was marked done.

## Context and Orientation

The workspace is defined in `Cargo.toml` with multiple crates: `wildside-core`,
`wildside-data`, `wildside-scorer`, `wildside-solver-vrp`, `wildside-cli`, and
`wildside-fs`. The root manifest is currently a virtual workspace, but
`src/lib.rs` exists and can become the root package if needed.

Solver wiring lives in `wildside-cli/src/solve.rs`, which currently depends on
`wildside-solver-vrp` and `wildside-core::SqlitePoiStore` unconditionally.
SQLite store logic is implemented in `wildside-core/src/store.rs` and re-
exported from `wildside-core/src/lib.rs`. The roadmap requirement calls for
feature flags named `solver-vrp`, `solver-ortools`, and `store-sqlite`, with
conditional compilation via `#[cfg(feature = "...")]`.

Testing guidance is in `docs/rust-testing-with-rstest-fixtures.md` and
`docs/rstest-bdd-users-guide.md`. Behavioural tests must use `rstest-bdd`
version 0.3.0, so the workspace dependency versions likely need updating.
Design decisions must be recorded in `docs/wildside-engine-design.md`, and the
roadmap item in `docs/roadmap.md` must be marked done after completion.

## Plan of Work

First, inspect the manifests and decide how to host features in the root. Cargo
only allows features on packages, so the most direct path is to add a
`[package]` section to the root manifest and turn `src/lib.rs` into a small
facade crate (for example `wildside-engine`) that re-exports the core types and
conditionally re-exports solver and store implementations. Define optional
dependencies in the root manifest using `dep:` syntax and declare features
`solver-vrp`, `solver-ortools`, and `store-sqlite`, with defaults matching the
existing behaviour (likely solver-vrp plus store-sqlite). Ensure the feature
names align with the roadmap and update `Cargo.lock` as required.

Next, ensure the solver-ortools feature can compile under
`cargo clippy --all-features`. If no OR-Tools solver exists yet, add a minimal
`wildside-solver-ortools` crate (library only) that implements `Solver` with a
clear placeholder behaviour (for example returning `SolveError::InvalidRequest`
with a documented rationale). Add module-level documentation and keep the
implementation trivial but lint-clean. Add the new crate to the workspace
members so `--all-features` resolves it.

Then, gate the SQLite store in `wildside-core`. Introduce a feature such as
`store-sqlite` (or `sqlite` if you want a shorter internal name) and make
`rusqlite`, `serde`, `serde_json`, and `bincode` optional dependencies that are
activated by that feature. Move SQLite-specific types (`SqlitePoiStore`,
`SqlitePoiStoreError`, spatial index helpers) into a module compiled only when
that feature is enabled, and keep `PoiStore` and any in-memory/test stores
available without it. Update `wildside-core/src/lib.rs` to conditionally
re-export the SQLite store and to add `doc(cfg(...))` hints where appropriate.
Adjust existing unit tests in `wildside-core/src/store.rs` so SQLite tests are
only compiled when the feature is enabled.

After that, update `wildside-cli` to respect the feature flags. Use
`#[cfg(feature = "solver-vrp")]` and `#[cfg(feature = "solver-ortools")]` to
select the solver implementation in `wildside-cli/src/solve.rs` (a small
factory module can keep the conditional code contained). Similarly, gate
`SqlitePoiStore` usage on the store feature and introduce an explicit error
variant in `wildside-cli/src/error.rs` for missing feature support, so the CLI
fails gracefully when built without a required feature. Add feature forwarding
in `wildside-cli/Cargo.toml` so its features map to the root features via
`dep:` dependencies, keeping the root manifest as the source of truth.

Add tests for both happy and unhappy paths. Use `rstest` for unit-level tests
that exercise solver/store selection, and `rstest-bdd` for behavioural tests
that assert the user-visible errors when a feature is disabled. Because the
feature combinations are compile-time, plan to run targeted test commands for
specific feature sets (for example, `--no-default-features` with solver-only or
store-only). Ensure all test modules and scenarios are guarded with the same
`#[cfg(feature = "...")]` selectors so they only compile under the matching
feature set.

Finally, record the feature flag decisions in `docs/wildside-engine-design.md`,
update any testing notes affected by the rstest-bdd version bump, and mark the
Phase 4 feature-flag entry in `docs/roadmap.md` as done. Run formatting,
linting, and tests via the Makefile and keep logs for review.

## Concrete Steps

    rg --files -g 'Cargo.toml'
    rg -n "SqlitePoiStore|Solver" wildside-core wildside-cli
    rg -n "rstest-bdd" -S .

    # If creating the OR-Tools stub crate:
    cargo new --lib wildside-solver-ortools

    # Update manifests and source files as described in the Plan of Work.

    # Format and lint after code/doc changes (capture logs):
    set -o pipefail
    make fmt 2>&1 | tee /tmp/wildside-fmt.log
    make markdownlint 2>&1 | tee /tmp/wildside-markdownlint.log
    make nixie 2>&1 | tee /tmp/wildside-nixie.log
    make check-fmt 2>&1 | tee /tmp/wildside-check-fmt.log
    make lint 2>&1 | tee /tmp/wildside-lint.log

    # Run tests for the default feature set:
    make test 2>&1 | tee /tmp/wildside-test.log

    # Run targeted feature-set tests for missing-feature paths:
    set -o pipefail
    cargo test -p wildside-cli --no-default-features --features solver-vrp \
        2>&1 | tee /tmp/wildside-cli-test-solver-only.log
    cargo test -p wildside-cli --no-default-features --features store-sqlite \
        2>&1 | tee /tmp/wildside-cli-test-store-only.log
    cargo test -p wildside-cli --no-default-features \
        2>&1 | tee /tmp/wildside-cli-test-none.log

## Validation and Acceptance

The change is acceptable when all of the following are true:

- `make check-fmt`, `make lint`, and `make test` succeed with default features
  enabled, and the log files show no warnings or errors.
- Building with `--no-default-features` and only `solver-vrp` or
  `store-sqlite` succeeds, and the corresponding tests assert the expected
  error path for the missing feature.
- Behavioural tests written with `rstest-bdd` v0.3.0 cover both a successful
  solver/store path and at least one missing-feature path.
- `docs/wildside-engine-design.md` documents the new feature flags and default
  behaviour, and `docs/roadmap.md` marks the Phase 4 feature flag entry as done.

## Idempotence and Recovery

All steps are repeatable. If a feature-gated module fails to compile, confirm
that the module is behind the intended `#[cfg(feature = "...")]` guard and that
its dependencies are marked `optional = true` with feature activation via
`dep:`. If a test run fails, re-run the specific feature-set command after the
fix; the commands above are safe to repeat.

## Artifacts and Notes

Keep the following evidence in the logs referenced above:

- `make lint` output showing no clippy warnings.
- `make test` output showing the full test suite passes.
- Feature-specific `cargo test` logs demonstrating missing-feature paths are
  exercised.

## Interfaces and Dependencies

Feature flags must exist in the root manifest with these names:

- `solver-vrp`: enables `wildside-solver-vrp`.
- `solver-ortools`: enables `wildside-solver-ortools`.
- `store-sqlite`: enables the SQLite-backed POI store.

The root package should expose, at minimum:

    pub use wildside_core::{SolveRequest, SolveResponse, Solver};
    #[cfg(feature = "store-sqlite")]
    pub use wildside_core::SqlitePoiStore;

`wildside-core` must compile without `store-sqlite`, and only expose
`SqlitePoiStore` and its errors when the feature is enabled. `wildside-cli`
should compile under any feature combination, returning a dedicated `CliError`
when a solver or store feature is missing.

## Revision note

Updated progress, recorded the workspace dependency surprise, and logged the
feature-gating decisions. Remaining work is the full formatting, linting, and
test validation cycle plus any fixes they uncover.
