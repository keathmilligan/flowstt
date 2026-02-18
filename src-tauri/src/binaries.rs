//! Binary path resolution for bundled service and CLI binaries.
//!
//! When FlowSTT is installed from a distribution package, the service and CLI
//! binaries are bundled as resources in the app bundle. This module provides
//! paths to these binaries without extracting them - they run directly from
//! the bundle, which means they're removed when the app is uninstalled.
//!
//! Native libraries (whisper, ggml) are also bundled alongside binaries
//! so the service can find them at runtime.

use std::io;
use std::path::PathBuf;
use tauri::Manager;

/// Names of the bundled binaries (without platform-specific extension)
const SERVICE_BINARY_NAME: &str = "flowstt-service";
const CLI_BINARY_NAME: &str = "flowstt";

/// Get the binary name with platform-specific extension
#[cfg(windows)]
fn with_exe_suffix(name: &str) -> String {
    format!("{}.exe", name)
}

#[cfg(not(windows))]
fn with_exe_suffix(name: &str) -> String {
    name.to_string()
}

/// Get the path to bundled binaries directory in the app bundle.
/// Returns None if running in development mode (no bundle).
pub fn bundle_binaries_dir(app_handle: &tauri::AppHandle) -> Option<PathBuf> {
    let resource_path = app_handle.path().resource_dir().ok()?;
    let binaries_dir = resource_path.join("binaries");
    if binaries_dir.exists() {
        Some(binaries_dir)
    } else {
        None
    }
}

/// Get the path to the service binary.
/// Checks bundle first, then falls back to development path.
pub fn get_service_path(app_handle: &tauri::AppHandle) -> PathBuf {
    let platform_name = with_exe_suffix(SERVICE_BINARY_NAME);

    // First, check bundle resources (installed app)
    if let Some(bundle_dir) = bundle_binaries_dir(app_handle) {
        let service_path = bundle_dir.join(&platform_name);
        if service_path.exists() {
            return service_path;
        }
    }

    // Fall back to development path (next to GUI binary)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let service_path = dir.join(&platform_name);
            if service_path.exists() {
                return service_path;
            }
        }
    }

    // Last resort: assume on PATH
    PathBuf::from(platform_name)
}

/// Get the path to the CLI binary.
/// Checks bundle first, then falls back to development path.
#[allow(dead_code)]
pub fn get_cli_path(app_handle: &tauri::AppHandle) -> PathBuf {
    let platform_name = with_exe_suffix(CLI_BINARY_NAME);

    if let Some(bundle_dir) = bundle_binaries_dir(app_handle) {
        let cli_path = bundle_dir.join(&platform_name);
        if cli_path.exists() {
            return cli_path;
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let cli_path = dir.join(&platform_name);
            if cli_path.exists() {
                return cli_path;
            }
        }
    }

    PathBuf::from(platform_name)
}

/// Paths to bundled binaries.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct BundlePaths {
    /// Path to the flowstt-service binary
    pub service: PathBuf,
    /// Path to the flowstt CLI binary
    pub cli: PathBuf,
}

/// Error when binaries are not found.
#[derive(Debug)]
pub struct BinariesNotFoundError {
    pub message: String,
}

impl std::fmt::Display for BinariesNotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BinariesNotFoundError {}

/// Get paths to bundled binaries.
/// Returns an error if binaries are not found (neither in bundle nor development).
pub fn get_bundle_paths(
    app_handle: &tauri::AppHandle,
) -> Result<BundlePaths, BinariesNotFoundError> {
    let service_path = get_service_path(app_handle);
    let cli_path = get_cli_path(app_handle);

    if !service_path.exists() {
        return Err(BinariesNotFoundError {
            message: format!("Service binary not found: {:?}", service_path),
        });
    }

    if !cli_path.exists() {
        return Err(BinariesNotFoundError {
            message: format!("CLI binary not found: {:?}", cli_path),
        });
    }

    Ok(BundlePaths {
        service: service_path,
        cli: cli_path,
    })
}

/// Check if running from installed app bundle (vs development).
#[allow(dead_code)]
pub fn is_bundled(app_handle: &tauri::AppHandle) -> bool {
    bundle_binaries_dir(app_handle).is_some()
}
