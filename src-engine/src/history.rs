//! Persistent transcription history management.
//!
//! Stores transcription results with metadata in a JSON file alongside
//! cached WAV recordings in the OS-standard application data directory.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{info, warn};

/// A single entry in the transcription history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Unique identifier for this entry
    pub id: String,
    /// Transcribed text
    pub text: String,
    /// ISO 8601 timestamp of when the transcription occurred
    pub timestamp: String,
    /// Path to the cached WAV file, if it still exists
    pub wav_path: Option<String>,
}

/// Manages persistent transcription history.
pub struct TranscriptionHistory {
    /// Path to the history JSON file
    history_path: PathBuf,
    /// In-memory history entries
    entries: Vec<HistoryEntry>,
}

impl TranscriptionHistory {
    /// Get the application data directory for FlowSTT.
    pub fn data_dir() -> PathBuf {
        directories::ProjectDirs::from("", "", "flowstt")
            .map(|dirs| dirs.data_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".").join("flowstt-data"))
    }

    /// Get the recordings subdirectory within the data directory.
    pub fn recordings_dir() -> PathBuf {
        Self::data_dir().join("recordings")
    }

    /// Load history from disk, creating directories as needed.
    pub fn load() -> Self {
        let data_dir = Self::data_dir();
        if let Err(e) = fs::create_dir_all(&data_dir) {
            warn!("Failed to create data directory {:?}: {}", data_dir, e);
        }

        let history_path = data_dir.join("history.json");
        let entries = if history_path.exists() {
            match fs::read_to_string(&history_path) {
                Ok(content) => match serde_json::from_str::<Vec<HistoryEntry>>(&content) {
                    Ok(entries) => {
                        info!(
                            "Loaded {} history entries from {:?}",
                            entries.len(),
                            history_path
                        );
                        entries
                    }
                    Err(e) => {
                        warn!(
                            "Corrupted history file, backing up and starting fresh: {}",
                            e
                        );
                        // Backup corrupted file
                        let backup_path = data_dir.join("history.json.bak");
                        let _ = fs::rename(&history_path, &backup_path);
                        Vec::new()
                    }
                },
                Err(e) => {
                    warn!("Failed to read history file: {}", e);
                    Vec::new()
                }
            }
        } else {
            info!("No history file found, starting fresh");
            Vec::new()
        };

        Self {
            history_path,
            entries,
        }
    }

    /// Save history to disk.
    pub fn save(&self) -> Result<(), String> {
        let content = serde_json::to_string_pretty(&self.entries)
            .map_err(|e| format!("Failed to serialize history: {}", e))?;
        fs::write(&self.history_path, content)
            .map_err(|e| format!("Failed to write history file: {}", e))?;
        Ok(())
    }

    /// Add a new entry to the history and save.
    pub fn add_entry(&mut self, text: String, wav_path: Option<String>) -> HistoryEntry {
        let entry = HistoryEntry {
            id: generate_id(),
            text,
            timestamp: Utc::now().to_rfc3339(),
            wav_path,
        };
        self.entries.push(entry.clone());
        if let Err(e) = self.save() {
            warn!("Failed to save history after adding entry: {}", e);
        }
        entry
    }

    /// Delete an entry by ID. Returns true if found and deleted.
    /// Also deletes the associated WAV file if present.
    pub fn delete_entry(&mut self, id: &str) -> bool {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            let entry = self.entries.remove(pos);
            // Delete WAV file if it exists
            if let Some(ref wav_path) = entry.wav_path {
                let path = Path::new(wav_path);
                if path.exists() {
                    if let Err(e) = fs::remove_file(path) {
                        warn!("Failed to delete WAV file {:?}: {}", path, e);
                    } else {
                        info!("Deleted WAV file: {:?}", path);
                    }
                }
            }
            if let Err(e) = self.save() {
                warn!("Failed to save history after deleting entry: {}", e);
            }
            true
        } else {
            false
        }
    }

    /// Get all history entries.
    pub fn get_entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Clean up WAV files older than the specified duration.
    /// Sets wav_path to None for affected entries but preserves the text.
    pub fn cleanup_wav_files(&mut self, max_age: Duration) {
        let recordings_dir = Self::recordings_dir();
        if !recordings_dir.exists() {
            return;
        }

        let now = std::time::SystemTime::now();
        let mut cleaned = 0u32;

        // Scan recordings directory for old files
        if let Ok(entries) = fs::read_dir(&recordings_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "wav") {
                    if let Ok(metadata) = path.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(age) = now.duration_since(modified) {
                                if age > max_age {
                                    if let Err(e) = fs::remove_file(&path) {
                                        warn!("Failed to delete old WAV file {:?}: {}", path, e);
                                    } else {
                                        cleaned += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if cleaned > 0 {
            info!("Cleaned up {} old WAV file(s)", cleaned);

            // Nullify wav_path references for deleted files
            for entry in &mut self.entries {
                if let Some(ref wav_path) = entry.wav_path {
                    if !Path::new(wav_path).exists() {
                        entry.wav_path = None;
                    }
                }
            }

            if let Err(e) = self.save() {
                warn!("Failed to save history after WAV cleanup: {}", e);
            }
        }
    }
}

/// Generate a unique ID for a history entry.
fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    // Simple ID: timestamp + random suffix
    let random: u32 = rand_u32();
    format!("{}-{:08x}", timestamp, random)
}

/// Simple pseudo-random u32 without external dependency.
fn rand_u32() -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut hasher = DefaultHasher::new();
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    hasher.finish() as u32
}

// ============================================================================
// Global History State
// ============================================================================

/// Global shared history instance.
static HISTORY: std::sync::OnceLock<Arc<Mutex<TranscriptionHistory>>> = std::sync::OnceLock::new();

/// Get or initialize the global history instance.
pub fn get_history() -> Arc<Mutex<TranscriptionHistory>> {
    HISTORY
        .get_or_init(|| Arc::new(Mutex::new(TranscriptionHistory::load())))
        .clone()
}
