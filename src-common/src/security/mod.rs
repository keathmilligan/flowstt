//! Security modules for IPC authentication and validation.

pub mod peer_verify;

/// Executable names permitted to connect to the IPC server.
pub const TRUSTED_EXECUTABLES: &[&str] = &["flowstt", "flowstt-app"];

/// Trusted installation directories (Linux).
#[cfg(target_os = "linux")]
pub const TRUSTED_DIRECTORIES: &[&str] = &["/usr/bin", "/usr/local/bin", "/opt/flowstt/bin"];

/// Trusted installation directories (macOS).
#[cfg(target_os = "macos")]
pub const TRUSTED_DIRECTORIES: &[&str] = &[
    "/Applications/FlowSTT.app/Contents/MacOS",
    "/usr/local/bin",
    "/opt/homebrew/bin",
];

/// Trusted installation directories (Windows).
#[cfg(target_os = "windows")]
pub const TRUSTED_DIRECTORIES: &[&str] = &[
    r"C:\Program Files\FlowSTT",
    r"C:\Program Files (x86)\FlowSTT",
];
