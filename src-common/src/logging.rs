//! Platform-specific logging directory resolution.

use std::path::PathBuf;

/// Returns the platform-appropriate directory for log files.
///
/// | Platform | Directory |
/// |----------|-----------|
/// | Linux | `$XDG_STATE_HOME/flowstt/logs` or `~/.local/state/flowstt/logs` |
/// | macOS | `~/Library/Logs/flowstt` |
/// | Windows | `%APPDATA%/flowstt/logs` |
pub fn log_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let base = directories::ProjectDirs::from("io", "flowstt", "flowstt")
            .expect("Failed to determine project directories");
        base.state_dir()
            .unwrap_or_else(|| base.data_local_dir().join("state"))
            .join("logs")
    }

    #[cfg(target_os = "macos")]
    {
        // Use standard macOS log location: ~/Library/Logs/<app>/
        dirs::home_dir()
            .expect("Failed to determine home directory")
            .join("Library")
            .join("Logs")
            .join("flowstt")
    }

    #[cfg(target_os = "windows")]
    {
        let base = directories::ProjectDirs::from("io", "flowstt", "flowstt")
            .expect("Failed to determine project directories");
        base.data_local_dir().join("logs")
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let base = directories::ProjectDirs::from("io", "flowstt", "flowstt")
            .expect("Failed to determine project directories");
        base.data_local_dir().join("logs")
    }
}

/// Ensures the log directory exists, creating it if necessary.
///
/// Returns `Ok(())` if the directory exists or was created.
/// Returns `Err` if the directory could not be created.
pub fn ensure_log_dir() -> Result<(), std::io::Error> {
    let dir = log_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(())
}

/// Returns the path to the application log file.
pub fn app_log_path() -> PathBuf {
    log_dir().join("flowstt-app.log")
}
