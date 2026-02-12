//! Configuration persistence for FlowSTT service.
//!
//! This module handles loading and saving service configuration to a JSON file
//! in the user's configuration directory.

use directories::BaseDirs;
use flowstt_common::{KeyCode, TranscriptionMode};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;
use tracing::{info, warn};

/// Service configuration that persists across restarts.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Current transcription mode (Automatic or PushToTalk)
    #[serde(default)]
    pub transcription_mode: TranscriptionMode,
    /// Configured push-to-talk hotkey
    #[serde(default)]
    pub ptt_key: KeyCode,
}

impl Config {
    /// Get the path to the configuration file.
    ///
    /// Returns platform-specific path:
    /// - Linux: ~/.config/flowstt/config.json
    /// - macOS: ~/Library/Application Support/flowstt/config.json
    /// - Windows: %APPDATA%\flowstt\config.json
    pub fn config_path() -> PathBuf {
        BaseDirs::new()
            .map(|d| d.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
            .join("flowstt")
            .join("config.json")
    }

    /// Load configuration from disk.
    ///
    /// Returns the loaded configuration, or a default configuration if the file
    /// doesn't exist or can't be parsed. Errors are logged but don't fail the load.
    pub fn load() -> Self {
        let path = Self::config_path();

        if !path.exists() {
            info!("No config file found at {:?}, using defaults", path);
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(config) => {
                    info!("Loaded config from {:?}", path);
                    config
                }
                Err(e) => {
                    warn!(
                        "Failed to parse config file {:?}: {}, using defaults",
                        path, e
                    );
                    Self::default()
                }
            },
            Err(e) => {
                warn!(
                    "Failed to read config file {:?}: {}, using defaults",
                    path, e
                );
                Self::default()
            }
        }
    }

    /// Save configuration to disk.
    ///
    /// Creates the configuration directory if it doesn't exist.
    pub fn save(&self) -> io::Result<()> {
        let path = Self::config_path();

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&path, contents)?;

        info!("Saved config to {:?}", path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.transcription_mode, TranscriptionMode::default());
        assert_eq!(config.ptt_key, KeyCode::default());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            transcription_mode: TranscriptionMode::Automatic,
            ptt_key: KeyCode::F13,
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.transcription_mode, TranscriptionMode::Automatic);
        assert_eq!(parsed.ptt_key, KeyCode::F13);
    }
}
