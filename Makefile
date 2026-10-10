# Public development interface. Specialized backend tools remain in tools/.
.DEFAULT_GOAL := help
.NOTPARALLEL:

NODE ?= node
CARGO ?= cargo
UV ?= uv
PROFILE ?= debug
OUTPUT ?=
ARGS ?=
CODEGRAPH_VERSION := 1.6.2
BACKEND = $(NODE) tools/backend.mjs
export UV
export IRIS_PROFILE = $(PROFILE)
export IRIS_OUTPUT = $(OUTPUT)

ifeq ($(filter $(PROFILE),debug release),)
$(error PROFILE must be debug or release)
endif
ifeq ($(PROFILE),release)
PROFILE_FLAGS := --release
endif

.PHONY: help doctor env env-validation env-models setup deps hooks hooks-test \
 codegraph codegraph-install codegraph-init codegraph-sync codegraph-status \
 check check-rust check-web fmt fmt-check test test-rust test-tools test-web \
 test-native build build-core build-web build-desktop api api-check public-check \
 staged-check verify models models-dino models-scrfd models-dino-directml \
 media raw directml source portable require-output require-windows

help: ## List commands and configuration
	@$(NODE) tools/development.mjs help
doctor: ## Check toolchains and the local backend environment without installing
	@$(NODE) tools/development.mjs doctor
env: ## Prepare an isolated backend runtime using uv (may download)
	$(NODE) tools/development.mjs env
env-validation: env ## Install optional media validation dependencies
	$(UV) pip install --python "$(CURDIR)/.venv/$(if $(filter Windows_NT,$(OS)),Scripts/python.exe,bin/python)" -r tools/validation-requirements.txt
env-models: env ## Install optional model conversion dependencies
	$(UV) pip install --python "$(CURDIR)/.venv/$(if $(filter Windows_NT,$(OS)),Scripts/python.exe,bin/python)" onnx==1.23.2
setup: env deps hooks ## Prepare environment, locked dependencies and repository Git hooks
deps: ## Install locked repository and frontend dependencies
	npm ci
	npm --prefix apps/shell ci
hooks: ## Install Lefthook and the repository-local commit template
	$(NODE) tools/install-hooks.mjs
hooks-test: ## Exercise real Git commits in disposable repositories
	$(BACKEND) -m unittest discover -s tools/tests -p test_git_hooks.py -v

codegraph: codegraph-install ## Install optional CodeGraph locally (no indexing or agent registration)
codegraph-install:
	npm install --prefix .tools/codegraph --no-save --package-lock=false --no-audit --no-fund @colbymchenry/codegraph@$(CODEGRAPH_VERSION)
codegraph-init: ## Explicitly create a local project graph with the installed CodeGraph
	npm --prefix .tools/codegraph exec --no -- codegraph init "$(CURDIR)"
codegraph-sync: ## Refresh an existing local graph
	npm --prefix .tools/codegraph exec --no -- codegraph sync "$(CURDIR)"
codegraph-status: ## Inspect the local graph
	npm --prefix .tools/codegraph exec --no -- codegraph status "$(CURDIR)"

check: fmt-check check-rust check-web ## Check all Rust formatting, core compilation and frontend types
check-rust: ## Check the core Rust workspace
	$(CARGO) check --workspace --locked
check-web: ## Check frontend types
	npm --prefix apps/shell run check
fmt: ## Format all three Rust workspaces
	$(CARGO) fmt --all
	$(CARGO) fmt --manifest-path apps/shell/src-tauri/Cargo.toml --all
	$(CARGO) fmt --manifest-path components/raw-decoder/Cargo.toml --all
fmt-check: ## Check formatting in all three Rust workspaces
	$(CARGO) fmt --all -- --check
	$(CARGO) fmt --manifest-path apps/shell/src-tauri/Cargo.toml --all -- --check
	$(CARGO) fmt --manifest-path components/raw-decoder/Cargo.toml --all -- --check
test: test-rust test-tools test-web test-native ## Run core, tooling, frontend and native library tests
test-rust: ## Test the core workspace (ignored media tests stay ignored)
	$(CARGO) test --workspace --locked $(PROFILE_FLAGS)
test-tools: ## Test Make integration and internal provisioning/packaging tools
	$(NODE) --test tools/tests/development.test.mjs
	$(BACKEND) -m unittest discover -s tools/tests -p test_*.py -v
test-web: ## Test the frontend (live API requires configured fixtures)
	npm --prefix apps/shell test
test-native: ## Test the native host and updater library
	$(CARGO) test --locked --manifest-path apps/shell/src-tauri/Cargo.toml --features updater-client --lib $(PROFILE_FLAGS)
build: build-core build-web ## Build core binaries, independent RAW converter and frontend
build-core: ## Build core binaries and the independent RAW converter
	$(CARGO) build --workspace --locked $(PROFILE_FLAGS)
	$(CARGO) build --locked --manifest-path components/raw-decoder/Cargo.toml $(PROFILE_FLAGS)
build-web: ## Build frontend assets and notices
	npm --prefix apps/shell run build
build-desktop: build-web ## Build the native desktop shell
	$(CARGO) build --locked --manifest-path apps/shell/src-tauri/Cargo.toml --features desktop $(PROFILE_FLAGS)
api: ## Regenerate the API schema and frontend declarations
	$(NODE) tools/api.mjs
api-check: ## Compare API outputs without writing tracked files
	$(NODE) tools/api.mjs --check
public-check: ## Check the staged public-source tree and documentation links
	$(BACKEND) tools/check-public-tree.py
staged-check: ## Check staged syntax and source boundaries
	$(BACKEND) tools/check-staged.py
verify: check test api-check public-check ## Run source checks, tests, API comparison and public-tree validation

models: ## Download and hash-check default inference models
	$(BACKEND) tools/setup-models.py $(ARGS)
models-dino: env-models ## Explicit optional DINOv3 provisioning; review its license first
	$(BACKEND) tools/setup-dinov3.py $(ARGS)
models-scrfd: env-models ## Explicit research-only SCRFD provisioning; requires ARGS=--research-only
	$(BACKEND) tools/setup_scrfd.py $(ARGS)
models-dino-directml: require-windows env-models ## Prepare an optional DirectML DINO graph
	$(BACKEND) tools/setup-dinov3-directml.py $(ARGS)
media: require-windows ## Build the Windows HEIC runtime from pinned sources
	$(BACKEND) tools/setup-heif-runtime.py $(ARGS)
raw: require-windows ## Provision ExifTool and build the independent RAW converter
	$(BACKEND) tools/setup-raw-runtime.py $(ARGS)
	$(CARGO) build --locked --manifest-path components/raw-decoder/Cargo.toml $(PROFILE_FLAGS)
directml: require-windows ## Provision the optional Windows DirectML runtime
	$(BACKEND) tools/setup-directml-runtime.py $(ARGS)
source: require-output ## Export clean HEAD to OUTPUT (existing destinations are refused)
	$(BACKEND) tools/package-source.py --output "$(OUTPUT)"
portable: require-windows require-output ## Build a Windows portable package in OUTPUT
	$(NODE) tools/development.mjs portable
require-output:
	@$(NODE) tools/development.mjs require-output
require-windows:
	@$(NODE) tools/development.mjs require-windows
