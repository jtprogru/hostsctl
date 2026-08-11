.DEFAULT_GOAL := help
SHELL := /usr/bin/env bash

CARGO   ?= cargo
BIN     := hostsctl
VERSION := $(shell awk -F'"' '/^version = / {print $$2; exit}' Cargo.toml)
MSRV    := $(shell awk -F'"' '/^rust-version/ {print $$2; exit}' Cargo.toml)
TARGET  ?=
DIST    := dist
PREFIX  ?= /usr/local
BINDIR  ?= $(PREFIX)/bin

DEBUG_BIN := target/debug/$(BIN)

# Страницы справочника собираются из двух частей: рукописной обвязки в
# docs/src/parts/<locale>/ и тела, которое печатает сам бинарь. Собранный файл
# лежит прямо в коллекции контента — только так у страницы появляется
# оглавление, а `gen-check` роняет сборку, когда копия разошлась с кодом.
PARTS_DIR := docs/src/parts
DOCS_DIR  := docs/src/content/docs
GEN_FILES := $(DOCS_DIR)/reference/cli.md $(DOCS_DIR)/reference/exit-codes.md \
             $(DOCS_DIR)/ru/reference/cli.md $(DOCS_DIR)/ru/reference/exit-codes.md

.PHONY: help
help: ## Show this help
	@awk 'BEGIN {FS = ":.*##"; printf "Usage:\n  make \033[36m<target>\033[0m\n\nTargets:\n"} \
	     /^[a-zA-Z_-]+:.*?##/ { printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

# --- build ------------------------------------------------------------------

.PHONY: build
build: ## Build the debug binary
	$(CARGO) build

.PHONY: release
release: ## Build the optimized binary
	$(CARGO) build --release --locked

.PHONY: run
run: ## Run the binary (make run ARGS="status")
	$(CARGO) run -- $(ARGS)

.PHONY: install
install: release ## Install the binary into $(BINDIR)
	install -d $(BINDIR)
	install -m 0755 target/release/$(BIN) $(BINDIR)/$(BIN)
	@echo "installed: $(BINDIR)/$(BIN)"

.PHONY: uninstall
uninstall: ## Remove the installed binary
	rm -f $(BINDIR)/$(BIN)

# --- quality ----------------------------------------------------------------

.PHONY: fmt
fmt: ## Format the sources
	$(CARGO) fmt --all

.PHONY: fmt-check
fmt-check: ## Fail if the sources are not formatted
	$(CARGO) fmt --all -- --check

.PHONY: clippy
clippy: ## Lint with clippy, warnings are errors
	$(CARGO) clippy --all-targets -- -D warnings

.PHONY: shellcheck
shellcheck: ## Lint the install script
	@command -v shellcheck >/dev/null || { echo "shellcheck not found; run: make install-tools"; exit 1; }
	shellcheck scripts/install.sh
	@if command -v shfmt >/dev/null; then shfmt -d -i 2 -ci scripts/install.sh; fi

.PHONY: actionlint
actionlint: ## Lint the workflow definitions
	@if command -v actionlint >/dev/null; then actionlint; \
	 else echo "actionlint not found — skipping (run: make install-tools)"; fi

.PHONY: lint
lint: fmt-check clippy shellcheck actionlint ## Run every linter

.PHONY: check
check: ## Type-check without producing a binary
	$(CARGO) check --all-targets

# --- tests ------------------------------------------------------------------

.PHONY: test
test: ## Run the unit, integration and doc tests
	$(CARGO) test --locked

.PHONY: msrv
msrv: ## Verify the crate still builds on its minimum supported Rust version
	@command -v rustup >/dev/null || { echo "rustup is required for the MSRV check"; exit 1; }
	rustup toolchain install $(MSRV) --profile minimal --no-self-update
	# `rustup run`, не `cargo +$(MSRV)`: cargo из Homebrew — не rustup-шим и
	# аргумент `+toolchain` не понимает, так что короткая форма работает не везде.
	rustup run $(MSRV) cargo check --all-targets --locked

.PHONY: audit
audit: ## Check dependencies against the RustSec advisory database
	@command -v cargo-audit >/dev/null || $(CARGO) install cargo-audit --locked
	$(CARGO) audit --deny warnings

.PHONY: deny
deny: ## Check licences, advisories, sources and duplicate dependencies
	@command -v cargo-deny >/dev/null || $(CARGO) install cargo-deny --locked
	$(CARGO) deny check

# --- generated documentation ------------------------------------------------

# $(1) — locale directory under parts/, $(2) — page directory under content/docs
define assemble
	@mkdir -p $(2)/reference
	@cat $(PARTS_DIR)/$(1)/reference-cli.head.md          >  $(2)/reference/cli.md
	@$(DEBUG_BIN) docs cli                                >> $(2)/reference/cli.md
	@cat $(PARTS_DIR)/$(1)/reference-cli.tail.md          >> $(2)/reference/cli.md
	@cat $(PARTS_DIR)/$(1)/reference-exit-codes.head.md   >  $(2)/reference/exit-codes.md
	@$(DEBUG_BIN) docs exit-codes                         >> $(2)/reference/exit-codes.md
	@cat $(PARTS_DIR)/$(1)/reference-exit-codes.tail.md   >> $(2)/reference/exit-codes.md
endef

.PHONY: gen
gen: build ## Regenerate the reference pages from the code
	$(call assemble,en,$(DOCS_DIR))
	$(call assemble,ru,$(DOCS_DIR)/ru)
	@echo "regenerated: $(GEN_FILES)"

.PHONY: gen-check
gen-check: ## Fail when the committed reference differs from a fresh generation
	@$(MAKE) --no-print-directory gen
	@if ! git diff --quiet -- $(GEN_FILES); then \
	  echo "generated docs are out of date; run 'make gen' and commit the result" >&2; \
	  git --no-pager diff -- $(GEN_FILES); \
	  exit 1; \
	fi

.PHONY: completions
completions: build ## Generate shell completions into $(DIST)/completions
	@mkdir -p $(DIST)/completions
	@for sh in bash zsh fish; do \
	  $(DEBUG_BIN) completions $$sh > $(DIST)/completions/$(BIN).$$sh; \
	done
	@echo "completions → $(DIST)/completions"

.PHONY: man
man: build ## Generate the man page into $(DIST)/man
	@mkdir -p $(DIST)/man
	@$(DEBUG_BIN) man > $(DIST)/man/$(BIN).1
	@echo "man page → $(DIST)/man/$(BIN).1"

# --- documentation site -----------------------------------------------------

.PHONY: docs-install
docs-install: ## Install the docs site dependencies
	cd docs && npm ci

.PHONY: docs-dev
docs-dev: gen ## Serve the docs site with live reload
	cd docs && npm run dev

.PHONY: docs-build
docs-build: gen ## Build the static docs site
	cd docs && npm run build

.PHONY: docs-preview
docs-preview: ## Serve the built docs site
	cd docs && npm run preview

# --- distribution -----------------------------------------------------------

.PHONY: dist
dist: completions man ## Build one release archive (make dist TARGET=aarch64-apple-darwin)
	@test -n "$(TARGET)" || { echo "usage: make dist TARGET=<rust-target>"; exit 1; }
	$(CARGO) build --release --locked --target $(TARGET)
	@rm -rf $(DIST)/stage && mkdir -p $(DIST)/stage
	@cp target/$(TARGET)/release/$(BIN) $(DIST)/stage/
	@cp -R $(DIST)/completions $(DIST)/man $(DIST)/stage/
	@cp README.md LICENSE $(DIST)/stage/
	tar -czf $(DIST)/$(BIN)-$(TARGET).tar.gz -C $(DIST)/stage .
	@rm -rf $(DIST)/stage
	@echo "$(DIST)/$(BIN)-$(TARGET).tar.gz"

.PHONY: dist-all
dist-all: ## Build every archive the release matrix produces
	@for t in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu \
	          x86_64-unknown-linux-musl aarch64-unknown-linux-musl \
	          x86_64-apple-darwin aarch64-apple-darwin; do \
	  $(MAKE) --no-print-directory dist TARGET=$$t || echo "skipped $$t"; \
	done

.PHONY: checksums
checksums: ## Write dist/checksums.txt for whatever is in dist/
	@cd $(DIST) && { command -v sha256sum >/dev/null && sha256sum $(BIN)-*.tar.gz \
	  || shasum -a 256 $(BIN)-*.tar.gz; } > checksums.txt
	@cat $(DIST)/checksums.txt

# --- release ----------------------------------------------------------------

.PHONY: version-check
version-check: ## Verify Cargo.toml matches a tag (make version-check TAG=v0.1.0)
	@test -n "$(TAG)" || { echo "usage: make version-check TAG=vX.Y.Z"; exit 1; }
	@want="$(TAG)"; want="$${want#v}"; \
	 if [ "$(VERSION)" != "$$want" ]; then \
	   echo "Cargo.toml version is $(VERSION), tag says $$want" >&2; \
	   echo "run 'make release-prep VERSION=$$want' before tagging" >&2; \
	   exit 1; \
	 fi; \
	 if ! grep -q "^## \[$$want\]" CHANGELOG.md; then \
	   echo "CHANGELOG.md has no '## [$$want]' section" >&2; exit 1; \
	 fi; \
	 echo "version $$want is consistent across Cargo.toml, CHANGELOG.md and the tag"

.PHONY: release-prep
release-prep: ## Stamp a version into Cargo.toml (make release-prep VERSION=0.2.0)
	@test -n "$(VERSION)" || { echo "usage: make release-prep VERSION=X.Y.Z"; exit 1; }
	@v="$(VERSION)"; v="$${v#v}"; \
	 awk -v v="$$v" '!done && /^version = /{sub(/"[^"]*"/, "\"" v "\""); done=1} {print}' \
	   Cargo.toml > Cargo.toml.new && mv Cargo.toml.new Cargo.toml; \
	 if ! grep -qx "version = \"$$v\"" Cargo.toml; then \
	   echo "failed to stamp $$v into Cargo.toml" >&2; exit 1; \
	 fi; \
	 $(CARGO) update --workspace --quiet; \
	 echo "stamped $$v — now update CHANGELOG.md, commit, and tag v$$v"

.PHONY: publish-dry
publish-dry: ## Verify the crates.io package without publishing
	$(CARGO) publish --dry-run --locked

# --- aggregate --------------------------------------------------------------

.PHONY: ci
ci: lint test gen-check msrv ## Everything CI runs, in the order CI runs it

.PHONY: install-tools
install-tools: ## Install the external tools the targets above expect
	@if [ "$$(uname)" = "Darwin" ]; then \
	  brew install shellcheck shfmt actionlint; \
	elif command -v apt-get >/dev/null; then \
	  sudo apt-get update && sudo apt-get install -y shellcheck; \
	  bash <(curl -sSfL https://raw.githubusercontent.com/rhysd/actionlint/main/scripts/download-actionlint.bash); \
	  sudo install -m 0755 ./actionlint /usr/local/bin/actionlint; \
	else \
	  echo "unsupported OS — install shellcheck, shfmt and actionlint by hand"; exit 1; \
	fi
	$(CARGO) install cargo-deny cargo-audit --locked

.PHONY: clean
clean: ## Remove build artefacts
	$(CARGO) clean
	rm -rf $(DIST) docs/dist docs/.astro
