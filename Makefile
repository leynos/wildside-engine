.PHONY: help all clean test bench build release lint fmt check-fmt markdownlint nixie typecheck

APP ?= wildside-engine
CARGO ?= cargo
BUILD_JOBS ?=
CLIPPY_FLAGS ?= --workspace --all-targets --all-features -- -D warnings
MDLINT ?= markdownlint
NIXIE ?= nixie
TEST_FLAGS ?=

build: target/debug/$(APP) ## Build debug binary
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artifacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="-D warnings" $(CARGO) nextest run --workspace --all-targets --features test-support $(TEST_FLAGS) $(BUILD_JOBS)

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

markdownlint: ## Lint Markdown files
	find . -type f -name '*.md' -not -path './target/*' -print0 | xargs -0 $(MDLINT)

nixie: ## Validate Mermaid diagrams
	# CI currently requires --no-sandbox; remove once nixie supports
	# environment variable control for this option
	nixie --no-sandbox

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
