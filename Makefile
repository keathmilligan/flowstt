# FlowSTT Makefile
# Build all components for testing

.PHONY: all clean build build-debug build-release build-cuda \
        frontend app cli \
        run-app run-app-release \
        run-cli run-cli-release \
        lint lint-rust lint-ts test \
        install-deps check-binaries help \
        package package-release

# Default target
all: build

# Build all components in release mode
build: build-release

# Build all components in debug mode (faster compilation)
build-debug: frontend app-debug cli-debug

# Build all components in release mode
build-release: frontend app-release cli-release

# Build all components with CUDA acceleration (requires NVIDIA CUDA Toolkit on Linux)
build-cuda: frontend app-cuda cli-release

# =============================================================================
# Individual Components
# =============================================================================

# Build frontend (TypeScript/Vite)
frontend:
	@echo "==> Building frontend..."
	pnpm build

# Build flowstt-app/Tauri GUI (debug) - includes engine
app-debug:
	@echo "==> Building flowstt-app (debug)..."
	cargo build -p flowstt-app

# Build flowstt-app/Tauri GUI (release) - includes engine
app-release: cli-release
	@echo "==> Building flowstt-app (release)..."
	cargo build -p flowstt-app --release

# Build flowstt-app with CUDA acceleration (release)
app-cuda: cli-release
	@echo "==> Building flowstt-app with CUDA (release)..."
	cargo build -p flowstt-app --release --features cuda

# Alias for release
app: app-release

# Build flowstt CLI (debug)
cli-debug:
	@echo "==> Building flowstt CLI (debug)..."
	cargo build -p flowstt-cli

# Build flowstt CLI (release)
cli-release:
	@echo "==> Building flowstt CLI (release)..."
	cargo build -p flowstt-cli --release

# Alias for release
cli: cli-release

# =============================================================================
# Linting
# =============================================================================

# Run all linters
lint: lint-rust lint-ts

# Rust linting (per crate, in order)
# src-tauri is linted last because tauri-build validates macOS bundle resource paths
# at build-script time (even during clippy). cli-release produces
# target/release/flowstt, and building flowstt-engine runs its build script which
# downloads libwhisper.dylib and copies it to target/release/. Both must exist
# before tauri-build executes.
lint-rust: cli-release
	@echo "==> Linting src-common..."
	cargo clippy --manifest-path src-common/Cargo.toml --all-targets -- -D warnings
	@echo "==> Linting src-engine..."
	cargo clippy --manifest-path src-engine/Cargo.toml --all-targets -- -D warnings
	@echo "==> Linting src-cli..."
	cargo clippy --manifest-path src-cli/Cargo.toml --all-targets -- -D warnings
	@echo "==> Linting src-tauri..."
	cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings

# TypeScript linting
lint-ts:
	@echo "==> TypeScript type check..."
	pnpm exec tsc --noEmit

# =============================================================================
# Testing
# =============================================================================

# Run all tests
test: test-rust

# Rust tests (all crates)
test-rust:
	@echo "==> Testing all Rust crates..."
	cargo test --workspace

# =============================================================================
# Cleaning
# =============================================================================

# Clean all build artifacts
clean:
	@echo "==> Cleaning frontend..."
	rm -rf dist
	@echo "==> Cleaning Rust targets..."
	cargo clean

# =============================================================================
# Dependencies
# =============================================================================

# Install all dependencies
install-deps:
	@echo "==> Installing pnpm dependencies..."
	pnpm install
	@echo "==> Checking Rust toolchain..."
	rustup show
	@echo ""
	@echo "Note: System dependencies must be installed manually."
	@echo "See README.md for platform-specific instructions."

# =============================================================================
# Development Helpers
# =============================================================================

# Build and run GUI app (debug)
run-app: app-debug
	@echo "==> Running flowstt-app (debug)..."
	./target/debug/flowstt-app

# Build and run GUI app (release)
run-app-release: app-release
	@echo "==> Running flowstt-app (release)..."
	./target/release/flowstt-app

# Build and run CLI (debug)
run-cli: cli-debug
	@echo "==> Running flowstt CLI (debug)..."
	./target/debug/flowstt

# Build and run CLI (release)
run-cli-release: cli-release
	@echo "==> Running flowstt CLI (release)..."
	./target/release/flowstt

# Check if all binaries exist (after build)
check-binaries:
	@echo "Checking built binaries..."
	@test -f target/release/flowstt-app && echo "  [OK] flowstt-app" || echo "  [MISSING] flowstt-app"
	@test -f target/release/flowstt && echo "  [OK] flowstt" || echo "  [MISSING] flowstt"

# =============================================================================
# Packaging
# =============================================================================

# Package the application (build installers)
package: build-release
	@echo "==> Building Tauri application package..."
	pnpm tauri build
	@echo "==> Package complete!"
	@echo "Installers available in: src-tauri/target/release/bundle/"

# Package the application (release mode with all optimizations)
package-release: package

# =============================================================================
# Help
# =============================================================================

help:
	@echo "FlowSTT Build System"
	@echo ""
	@echo "Usage: make [target]"
	@echo ""
	@echo "Build Targets:"
	@echo "  all, build       Build all components (release mode)"
	@echo "  build-debug      Build all components (debug mode, faster)"
	@echo "  build-release    Build all components (release mode)"
	@echo "  build-cuda       Build with CUDA GPU acceleration for transcription"
	@echo "  frontend         Build frontend only"
	@echo "  app              Build flowstt-app (release, includes engine)"
	@echo "  app-debug        Build flowstt-app (debug)"
	@echo "  app-cuda         Build flowstt-app with CUDA (release)"
	@echo "  cli              Build flowstt CLI (release)"
	@echo "  cli-debug        Build flowstt CLI (debug)"
	@echo ""
	@echo "Quality Targets:"
	@echo "  lint             Run all linters"
	@echo "  lint-rust        Run Rust clippy on all crates"
	@echo "  lint-ts          Run TypeScript type check"
	@echo "  test             Run all tests"
	@echo "  test-rust        Run Rust tests on all crates"
	@echo ""
	@echo "Run Targets:"
	@echo "  run-app             Build and run GUI app (debug)"
	@echo "  run-app-release     Build and run GUI app (release)"
	@echo "  run-cli             Build and run CLI (debug)"
	@echo "  run-cli-release     Build and run CLI (release)"
	@echo ""
	@echo "Utility Targets:"
	@echo "  clean            Clean all build artifacts"
	@echo "  install-deps     Install npm/pnpm dependencies"
	@echo "  check-binaries   Check if all binaries were built"
	@echo "  help             Show this help message"
	@echo ""
	@echo "Packaging Targets:"
	@echo "  package          Build all binaries and create installers"
	@echo "  package-release  Same as package (release mode)"
	@echo ""
	@echo "CUDA Acceleration:"
	@echo "  The 'cuda' targets enable GPU-accelerated transcription via whisper.cpp."
	@echo "  - Linux: Requires NVIDIA CUDA Toolkit (nvcc, cuBLAS) at build time"
	@echo "  - Windows: CUDA binaries always used (auto CPU fallback when no GPU)"
	@echo "  - macOS: No effect (Metal acceleration is always used)"
