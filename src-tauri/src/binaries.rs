//! Binary extraction and management for bundled service and CLI binaries.
//!
//! When FlowSTT is installed from a distribution package, the service and CLI
//! binaries are bundled as resources. This module handles extracting them to
//! the application support directory on first launch and providing their paths
//! at runtime.

use std::fs;
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

/// Get the directory where extracted binaries are stored.
pub fn binaries_dir() -> PathBuf {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("FlowSTT");
    data_dir.join("bin")
}

/// Ensure the binaries directory exists.
fn ensure_binaries_dir() -> io::Result<PathBuf> {
    let dir = binaries_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

/// Extract a bundled binary to the application support directory.
/// Returns the path to the extracted binary.
fn extract_binary(app_handle: &tauri::AppHandle, binary_name: &str) -> io::Result<PathBuf> {
    let dest_dir = ensure_binaries_dir()?;
    let platform_binary_name = with_exe_suffix(binary_name);
    let dest_path = dest_dir.join(&platform_binary_name);

    // Check if already extracted
    if dest_path.exists() {
        return Ok(dest_path);
    }

    // Try to get the resource path
    let resource_path = app_handle
        .path()
        .resource_dir()
        .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e))?;

    let src_path = resource_path.join("binaries").join(&platform_binary_name);

    if !src_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Binary not found in bundle: {}", platform_binary_name),
        ));
    }

    // Copy the binary
    fs::copy(&src_path, &dest_path)?;

    // Set executable permission on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dest_path, fs::Permissions::from_mode(0o755))?;
    }

    Ok(dest_path)
}

/// Extract all bundled binaries if not already extracted.
/// Returns paths to the service and CLI binaries.
pub fn extract_all_binaries(
    app_handle: &tauri::AppHandle,
) -> Result<ExtractedBinaries, ExtractionError> {
    let service_path =
        extract_binary(app_handle, SERVICE_BINARY_NAME).map_err(|e| ExtractionError {
            binary: SERVICE_BINARY_NAME.to_string(),
            source: e,
        })?;

    let cli_path = extract_binary(app_handle, CLI_BINARY_NAME).map_err(|e| ExtractionError {
        binary: CLI_BINARY_NAME.to_string(),
        source: e,
    })?;

    Ok(ExtractedBinaries {
        service: service_path,
        cli: cli_path,
    })
}

/// Paths to extracted binaries.
#[derive(Clone, Debug)]
pub struct ExtractedBinaries {
    /// Path to the flowstt-service binary
    pub service: PathBuf,
    /// Path to the flowstt CLI binary
    pub cli: PathBuf,
}

/// Error during binary extraction.
#[derive(Debug)]
pub struct ExtractionError {
    /// Name of the binary that failed to extract
    pub binary: String,
    /// The underlying I/O error
    pub source: io::Error,
}

impl std::fmt::Display for ExtractionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Failed to extract binary '{}': {}",
            self.binary, self.source
        )
    }
}

impl std::error::Error for ExtractionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Check if binaries are bundled (running from installed app) or development.
/// Returns true if running from an installed application.
#[allow(dead_code)]
pub fn is_bundled() -> bool {
    // In development, binaries won't be in the resources directory
    // We check if the binaries directory exists in resources
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
        .map(|dir| {
            dir.join("binaries")
                .join(with_exe_suffix(SERVICE_BINARY_NAME))
                .exists()
        })
        .unwrap_or(false)
}
