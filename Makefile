.PHONY: help clean setup dev dev-full sync-content build run test check trex ios-setup ios-init ios-dev ios-build ios-release ios-sim ios-open screenshot e2e e2e-build demo-build demo-record demo release release-dry-run release-ios release-ios-dry-run lint test-windows test-windows-status test-windows-screenshot

help:
	@echo "Immerse Yourself - Development Commands"
	@echo ""
	@echo "Development:"
	@echo "  make dev             - Start dev server with hot reload"
	@echo "  make dev-full        - Dev with full content from private repo"
	@echo "  make sync-content    - Sync private repo content (no build)"
	@echo "  make build           - Build production application"
	@echo "  make test            - Run Rust tests (uses Rust 1.89)"
	@echo "  make check           - Check code compiles (uses Rust 1.89)"
	@echo ""
	@echo "Running:"
	@echo "  make run             - Run pre-built application"
	@echo "  make trex            - Build and launch application"
	@echo "  make d / make desktop - Alias for make run"
	@echo ""
	@echo "iOS (local macOS build):"
	@echo "  make ios-setup       - Install all prerequisites (Rust, Node, Tauri CLI, iOS targets)"
	@echo "  make ios-init        - Initialize Xcode project (first time only)"
	@echo "  make ios-dev         - Run on iOS Simulator with hot reload"
	@echo "  make ios-build       - Build frontend + Rust for iOS (debug)"
	@echo "  make ios-release     - Build release IPA for TestFlight"
	@echo "  make ios-sim         - Build and run on iOS Simulator"
	@echo "  make ios-open        - Open Xcode project for manual signing/debugging"
	@echo ""
	@echo "E2E Testing (Docker):"
	@echo "  make e2e-build       - Build the E2E test Docker image"
	@echo "  make e2e             - Run all E2E tests"
	@echo "  make screenshot      - Capture project screenshot (Travel > Afternoon)"
	@echo ""
	@echo "Demo Recording (Docker):"
	@echo "  make demo-build      - Build the demo recording Docker image"
	@echo "  make demo-record     - Record full demo (screen + camera)"
	@echo "  make demo            - Alias for demo-record"
	@echo ""
	@echo "Windows Smoke Test (CI):"
	@echo "  make test-windows              - Trigger smoke test on GitHub Actions (real Windows)"
	@echo "  make test-windows VERSION=TAG  - Test a specific release version"
	@echo "  make test-windows-status       - Check smoke test run status"
	@echo "  make test-windows-screenshot   - Download screenshot from latest completed run"
	@echo ""
	@echo "Release:"
	@echo "  make release             - Cut a release (bump patch, tag, push — builds desktop + iOS)"
	@echo "  make release-dry-run     - Show what a release would do without doing it"
	@echo "  make release-ios         - iOS TestFlight only (bump patch, push to main, no tag)"
	@echo "  make release-ios-dry-run - Show what an iOS-only release would do"
	@echo ""
	@echo "Setup:"
	@echo "  make setup           - Install Tauri CLI + configure git hooks"
	@echo ""
	@echo "Maintenance:"
	@echo "  make lint            - Lint Python scripts with ruff"
	@echo "  make clean           - Remove build artifacts"

lint:
	@test -f .venv/bin/ruff || (echo "ruff not found. Run: python3 -m venv .venv && .venv/bin/pip3 install ruff" && exit 1)
	.venv/bin/ruff check scripts/ tests/e2e/

clean:
	rm -rf rust/target 2>/dev/null || true
	@echo "Cleaned build artifacts"

# ============================================================================
# Build Targets (React Frontend + Rust Backend via Tauri)
# ============================================================================

RUST_DIR = rust
# Use cargo-1.89 if available (for systems with multiple Rust versions)
CARGO := $(shell which cargo-1.89 2>/dev/null || which cargo)

TAURI_DIR = $(RUST_DIR)/immerse-tauri

setup:
	@echo "Installing Tauri CLI..."
	$(CARGO) install tauri-cli
	@echo "Configuring git hooks..."
	git config core.hooksPath .githooks
	@echo "Setup complete (Tauri CLI installed, git hooks configured)"

TAURI_CARGO_WRAPPER = /tmp/cargo-wrapper
CARGO_189 = /usr/lib/rust-1.89/bin/cargo
RUSTC_189 = /usr/lib/rust-1.89/bin/rustc
CARGO_TAURI = $(HOME)/.cargo/bin/cargo-tauri

dev:
	@test -f $(CARGO_TAURI) || $(CARGO) install tauri-cli
	@echo "Starting development server..."
	cd $(TAURI_DIR)/ui && npm install && npm run dev &
	@mkdir -p $(TAURI_CARGO_WRAPPER) && printf '#!/bin/bash\nRUSTC=$(RUSTC_189) exec $(CARGO_189) "$$@"\n' > $(TAURI_CARGO_WRAPPER)/cargo && chmod +x $(TAURI_CARGO_WRAPPER)/cargo
	cd $(TAURI_DIR) && PATH=$(TAURI_CARGO_WRAPPER):$$PATH $(CARGO_TAURI) dev
	@rm -rf $(TAURI_CARGO_WRAPPER)

# Default path to private content repo
IMMERSE_PRIVATE_REPO ?= $(HOME)/iye/immerse_yourself

# User content directory (Linux default)
USER_CONTENT_DIR ?= $(HOME)/.local/share/com.peterlesko.immerseyourself

dev-full: sync-content ## Start dev with all private content loaded
	$(MAKE) dev

sync-content: ## Copy all private repo content to user content dir (no build)
	@if [ ! -d "$(IMMERSE_PRIVATE_REPO)/env_conf" ]; then \
		echo "Error: Private repo not found at $(IMMERSE_PRIVATE_REPO)"; \
		echo "Set IMMERSE_PRIVATE_REPO to the correct path"; \
		exit 1; \
	fi
	@mkdir -p "$(USER_CONTENT_DIR)/env_conf" "$(USER_CONTENT_DIR)/sound_conf" "$(USER_CONTENT_DIR)/sounds"
	@cp -u $(IMMERSE_PRIVATE_REPO)/env_conf/*.yaml "$(USER_CONTENT_DIR)/env_conf/" 2>/dev/null || true
	@cp -u $(IMMERSE_PRIVATE_REPO)/sound_conf/*.yaml "$(USER_CONTENT_DIR)/sound_conf/" 2>/dev/null || true
	@cp -u $(IMMERSE_PRIVATE_REPO)/sounds/* "$(USER_CONTENT_DIR)/sounds/" 2>/dev/null || true
	@if [ -d "$(IMMERSE_PRIVATE_REPO)/freesound_sounds" ]; then \
		mkdir -p "$(USER_CONTENT_DIR)/freesound_sounds/cc0" "$(USER_CONTENT_DIR)/freesound_sounds/cc-by"; \
		cp -u $(IMMERSE_PRIVATE_REPO)/freesound_sounds/cc0/* "$(USER_CONTENT_DIR)/freesound_sounds/cc0/" 2>/dev/null || true; \
		cp -u $(IMMERSE_PRIVATE_REPO)/freesound_sounds/cc-by/* "$(USER_CONTENT_DIR)/freesound_sounds/cc-by/" 2>/dev/null || true; \
		cp -u "$(IMMERSE_PRIVATE_REPO)/freesound_sounds/manifest.json" "$(USER_CONTENT_DIR)/freesound_sounds/manifest.json" 2>/dev/null || true; \
	fi
	@echo "Synced content from $(IMMERSE_PRIVATE_REPO) into $(USER_CONTENT_DIR)"

build:
	@test -f $(CARGO_TAURI) || $(CARGO) install tauri-cli
	@echo "Building application..."
	cd $(TAURI_DIR)/ui && npm install && npm run build
	@mkdir -p $(TAURI_CARGO_WRAPPER) && printf '#!/bin/bash\nRUSTC=$(RUSTC_189) exec $(CARGO_189) "$$@"\n' > $(TAURI_CARGO_WRAPPER)/cargo && chmod +x $(TAURI_CARGO_WRAPPER)/cargo
	cd $(TAURI_DIR) && PATH=$(TAURI_CARGO_WRAPPER):$$PATH $(CARGO_TAURI) build --no-bundle || PATH=$(TAURI_CARGO_WRAPPER):$$PATH $(CARGO_TAURI) build
	@rm -rf $(TAURI_CARGO_WRAPPER)
	@echo "Build complete. Check $(TAURI_DIR)/target/release/"

trex: build
	@echo "Launching Immerse Yourself..."
	$(RUST_DIR)/target/release/immerse-tauri

test:
	@echo "Running Rust tests (using Rust 1.89)..."
	@mkdir -p $(TAURI_CARGO_WRAPPER) && printf '#!/bin/bash\nRUSTC=$(RUSTC_189) exec $(CARGO_189) "$$@"\n' > $(TAURI_CARGO_WRAPPER)/cargo && chmod +x $(TAURI_CARGO_WRAPPER)/cargo
	cd $(RUST_DIR) && PATH=$(TAURI_CARGO_WRAPPER):$$PATH cargo test --lib --bins
	@rm -rf $(TAURI_CARGO_WRAPPER)

check:
	@echo "Checking code compiles (using Rust 1.89)..."
	@mkdir -p $(TAURI_CARGO_WRAPPER) && printf '#!/bin/bash\nRUSTC=$(RUSTC_189) exec $(CARGO_189) "$$@"\n' > $(TAURI_CARGO_WRAPPER)/cargo && chmod +x $(TAURI_CARGO_WRAPPER)/cargo
	cd $(RUST_DIR) && PATH=$(TAURI_CARGO_WRAPPER):$$PATH cargo check
	@rm -rf $(TAURI_CARGO_WRAPPER)

run:
	@echo "Running Immerse Yourself..."
	$(RUST_DIR)/target/release/immerse-tauri

d: run

desktop: run

# ============================================================================
# iOS Local Build Targets (macOS only)
# ============================================================================
# These targets work on macOS without the Rust 1.89 wrapper hack needed on
# Linux. A fresh `rustup install` gives the latest Rust directly.

ios-setup:
	@echo "=== iOS Development Setup ==="
	@echo "Checking prerequisites..."
	@which xcodebuild > /dev/null 2>&1 || (echo "Error: Xcode not found. Install from App Store first." && exit 1)
	@xcode-select -p > /dev/null 2>&1 || (echo "Run: sudo xcode-select --install" && exit 1)
	@echo "[OK] Xcode found"
	@which rustup > /dev/null 2>&1 || (echo "Installing Rust..." && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y)
	@echo "[OK] Rust installed"
	rustup target add aarch64-apple-ios aarch64-apple-ios-sim
	@echo "[OK] iOS targets added"
	@which node > /dev/null 2>&1 || (echo "Error: Node.js not found. Run: brew install node" && exit 1)
	@echo "[OK] Node.js found"
	@which cargo-tauri > /dev/null 2>&1 || (echo "Installing Tauri CLI via binstall..." && \
		curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash && \
		cargo binstall tauri-cli --no-confirm)
	@echo "[OK] Tauri CLI installed"
	@echo ""
	@echo "=== Setup complete! Next steps: ==="
	@echo "  make ios-init   # first time only"
	@echo "  make ios-dev    # run on simulator"

ios-frontend:
	@echo "Building frontend..."
	cd $(TAURI_DIR)/ui && npm install && npm run build

ios-init: ios-frontend
	@echo "Initializing iOS Xcode project..."
	cd $(TAURI_DIR) && cargo tauri ios init
	@echo ""
	@echo "Xcode project created at $(TAURI_DIR)/gen/apple/"
	@echo "To configure signing: make ios-open"

ios-dev:
	@echo "Starting iOS Simulator with hot reload..."
	@echo "(Press Ctrl+C to stop)"
	cd $(TAURI_DIR)/ui && npm install
	cd $(TAURI_DIR) && cargo tauri ios dev

ios-build: ios-frontend
	@echo "Building iOS app (debug)..."
	cd $(TAURI_DIR) && cargo tauri ios build

ios-release: ios-frontend
	@echo "Building iOS release IPA for TestFlight..."
	cd $(TAURI_DIR) && cargo tauri ios build --export-method app-store-connect
	@echo ""
	@IPA=$$(find $(TAURI_DIR)/gen/apple -name "*.ipa" -type f 2>/dev/null | head -1); \
	if [ -n "$$IPA" ]; then \
		echo "IPA built: $$IPA"; \
		echo "Upload to TestFlight with:"; \
		echo "  xcrun altool --upload-app --type ios --file \"$$IPA\" --username YOUR_APPLE_ID --password YOUR_APP_SPECIFIC_PASSWORD"; \
	else \
		echo "Warning: IPA file not found. Check build output above."; \
	fi

ios-sim: ios-frontend
	@echo "Building and launching on iOS Simulator..."
	cd $(TAURI_DIR) && cargo tauri ios dev --no-watch

ios-open:
	@echo "Opening Xcode project..."
	@test -d $(TAURI_DIR)/gen/apple || (echo "Run 'make ios-init' first" && exit 1)
	open $(TAURI_DIR)/gen/apple/immerse-tauri.xcodeproj

# ============================================================================
# E2E Testing (Docker-based)
# ============================================================================
# Builds the app inside Docker, launches it with Xvfb, and runs Python tests
# that interact with the UI via the WebKit Inspector Protocol.

E2E_DIR = tests/e2e
E2E_IMAGE = immerse-e2e
# Use sudo if the current user can't access the Docker socket
DOCKER := $(shell docker info >/dev/null 2>&1 && echo docker || echo sudo docker)

e2e-build:
	@echo "Building E2E test Docker image..."
	$(DOCKER) build -t $(E2E_IMAGE) -f $(E2E_DIR)/Dockerfile .

e2e: e2e-build
	@echo "Running E2E tests..."
	$(DOCKER) run --rm -v $(CURDIR)/$(E2E_DIR)/output:/output $(E2E_IMAGE) -v

screenshot:
	@echo "Capturing project screenshot (Travel > Afternoon)..."
	DISPLAY=$${DISPLAY:-:1} python3 tests/e2e/run_local.py
	@if [ -f $(E2E_DIR)/output/immerse_yourself_screenshot.jpg ]; then \
		cp -f $(E2E_DIR)/output/immerse_yourself_screenshot.jpg immerse_yourself_screenshot.jpg; \
		echo "Screenshot saved: immerse_yourself_screenshot.jpg"; \
	else \
		echo "Error: Screenshot not generated"; \
		exit 1; \
	fi

# ============================================================================
# Demo Recording (Docker-based)
# ============================================================================
# Runs the full demo inside Docker with screen recording (Xvfb + ffmpeg).
# Camera recording for smart lights runs on the host.

DEMO_IMAGE = immerse-demo

demo-build:
	@if ! $(DOCKER) image inspect $(E2E_IMAGE) >/dev/null 2>&1; then \
		echo "E2E base image not found, building first..."; \
		$(MAKE) e2e-build; \
	fi
	@echo "Building demo recording image..."
	$(DOCKER) build -t $(DEMO_IMAGE) demo/

demo-record: demo-build
	bash demo/record_demo.sh

demo: demo-record

# ============================================================================
# Releases (Desktop + iOS)
# ============================================================================
# `make release`     — Bumps version, creates a git tag, pushes to origin.
#                      The tag triggers desktop-build.yml; the push triggers ios-build.yml.
# `make release-ios` — Bumps version, pushes to main WITHOUT a tag.
#                      Only triggers ios-build.yml (TestFlight iteration).
# Pass RELEASE_ARGS for options: --minor, --major, --version X.Y.Z, --no-monitor

RELEASE_ARGS ?=

release:
	python3 scripts/desktop-release.py $(RELEASE_ARGS)

release-dry-run:
	python3 scripts/desktop-release.py --dry-run $(RELEASE_ARGS)

release-ios:
	python3 scripts/desktop-release.py --ios-only $(RELEASE_ARGS)

release-ios-dry-run:
	python3 scripts/desktop-release.py --ios-only --dry-run $(RELEASE_ARGS)

# ============================================================================
# Windows Smoke Test (CI-based, real Windows)
# ============================================================================
# Launches the Windows build on a GitHub Actions windows-latest runner,
# verifies it starts, and captures a screenshot.
# Usage: make test-windows [VERSION=v0.3.24]

VERSION ?=

test-windows:
	@echo "Triggering Windows smoke test on GitHub Actions..."
	@if [ -n "$(VERSION)" ]; then \
		gh workflow run windows-smoke-test.yml -f version=$(VERSION); \
		echo "Testing version: $(VERSION)"; \
	else \
		gh workflow run windows-smoke-test.yml; \
		echo "Testing latest release"; \
	fi
	@echo ""
	@echo "Monitor progress:"
	@echo "  make test-windows-status"
	@echo "  make test-windows-screenshot"

test-windows-status:
	@gh run list --workflow=windows-smoke-test.yml --limit 3

test-windows-screenshot:
	@echo "Downloading latest smoke test screenshot..."
	@RUN_ID=$$(gh run list --workflow=windows-smoke-test.yml --status=completed --limit 1 --json databaseId --jq '.[0].databaseId'); \
	if [ -z "$$RUN_ID" ]; then \
		echo "No completed smoke test runs found."; \
		exit 1; \
	fi; \
	gh run download $$RUN_ID --dir windows-smoke-test-output/; \
	echo "Screenshot downloaded to windows-smoke-test-output/"
