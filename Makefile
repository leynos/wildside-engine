.PHONY: help all clean test test-workflow-contracts bench build release lint fmt check-fmt markdownlint spelling nixie typecheck

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
NIXIE_VERSION ?= 1.1.0
NIXIE = $(UV_ENV) $(UV) tool run --python 3.14 \
	--from nixie-cli@$(NIXIE_VERSION) nixie
TYPOS_CONFIG_BUILDER_VERSION ?= v0.1.1
TYPOS_CONFIG_BUILDER = $(UV_ENV) $(UV) tool run --python 3.14 --from \
	"git+https://github.com/leynos/typos-config-builder.git@$(TYPOS_CONFIG_BUILDER_VERSION)" \
	typos-config-builder
WHITAKER ?= whitaker
TEST_FLAGS ?=

build: target/debug/$(APP) ## Build debug binary
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artefacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="-D warnings" $(CARGO) nextest run --workspace --all-targets --features test-support $(TEST_FLAGS) $(BUILD_JOBS)

test-workflow-contracts: ## Validate the workflow contracts, including the CV-005 CodeScene shape
	uv run --with 'pytest>=8' --with 'pyyaml>=6' pytest tests/workflow_contracts -q

bench: ## Run performance benchmarks
	$(CARGO) bench --package wildside-solver-vrp

target/%/$(APP): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP)

lint: ## Run Clippy and the Whitaker Dylint suite with warnings denied
	$(CARGO) clippy $(CLIPPY_FLAGS)
	RUSTFLAGS="-D warnings" $(WHITAKER) --all -- --all-targets --all-features

typecheck: ## Typecheck the workspace
	RUSTFLAGS="-D warnings" $(CARGO) check --workspace --all-targets --all-features $(BUILD_JOBS)

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
