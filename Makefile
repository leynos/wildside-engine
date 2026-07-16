.PHONY: help all clean test test-workflow-contracts bench build release lint fmt check-fmt markdownlint spelling spelling-phrase-check spelling-config spelling-config-write spelling-helper-test nixie typecheck

APP ?= wildside-engine
CARGO ?= cargo
BUILD_JOBS ?=
CLIPPY_FLAGS ?= --workspace --all-targets --all-features -- -D warnings
MDLINT ?= markdownlint
UV ?= uv
UV_ENV = UV_CACHE_DIR=.uv-cache UV_TOOL_DIR=.uv-tools
NIXIE_VERSION ?= 1.1.0
NIXIE = $(UV_ENV) $(UV) tool run --python 3.14 \
	--from nixie-cli@$(NIXIE_VERSION) nixie
RUFF_VERSION ?= 0.15.12
PATHSPEC_VERSION ?= 1.1.1
TYPOS_VERSION ?= 1.48.0
TYPOS_CONFIG_BUILDER_COMMIT := b604f198797fdd36a567dd0f8f07b13f9539b241
TYPOS_CONFIG_BUILDER_SOURCE := git+https://github.com/leynos/typos-config-builder.git@$(TYPOS_CONFIG_BUILDER_COMMIT)
TYPOS_CONFIG_BUILDER := $(UV_ENV) $(UV) tool run --python 3.14 \
	--from "$(TYPOS_CONFIG_BUILDER_SOURCE)" typos-config-builder
SPELLING_PY_SRCS := \
	scripts/typos_rollout_check.py scripts/tests/test_typos_rollout_check.py
SPELLING_PY_TESTS := scripts/tests/test_typos_rollout_check.py
SPELLING_COVERAGE_ARGS := --cov=typos_rollout_check --cov-fail-under=90
SPELLING_PY_ENV := PYTHONDONTWRITEBYTECODE=1
SPELLING_COVERAGE_FILE ?= /tmp/$(APP)-spelling-helper.coverage
SPELLING_HELPER_PYTEST = PYTHONPATH=scripts $(SPELLING_PY_ENV) \
	COVERAGE_FILE=$(SPELLING_COVERAGE_FILE) $(UV_ENV) $(UV) run --no-project \
	--python 3.14 --with pathspec==$(PATHSPEC_VERSION) --with pytest==9.0.2 \
	--with pytest-cov==7.0.0 python -m pytest
TEST_FLAGS ?=

build: target/debug/$(APP) ## Build debug binary
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artefacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="-D warnings" $(CARGO) nextest run --workspace --all-targets --features test-support $(TEST_FLAGS) $(BUILD_JOBS)

test-workflow-contracts: ## Validate the mutation-testing caller contract
	uv run --with 'pytest>=8' --with 'pyyaml>=6' pytest tests/workflow_contracts -q

bench: ## Run performance benchmarks
	$(CARGO) bench --package wildside-solver-vrp

target/%/$(APP): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP)

lint: ## Run Clippy with warnings denied
	$(CARGO) clippy $(CLIPPY_FLAGS)

typecheck: ## Typecheck the workspace
	RUSTFLAGS="-D warnings" $(CARGO) check --workspace --all-targets --all-features $(BUILD_JOBS)

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check

markdownlint: spelling ## Lint Markdown files and enforce spelling
	git ls-files -z -- '*.md' | xargs -0 -r $(MDLINT)

spelling: spelling-phrase-check ## Enforce en-GB-oxendict spelling
	@git ls-files -z | xargs -0 -r env $(UV_ENV) \
		$(UV) tool run typos@$(TYPOS_VERSION) --config typos.toml --force-exclude --hidden

spelling-phrase-check: spelling-config ## Reject prohibited phrase forms
	@PYTHONPATH=scripts $(SPELLING_PY_ENV) $(UV_ENV) $(UV) run --no-project --python 3.14 \
		scripts/typos_rollout_check.py --repository .

spelling-config: spelling-helper-test ## Check generated spelling configuration
	@git ls-files --error-unmatch typos.toml >/dev/null
	@$(TYPOS_CONFIG_BUILDER) --repository . --check

spelling-config-write: spelling-helper-test ## Regenerate spelling configuration
	@$(TYPOS_CONFIG_BUILDER) --repository .

spelling-helper-test: ## Validate the spelling phrase helper
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) format --isolated --target-version py313 --check $(SPELLING_PY_SRCS)
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) check --isolated --target-version py313 $(SPELLING_PY_SRCS)
	@$(SPELLING_HELPER_PYTEST) $(SPELLING_PY_TESTS) -c /dev/null --rootdir=. -p no:cacheprovider $(SPELLING_COVERAGE_ARGS)

nixie: ## Validate Mermaid diagrams
	# CI currently requires --no-sandbox; remove once nixie supports
	# environment variable control for this option
	$(NIXIE) --no-sandbox

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
