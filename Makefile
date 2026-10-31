.PHONY: help all clean test test-benches test-workflow-contracts bench build release lint fmt check-fmt markdownlint spelling nixie typecheck

APP ?= wildside-engine
CARGO ?= cargo
BUILD_JOBS ?=
CLIPPY_FLAGS ?= --workspace --all-targets --all-features -- -D warnings
# markdownlint-cli2, which reads .markdownlint-cli2.jsonc; the fallback is
# Bun's bin directory, where the estate installs it.
MDLINT ?= $(shell command -v markdownlint-cli2 2>/dev/null || printf '%s' "$$HOME/.bun/bin/markdownlint-cli2")
# `make fmt` and `make check-fmt` call mdtablefix directly. `--git` selects the
# Markdown files Git tracks and `--include-untracked` adds the untracked files
# Git does not ignore, so a new document is formatted before it is staged.
# Both modes need mdtablefix 0.6.0 or later; CI pins the version at the
# install-mdtablefix step.
MDTABLEFIX ?= mdtablefix
MDTABLEFIX_SELECT = --git --include-untracked
MDTABLEFIX_RULES = --wrap --renumber --breaks --ellipsis --fences
UV ?= uv
UV_ENV = UV_CACHE_DIR=.uv-cache UV_TOOL_DIR=.uv-tools
# The CV-005 CodeScene contracts live in shared-actions and run from a full
# commit, so a fix is a pin bump. `.github/cv005.toml` holds this repository's
# only parameters.
CV005_CONTRACTS_REF ?= a38feb9be25755c30eca5bda96bd3786a5b89c6b
CV005_CONTRACTS = $(UV_ENV) $(UV) tool run --python 3.13 \
	--from 'git+https://github.com/leynos/shared-actions@$(CV005_CONTRACTS_REF)\#subdirectory=packages/cv005-contracts' \
	cv005-contracts
NIXIE_VERSION ?= 1.1.0
NIXIE = $(UV_ENV) $(UV) tool run --python 3.14 \
	--from nixie-cli@$(NIXIE_VERSION) nixie
TYPOS_CONFIG_BUILDER_VERSION ?= v0.1.3
TYPOS_CONFIG_BUILDER = $(UV_ENV) $(UV) tool run --python 3.14 --from \
	"git+https://github.com/leynos/typos-config-builder.git@$(TYPOS_CONFIG_BUILDER_VERSION)" \
	typos-config-builder
WHITAKER ?= whitaker
TEST_FLAGS ?=
# The build standard (Concordat `rust-build-defaults`): the parallel `rustc`
# frontend and, on Linux, the `mold` linker. An assigned `RUSTFLAGS` replaces
# every `rustflags` source in `.cargo/config.toml`, so each gate recipe that
# assigns it composes these onto any inherited value (CI's setup-rust exports
# one). The Whitaker line keeps its own boundary: Whitaker builds its lint
# driver on a different toolchain, which must not inherit these flags.
BUILD_HOST_OS := $(shell uname -s)
STANDARD_RUSTFLAGS := -Zthreads=8$(if $(filter Linux,$(BUILD_HOST_OS)), -Clink-arg=-fuse-ld=mold)
# The linker flag is chosen from the host, so a gate that compiles for another
# target would get the wrong one (`mold` on a non-Linux target, or none for a Linux
# target built from another host). Refuse that rather than guess; run a
# cross-compiling build with `cargo` directly.
STANDARD_TARGET_GUARD = $(if $(or $(strip $(CARGO_BUILD_TARGET)),$(findstring --target,$(TEST_FLAGS) $(CLIPPY_FLAGS) $(BUILD_JOBS))),$(error the build standard picks `mold` from the host; unset CARGO_BUILD_TARGET and drop --target, or run cargo directly))
GATE_RUSTFLAGS = $(STANDARD_TARGET_GUARD)RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings $(STANDARD_RUSTFLAGS)"
# Debug builds keep a caller's exported flags and add the standard ones, because
# an inherited `RUSTFLAGS` would otherwise displace the configuration. Release
# builds add neither (a caller's own value passes through untouched).
DEBUG_RUSTFLAGS = $(STANDARD_TARGET_GUARD)RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }$(STANDARD_RUSTFLAGS)"

build: target/debug/$(APP) ## Build debug binary
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artefacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	$(GATE_RUSTFLAGS) $(CARGO) nextest run --workspace --all-targets --features test-support $(TEST_FLAGS) $(BUILD_JOBS)

# The bench targets alone, under the same features as `test`. `--benches` also
# selects every library, whose `bench` setting defaults to true, so the filter
# keeps only the bench binaries. CI runs this instead of `test`, because the
# coverage run already executes every other target `test` selects; see
# tests/workflow_contracts/suite_runs_once_test.py.
test-benches: ## Run the benchmark targets once each as tests
	$(GATE_RUSTFLAGS) $(CARGO) nextest run --workspace --benches --features test-support -E 'kind(bench)' $(BUILD_JOBS)

test-workflow-contracts: ## Validate the workflow contracts, including the CV-005 CodeScene shape
	$(CV005_CONTRACTS) check --repository .
	uv run --with 'pytest>=8' --with 'pyyaml>=6' pytest tests/workflow_contracts -q

bench: ## Run performance benchmarks
	$(CARGO) bench --package wildside-solver-vrp

target/%/$(APP): ## Build binary in debug or release mode
	$(if $(findstring release,$(@)),,$(DEBUG_RUSTFLAGS) )$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP)

lint: ## Run Clippy and the Whitaker Dylint suite with warnings denied
	$(CARGO) clippy $(CLIPPY_FLAGS)
	RUSTFLAGS="-D warnings" $(WHITAKER) --all -- --all-targets --all-features

typecheck: ## Typecheck the workspace
	$(GATE_RUSTFLAGS) $(CARGO) check --workspace --all-targets --all-features $(BUILD_JOBS)

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	$(MDTABLEFIX) --in-place $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)
	@unset FORCE_COLOR; $(MDLINT) --fix "**/*.md"

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check
	$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)

markdownlint: spelling ## Lint Markdown files and enforce spelling
	git ls-files -z -- '*.md' | xargs -0 -r $(MDLINT)

spelling: ## Enforce en-GB-oxendict spelling
	$(TYPOS_CONFIG_BUILDER) gate --repository . --scope all

nixie: ## Validate Mermaid diagrams
	# CI currently requires --no-sandbox; remove once nixie supports
	# environment variable control for this option
	$(NIXIE) --no-sandbox

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
