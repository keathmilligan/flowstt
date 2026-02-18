//! Shared types for FlowSTT audio capture and transcription.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};

/// Audio source type for capture.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioSourceType {
    /// Microphone or other input device
    #[default]
    Input,
    /// System audio (monitor/loopback)
    System,
    /// Mixed input and system audio
    Mixed,
}

/// Recording mode - determines how multiple audio sources are combined.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingMode {
    /// Mix both streams together (default behavior)
    #[default]
    Mixed,
    /// Echo cancellation mode - output only echo-cancelled primary source
    EchoCancel,
}

/// Transcription mode - determines how speech segment boundaries are identified.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionMode {
    /// VAD-triggered - speech detection determines segment boundaries
    Automatic,
    /// Manual key-controlled - hotkey press/release determines segment boundaries
    #[default]
    PushToTalk,
}

/// Runtime mode - determines behavior for service lifecycle management.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    /// Development mode - service persists independently for debugging
    Development,
    /// Production mode - service lifecycle coupled to owner client
    #[default]
    Production,
}

impl RuntimeMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuntimeMode::Development => "development",
            RuntimeMode::Production => "production",
        }
    }
}

/// Platform-independent key codes for push-to-talk hotkey configuration.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyCode {
    // === Modifier Keys ===
    /// Right Alt/Option key (default on macOS)
    #[default]
    RightAlt,
    /// Left Alt/Option key
    LeftAlt,
    /// Right Control key
    RightControl,
    /// Left Control key
    LeftControl,
    /// Right Shift key
    RightShift,
    /// Left Shift key
    LeftShift,
    /// Caps Lock key
    CapsLock,
    /// Left Meta/Windows/Command key
    LeftMeta,
    /// Right Meta/Windows/Command key
    RightMeta,

    // === Function Keys ===
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,

    // === Letter Keys ===
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,

    // === Digit Keys ===
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,

    // === Navigation Keys ===
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,

    // === Special Keys ===
    Escape,
    Tab,
    Space,
    Enter,
    Backspace,
    PrintScreen,
    ScrollLock,
    Pause,

    // === Punctuation / Symbol Keys ===
    /// - / _ key
    Minus,
    /// = / + key
    Equal,
    /// [ / { key
    BracketLeft,
    /// ] / } key
    BracketRight,
    /// \ / | key
    Backslash,
    /// ; / : key
    Semicolon,
    /// ' / " key
    Quote,
    /// ` / ~ key
    Backquote,
    /// , / < key
    Comma,
    /// . / > key
    Period,
    /// / / ? key
    Slash,

    // === Numpad Keys ===
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadMultiply,
    NumpadAdd,
    NumpadSubtract,
    NumpadDecimal,
    NumpadDivide,
    NumLock,
}

impl KeyCode {
    /// Get a human-readable display name for the key.
    pub fn display_name(&self) -> &'static str {
        match self {
            // Modifiers
            KeyCode::RightAlt => "Right Alt",
            KeyCode::LeftAlt => "Left Alt",
            KeyCode::RightControl => "Right Ctrl",
            KeyCode::LeftControl => "Left Ctrl",
            KeyCode::RightShift => "Right Shift",
            KeyCode::LeftShift => "Left Shift",
            KeyCode::CapsLock => "Caps Lock",
            KeyCode::LeftMeta => "Left Win",
            KeyCode::RightMeta => "Right Win",
            // Function keys
            KeyCode::F1 => "F1",
            KeyCode::F2 => "F2",
            KeyCode::F3 => "F3",
            KeyCode::F4 => "F4",
            KeyCode::F5 => "F5",
            KeyCode::F6 => "F6",
            KeyCode::F7 => "F7",
            KeyCode::F8 => "F8",
            KeyCode::F9 => "F9",
            KeyCode::F10 => "F10",
            KeyCode::F11 => "F11",
            KeyCode::F12 => "F12",
            KeyCode::F13 => "F13",
            KeyCode::F14 => "F14",
            KeyCode::F15 => "F15",
            KeyCode::F16 => "F16",
            KeyCode::F17 => "F17",
            KeyCode::F18 => "F18",
            KeyCode::F19 => "F19",
            KeyCode::F20 => "F20",
            KeyCode::F21 => "F21",
            KeyCode::F22 => "F22",
            KeyCode::F23 => "F23",
            KeyCode::F24 => "F24",
            // Letters
            KeyCode::KeyA => "A",
            KeyCode::KeyB => "B",
            KeyCode::KeyC => "C",
            KeyCode::KeyD => "D",
            KeyCode::KeyE => "E",
            KeyCode::KeyF => "F",
            KeyCode::KeyG => "G",
            KeyCode::KeyH => "H",
            KeyCode::KeyI => "I",
            KeyCode::KeyJ => "J",
            KeyCode::KeyK => "K",
            KeyCode::KeyL => "L",
            KeyCode::KeyM => "M",
            KeyCode::KeyN => "N",
            KeyCode::KeyO => "O",
            KeyCode::KeyP => "P",
            KeyCode::KeyQ => "Q",
            KeyCode::KeyR => "R",
            KeyCode::KeyS => "S",
            KeyCode::KeyT => "T",
            KeyCode::KeyU => "U",
            KeyCode::KeyV => "V",
            KeyCode::KeyW => "W",
            KeyCode::KeyX => "X",
            KeyCode::KeyY => "Y",
            KeyCode::KeyZ => "Z",
            // Digits
            KeyCode::Digit0 => "0",
            KeyCode::Digit1 => "1",
            KeyCode::Digit2 => "2",
            KeyCode::Digit3 => "3",
            KeyCode::Digit4 => "4",
            KeyCode::Digit5 => "5",
            KeyCode::Digit6 => "6",
            KeyCode::Digit7 => "7",
            KeyCode::Digit8 => "8",
            KeyCode::Digit9 => "9",
            // Navigation
            KeyCode::ArrowUp => "Up",
            KeyCode::ArrowDown => "Down",
            KeyCode::ArrowLeft => "Left",
            KeyCode::ArrowRight => "Right",
            KeyCode::Home => "Home",
            KeyCode::End => "End",
            KeyCode::PageUp => "Page Up",
            KeyCode::PageDown => "Page Down",
            KeyCode::Insert => "Insert",
            KeyCode::Delete => "Delete",
            // Special
            KeyCode::Escape => "Esc",
            KeyCode::Tab => "Tab",
            KeyCode::Space => "Space",
            KeyCode::Enter => "Enter",
            KeyCode::Backspace => "Backspace",
            KeyCode::PrintScreen => "Print Screen",
            KeyCode::ScrollLock => "Scroll Lock",
            KeyCode::Pause => "Pause",
            // Punctuation
            KeyCode::Minus => "-",
            KeyCode::Equal => "=",
            KeyCode::BracketLeft => "[",
            KeyCode::BracketRight => "]",
            KeyCode::Backslash => "\\",
            KeyCode::Semicolon => ";",
            KeyCode::Quote => "'",
            KeyCode::Backquote => "`",
            KeyCode::Comma => ",",
            KeyCode::Period => ".",
            KeyCode::Slash => "/",
            // Numpad
            KeyCode::Numpad0 => "Num 0",
            KeyCode::Numpad1 => "Num 1",
            KeyCode::Numpad2 => "Num 2",
            KeyCode::Numpad3 => "Num 3",
            KeyCode::Numpad4 => "Num 4",
            KeyCode::Numpad5 => "Num 5",
            KeyCode::Numpad6 => "Num 6",
            KeyCode::Numpad7 => "Num 7",
            KeyCode::Numpad8 => "Num 8",
            KeyCode::Numpad9 => "Num 9",
            KeyCode::NumpadMultiply => "Num *",
            KeyCode::NumpadAdd => "Num +",
            KeyCode::NumpadSubtract => "Num -",
            KeyCode::NumpadDecimal => "Num .",
            KeyCode::NumpadDivide => "Num /",
            KeyCode::NumLock => "Num Lock",
        }
    }

    /// Whether this key is a modifier key (used for display ordering).
    pub fn is_modifier(&self) -> bool {
        matches!(
            self,
            KeyCode::LeftControl
                | KeyCode::RightControl
                | KeyCode::LeftAlt
                | KeyCode::RightAlt
                | KeyCode::LeftShift
                | KeyCode::RightShift
                | KeyCode::LeftMeta
                | KeyCode::RightMeta
        )
    }
}

/// A set of keys that must all be held simultaneously to trigger PTT.
///
/// Order of keys does not matter for equality -- two combinations with the same
/// keys in different order are considered equal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyCombination {
    /// One or more keys that must be held together.
    pub keys: Vec<KeyCode>,
}

impl HotkeyCombination {
    /// Create a new combination from a list of keys.
    /// Duplicates are removed and keys are sorted for consistent representation.
    pub fn new(keys: Vec<KeyCode>) -> Self {
        let mut unique: Vec<KeyCode> = keys
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        unique.sort_by_key(|k| format!("{:?}", k));
        Self { keys: unique }
    }

    /// Create a single-key combination (backward compat convenience).
    pub fn single(key: KeyCode) -> Self {
        Self { keys: vec![key] }
    }

    /// Check whether all keys in this combination are currently held.
    pub fn is_subset_of(&self, pressed: &HashSet<KeyCode>) -> bool {
        self.keys.iter().all(|k| pressed.contains(k))
    }

    /// Display the combination in human-readable format.
    /// Modifiers are listed first, then other keys, joined by " + ".
    pub fn display(&self) -> String {
        let mut modifiers: Vec<&KeyCode> = Vec::new();
        let mut others: Vec<&KeyCode> = Vec::new();
        for k in &self.keys {
            if k.is_modifier() {
                modifiers.push(k);
            } else {
                others.push(k);
            }
        }
        modifiers.sort_by_key(|k| format!("{:?}", k));
        others.sort_by_key(|k| format!("{:?}", k));

        let all: Vec<&str> = modifiers
            .iter()
            .chain(others.iter())
            .map(|k| k.display_name())
            .collect();
        all.join(" + ")
    }
}

impl PartialEq for HotkeyCombination {
    fn eq(&self, other: &Self) -> bool {
        let a: HashSet<_> = self.keys.iter().collect();
        let b: HashSet<_> = other.keys.iter().collect();
        a == b
    }
}

impl Eq for HotkeyCombination {}

impl Hash for HotkeyCombination {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash order-independently by sorting first
        let mut sorted: Vec<_> = self.keys.clone();
        sorted.sort_by_key(|k| format!("{:?}", k));
        sorted.hash(state);
    }
}

impl fmt::Display for HotkeyCombination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

impl Default for HotkeyCombination {
    fn default() -> Self {
        Self::single(KeyCode::default())
    }
}

/// Persisted configuration values returned by the GetConfig IPC request.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ConfigValues {
    /// Current transcription mode (Automatic or PushToTalk)
    pub transcription_mode: TranscriptionMode,
    /// Configured push-to-talk hotkey combinations
    pub ptt_hotkeys: Vec<HotkeyCombination>,
    /// Configured auto-mode toggle hotkeys
    #[serde(default)]
    pub auto_toggle_hotkeys: Vec<HotkeyCombination>,
    /// Whether auto-paste into the foreground application is enabled
    #[serde(default = "default_auto_paste_enabled")]
    pub auto_paste_enabled: bool,
    /// Delay in milliseconds between clipboard write and paste simulation
    #[serde(default = "default_auto_paste_delay_ms")]
    pub auto_paste_delay_ms: u32,
}

fn default_auto_paste_enabled() -> bool {
    true
}

fn default_auto_paste_delay_ms() -> u32 {
    50
}

/// Push-to-talk status information.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PttStatus {
    /// Current transcription mode
    pub mode: TranscriptionMode,
    /// Configured PTT hotkey combinations
    pub hotkeys: Vec<HotkeyCombination>,
    /// Configured auto-mode toggle hotkeys
    #[serde(default)]
    pub auto_toggle_hotkeys: Vec<HotkeyCombination>,
    /// Whether auto mode is currently active
    #[serde(default)]
    pub auto_mode_active: bool,
    /// Whether PTT key is currently pressed
    pub is_active: bool,
    /// Whether PTT is available on this platform
    pub available: bool,
    /// Error message if PTT is unavailable (e.g., missing permissions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Whether macOS Accessibility permission is currently granted.
    /// Always true on non-macOS platforms (permission not applicable).
    #[serde(default = "default_true")]
    pub accessibility_permission_granted: bool,
}

fn default_true() -> bool {
    true
}

/// Information about an audio device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    /// Unique identifier (PipeWire node ID, WASAPI endpoint ID, etc.)
    pub id: String,
    /// Display name for UI
    pub name: String,
    /// Type of audio source
    #[serde(default)]
    pub source_type: AudioSourceType,
}

/// Status of the transcription system.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranscribeStatus {
    /// Whether audio capture is running (sources configured and valid)
    pub capturing: bool,
    /// Whether currently capturing speech
    pub in_speech: bool,
    /// Number of segments waiting to be transcribed
    pub queue_depth: usize,
    /// Error message if capture failed (e.g., invalid source)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Currently configured primary audio source ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source1_id: Option<String>,
    /// Currently configured secondary audio source ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source2_id: Option<String>,
    /// Current transcription mode
    pub transcription_mode: TranscriptionMode,
}

/// Status of the Whisper model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    /// Whether the model file exists and is available
    pub available: bool,
    /// Path to the model file
    pub path: String,
}

/// CUDA/GPU acceleration status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CudaStatus {
    /// Whether the binary was built with CUDA support
    pub build_enabled: bool,
    /// Whether CUDA is available at runtime
    pub runtime_available: bool,
    /// System info string from whisper.cpp
    pub system_info: String,
}

/// A single column of spectrogram data ready for rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrogramColumn {
    /// RGB triplets for each pixel row (height * 3 bytes)
    pub colors: Vec<u8>,
}

/// Visualization data for real-time audio display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationData {
    /// Waveform amplitude values (downsampled for display)
    pub waveform: Vec<f32>,
    /// Spectrogram column (RGB color values, if ready)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spectrogram: Option<SpectrogramColumn>,
    /// Speech detection metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speech_metrics: Option<SpeechMetrics>,
}

/// Speech detection metrics for visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechMetrics {
    /// RMS amplitude in dB
    pub amplitude_db: f32,
    /// Zero-crossing rate (0.0-1.0)
    pub zcr: f32,
    /// Spectral centroid in Hz
    pub centroid_hz: f32,
    /// Whether speech is currently detected
    pub is_speaking: bool,
    /// Whether voiced onset is pending
    pub voiced_onset_pending: bool,
    /// Whether whisper onset is pending
    pub whisper_onset_pending: bool,
    /// Whether a transient was detected
    pub is_transient: bool,
    /// Whether this is lookback-determined speech
    pub is_lookback_speech: bool,
    /// Whether this is a word break
    pub is_word_break: bool,
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wav_path: Option<String>,
}

/// Transcription result for a speech segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    /// Unique history entry ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Transcribed text
    pub text: String,
    /// ISO 8601 timestamp of the transcription
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    /// Path to the saved audio file (if saved)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_path: Option<String>,
}
