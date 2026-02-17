# FlowSTT Makefile
# Build all components for testing

.PHONY: all clean build build-debug build-release build-cuda \
        frontend service service-cuda app cli \
        run-service run-service-release run-service-cuda \
        run-app run-app-release \
        run-cli run-cli-release \
        lint lint-rust lint-ts test \
        install-deps check-binaries help \
        package package-release prepare-binaries

# Default target
all: build

# Build all components in release mode
build: build-release

# Build all components in debug mode (faster compilation)
build-debug: frontend service-debug app-debug cli-debug

# Build all components in release mode
build-release: frontend service-release app-release cli-release

# Build all components with CUDA acceleration (requires NVIDIA CUDA Toolkit on Linux)
build-cuda: frontend service-cuda app-release cli-release

# =============================================================================
# Individual Components
# =============================================================================

# Build frontend (TypeScript/Vite)
frontend:
	@echo "==> Building frontend..."
	pnpm build

# Build flowstt-service (debug)
service-debug:
	@echo "==> Building flowstt-service (debug)..."
	cargo build -p flowstt-service

# Build flowstt-service (release)
service-release:
	@echo "==> Building flowstt-service (release)..."
	cargo build -p flowstt-service --release

# Build flowstt-service with CUDA acceleration (release)
# Requires: NVIDIA CUDA Toolkit (nvcc, cuBLAS) on Linux
#           On Windows, CUDA binaries are always used (this has no additional effect)
#           On macOS, this has no effect (Metal acceleration is always used)
service-cuda:
	@echo "==> Building flowstt-service with CUDA (release)..."
	cargo build -p flowstt-service --release --features cuda

# Alias for release
service: service-release

# Build flowstt-app/Tauri GUI (debug)
app-debug:
	@echo "==> Building flowstt-app (debug)..."
	cargo build -p flowstt-app

# Build flowstt-app/Tauri GUI (release)
app-release:
	@echo "==> Building flowstt-app (release)..."
	cargo build -p flowstt-app --release

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

# Rust linting (all crates)
lint-rust:
	@echo "==> Linting all Rust crates..."
	cargo clippy --workspace --all-targets --all-features -- -D warnings

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
	cargo test --workspace --all-features

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

# Build and run service in foreground (debug)
run-service: service-debug
	@echo "==> Running flowstt-service (debug)..."
	./target/debug/flowstt-service

# Build and run service in foreground (release)
run-service-release: service-release
	@echo "==> Running flowstt-service (release)..."
	./target/release/flowstt-service

# Build and run service with CUDA in foreground (release)
run-service-cuda: service-cuda
	@echo "==> Running flowstt-service with CUDA (release)..."
	./target/release/flowstt-service

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
	@test -f target/release/flowstt-service && echo "  [OK] flowstt-service" || echo "  [MISSING] flowstt-service"

# =============================================================================
# Packaging
# =============================================================================

# Prepare binaries for bundling (copy to src-tauri/binaries/)
prepare-binaries: build-release
	@echo "==> Preparing binaries for bundling..."
	@mkdir -p src-tauri/binaries
ifeq ($(OS),Windows_NT)
	@cp target/release/flowstt-service.exe src-tauri/binaries/flowstt-service.exe
	@cp target/release/flowstt.exe src-tauri/binaries/flowstt.exe
else
	@cp target/release/flowstt-service src-tauri/binaries/flowstt-service
	@cp target/release/flowstt src-tauri/binaries/flowstt
	@chmod +x src-tauri/binaries/flowstt-service src-tauri/binaries/flowstt
endif
	@echo "==> Binaries prepared in src-tauri/binaries/"

# Package the application (build installers)
package: prepare-binaries
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
	@echo "  service          Build flowstt-service (release)"
	@echo "  service-debug    Build flowstt-service (debug)"
	@echo "  service-cuda     Build flowstt-service with CUDA (release)"
	@echo "  app              Build flowstt-app GUI (release)"
	@echo "  app-debug        Build flowstt-app GUI (debug)"
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
	@echo "  run-service         Build and run service (debug)"
	@echo "  run-service-release Build and run service (release)"
	@echo "  run-service-cuda    Build and run service with CUDA (release)"
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
