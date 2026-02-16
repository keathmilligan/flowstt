//! Configuration persistence for FlowSTT.
//!
//! This module handles loading and saving service configuration to a JSON file
//! in the user's configuration directory. It is shared between the CLI and
//! service so both can read/write config in offline mode.

use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::types::{HotkeyCombination, KeyCode, TranscriptionMode};

/// Theme mode for the application UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Auto,
    Light,
    Dark,
}

impl Default for ThemeMode {
    fn default() -> Self {
        ThemeMode::Auto
    }
}

/// Service configuration that persists across restarts.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Current transcription mode (Automatic or PushToTalk)
    #[serde(default)]
    pub transcription_mode: TranscriptionMode,
    /// Configured push-to-talk hotkey combinations (new format)
    #[serde(default)]
    pub ptt_hotkeys: Vec<HotkeyCombination>,
    /// Whether auto-paste into the foreground application is enabled
    #[serde(default = "default_auto_paste_enabled")]
    pub auto_paste_enabled: bool,
    /// Delay in milliseconds between clipboard write and paste simulation
    #[serde(default = "default_auto_paste_delay_ms")]
    pub auto_paste_delay_ms: u32,
    /// UI theme mode: auto (follow OS), light, or dark
    #[serde(default)]
    pub theme_mode: ThemeMode,
}

fn default_auto_paste_enabled() -> bool {
    true
}

fn default_auto_paste_delay_ms() -> u32 {
    50
}

/// Legacy configuration format for backward-compatible loading.
#[derive(Debug, Deserialize)]
struct LegacyConfig {
    #[serde(default)]
    transcription_mode: TranscriptionMode,
    /// Old single-key field
    ptt_key: Option<KeyCode>,
    /// New multi-hotkey field (may be present if already migrated)
    ptt_hotkeys: Option<Vec<HotkeyCombination>>,
    /// Whether auto-paste is enabled (may be absent in old configs)
    auto_paste_enabled: Option<bool>,
    /// Auto-paste delay in ms (may be absent in old configs)
    auto_paste_delay_ms: Option<u32>,
    /// UI theme mode (may be absent in old configs)
    theme_mode: Option<ThemeMode>,
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
    /// Handles backward-compatible loading: if the file contains the legacy
    /// `ptt_key` field (single key), it is migrated to the new `ptt_hotkeys`
    /// array format automatically.
    ///
    /// Returns the loaded configuration, or a default configuration if the file
    /// doesn't exist or can't be parsed.
    pub fn load() -> Self {
        let path = Self::config_path();

        if !path.exists() {
            return Self::default_with_hotkeys();
        }

        match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str::<LegacyConfig>(&contents) {
                Ok(legacy) => Self::from_legacy(legacy),
                Err(_) => Self::default_with_hotkeys(),
            },
            Err(_) => Self::default_with_hotkeys(),
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

        Ok(())
    }

    /// Check if first-time setup is needed.
    ///
    /// Returns `true` when no config file exists on disk, indicating the user
    /// has never completed setup. This is used by the GUI to show the setup
    /// wizard on first launch.
    pub fn needs_setup() -> bool {
        !Self::config_path().exists()
    }

    /// Create a default config with the default hotkey binding.
    pub fn default_with_hotkeys() -> Self {
        Self {
            transcription_mode: TranscriptionMode::default(),
            ptt_hotkeys: vec![HotkeyCombination::single(KeyCode::default())],
            auto_paste_enabled: true,
            auto_paste_delay_ms: 50,
            theme_mode: ThemeMode::default(),
        }
    }

    /// Convert a legacy config (with optional ptt_key or ptt_hotkeys) to new format.
    fn from_legacy(legacy: LegacyConfig) -> Self {
        let ptt_hotkeys = if let Some(hotkeys) = legacy.ptt_hotkeys {
            // Already in new format
            if hotkeys.is_empty() {
                vec![HotkeyCombination::single(KeyCode::default())]
            } else {
                hotkeys
            }
        } else if let Some(key) = legacy.ptt_key {
            // Migrate from single key to single-element combination list
            vec![HotkeyCombination::single(key)]
        } else {
            // Neither field present, use default
            vec![HotkeyCombination::single(KeyCode::default())]
        };

        Self {
            transcription_mode: legacy.transcription_mode,
            ptt_hotkeys,
            auto_paste_enabled: legacy.auto_paste_enabled.unwrap_or(true),
            auto_paste_delay_ms: legacy.auto_paste_delay_ms.unwrap_or(50),
            theme_mode: legacy.theme_mode.unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default_with_hotkeys();
        assert_eq!(config.transcription_mode, TranscriptionMode::default());
        assert_eq!(config.ptt_hotkeys.len(), 1);
        assert_eq!(config.ptt_hotkeys[0].keys, vec![KeyCode::default()]);
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = Config {
            transcription_mode: TranscriptionMode::Automatic,
            ptt_hotkeys: vec![
                HotkeyCombination::single(KeyCode::F13),
                HotkeyCombination::new(vec![KeyCode::LeftControl, KeyCode::LeftAlt]),
            ],
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.transcription_mode, TranscriptionMode::Automatic);
        assert_eq!(parsed.ptt_hotkeys.len(), 2);
        assert_eq!(parsed.ptt_hotkeys[0].keys, vec![KeyCode::F13]);
    }

    #[test]
    fn test_legacy_ptt_key_migration() {
        let json = r#"{"transcription_mode": "push_to_talk", "ptt_key": "f13"}"#;
        let legacy: LegacyConfig = serde_json::from_str(json).unwrap();
        let config = Config::from_legacy(legacy);

        assert_eq!(config.transcription_mode, TranscriptionMode::PushToTalk);
        assert_eq!(config.ptt_hotkeys.len(), 1);
        assert_eq!(config.ptt_hotkeys[0].keys, vec![KeyCode::F13]);
    }

    #[test]
    fn test_legacy_missing_both_fields() {
        let json = r#"{"transcription_mode": "automatic"}"#;
        let legacy: LegacyConfig = serde_json::from_str(json).unwrap();
        let config = Config::from_legacy(legacy);

        assert_eq!(config.transcription_mode, TranscriptionMode::Automatic);
        assert_eq!(config.ptt_hotkeys.len(), 1);
        assert_eq!(config.ptt_hotkeys[0].keys, vec![KeyCode::default()]);
    }

    #[test]
    fn test_new_format_loaded_directly() {
        let json = r#"{"transcription_mode": "push_to_talk", "ptt_hotkeys": [{"keys": ["left_control", "left_alt"]}]}"#;
        let legacy: LegacyConfig = serde_json::from_str(json).unwrap();
        let config = Config::from_legacy(legacy);

        assert_eq!(config.ptt_hotkeys.len(), 1);
        assert!(config.ptt_hotkeys[0].keys.contains(&KeyCode::LeftControl));
        assert!(config.ptt_hotkeys[0].keys.contains(&KeyCode::LeftAlt));
    }
}
