//! FFI bindings to whisper.cpp for Windows and macOS.
//! This module uses libloading to dynamically load the whisper shared library at runtime.

use libloading::Library;
use std::ffi::{c_char, c_float, c_int, CStr, CString};
use std::path::Path;
use std::sync::OnceLock;

/// Opaque pointer to whisper_context
type WhisperContext = *mut std::ffi::c_void;

/// Callback types (function pointers, nullable)
type WhisperNewSegmentCallback = *const std::ffi::c_void;
type WhisperProgressCallback = *const std::ffi::c_void;
type WhisperEncoderBeginCallback = *const std::ffi::c_void;
type WhisperAbortCallback = *const std::ffi::c_void;
type WhisperLogitsFilterCallback = *const std::ffi::c_void;
type WhisperGrammarElement = *const std::ffi::c_void;

/// VAD parameters struct
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WhisperVadParams {
    pub threshold: c_float,
    pub min_speech_duration_ms: c_int,
    pub min_silence_duration_ms: c_int,
    pub max_speech_duration_s: c_float,
    pub speech_pad_ms: c_int,
    pub samples_overlap: c_float,
}

/// whisper_full_params matching the C struct layout from whisper.h
/// IMPORTANT: This must match the exact layout of whisper_full_params in whisper.cpp
#[repr(C)]
#[derive(Clone)]
pub struct WhisperFullParams {
    pub strategy: c_int, // enum whisper_sampling_strategy

    pub n_threads: c_int,
    pub n_max_text_ctx: c_int,
    pub offset_ms: c_int,
    pub duration_ms: c_int,

    pub translate: bool,
    pub no_context: bool,
    pub no_timestamps: bool,
    pub single_segment: bool,
    pub print_special: bool,
    pub print_progress: bool,
    pub print_realtime: bool,
    pub print_timestamps: bool,

    // Token-level timestamps
    pub token_timestamps: bool,
    pub thold_pt: c_float,
    pub thold_ptsum: c_float,
    pub max_len: c_int,
    pub split_on_word: bool,
    pub max_tokens: c_int,

    // Speed-up techniques
    pub debug_mode: bool,
    pub audio_ctx: c_int,

    // Tinydiarize
    pub tdrz_enable: bool,

    // Suppress regex
    pub suppress_regex: *const c_char,

    // Initial prompt
    pub initial_prompt: *const c_char,
    pub carry_initial_prompt: bool,
    pub prompt_tokens: *const c_int,
    pub prompt_n_tokens: c_int,

    // Language
    pub language: *const c_char,
    pub detect_language: bool,

    // Decoding parameters
    pub suppress_blank: bool,
    pub suppress_nst: bool,

    pub temperature: c_float,
    pub max_initial_ts: c_float,
    pub length_penalty: c_float,

    // Fallback parameters
    pub temperature_inc: c_float,
    pub entropy_thold: c_float,
    pub logprob_thold: c_float,
    pub no_speech_thold: c_float,

    // Greedy params
    pub greedy_best_of: c_int,

    // Beam search params
    pub beam_search_beam_size: c_int,
    pub beam_search_patience: c_float,

    // Callbacks
    pub new_segment_callback: WhisperNewSegmentCallback,
    pub new_segment_callback_user_data: *mut std::ffi::c_void,

    pub progress_callback: WhisperProgressCallback,
    pub progress_callback_user_data: *mut std::ffi::c_void,

    pub encoder_begin_callback: WhisperEncoderBeginCallback,
    pub encoder_begin_callback_user_data: *mut std::ffi::c_void,

    pub abort_callback: WhisperAbortCallback,
    pub abort_callback_user_data: *mut std::ffi::c_void,

    pub logits_filter_callback: WhisperLogitsFilterCallback,
    pub logits_filter_callback_user_data: *mut std::ffi::c_void,

    // Grammar
    pub grammar_rules: *const WhisperGrammarElement,
    pub n_grammar_rules: usize,
    pub i_start_rule: usize,
    pub grammar_penalty: c_float,

    // VAD
    pub vad: bool,
    pub vad_model_path: *const c_char,
    pub vad_params: WhisperVadParams,
}

impl WhisperFullParams {
    /// Configure parameters optimized for short audio segments (real-time transcription)
    /// - n_samples: total samples in the (possibly padded) audio buffer
    /// - duration_ms: actual speech duration in milliseconds (before padding)
    pub fn configure_for_short_audio(&mut self, n_samples: usize, duration_ms: c_int) {
        // Don't use past transcription as context (important for streaming)
        self.no_context = true;
        // Force single segment output (reduces latency)
        self.single_segment = true;
        // Suppress blank outputs
        self.suppress_blank = true;
        // Disable timestamps for speed
        self.no_timestamps = true;
        // Disable printing
        self.print_special = false;
        self.print_progress = false;
        self.print_realtime = false;
        self.print_timestamps = false;

        // Limit processing to actual speech duration (+ small buffer for safety)
        // This prevents whisper from hallucinating into padded silence
        self.duration_ms = duration_ms + 100;

        // Relax thresholds to accept lower-confidence results for short audio
        // Default logprob_thold (-1.0) is too strict for short segments
        self.logprob_thold = -2.0;
        // Disable entropy threshold - short repetitive audio (like "one two") has low entropy
        // Default entropy_thold (2.4) rejects valid short utterances
        self.entropy_thold = 0.0;
        // Disable temperature fallback - prefer fast results over retries
        self.temperature_inc = 0.0;
        // Limit max tokens to prevent repetition loops
        // For short audio (~1s), we expect at most ~10 tokens
        self.max_tokens = 16;

        // Optimize audio context size for actual audio length
        // audio_ctx is in mel spectrogram frames: n_samples / 160 (hop_length)
        // This can provide ~30% speedup for short audio
        let mel_frames = (n_samples / 160 + 1).min(1500) as c_int;
        self.audio_ctx = mel_frames;
    }
}

/// Sampling strategy enum matching whisper.cpp
#[repr(C)]
#[allow(dead_code)]
pub enum WhisperSamplingStrategy {
    Greedy = 0,
    BeamSearch = 1,
}

/// Global library handle
static WHISPER_LIB: OnceLock<Option<WhisperLibrary>> = OnceLock::new();

/// Wrapper around the loaded whisper library
pub struct WhisperLibrary {
    _lib: Library,
    // Function pointers
    init_from_file:
        unsafe extern "C" fn(path_model: *const c_char) -> WhisperContext,
    free: unsafe extern "C" fn(ctx: WhisperContext),
    full_default_params:
        unsafe extern "C" fn(strategy: c_int) -> WhisperFullParams,
    full: unsafe extern "C" fn(
        ctx: WhisperContext,
        params: WhisperFullParams,
        samples: *const c_float,
        n_samples: c_int,
    ) -> c_int,
    full_n_segments: unsafe extern "C" fn(ctx: WhisperContext) -> c_int,
    full_get_segment_text:
        unsafe extern "C" fn(ctx: WhisperContext, i_segment: c_int) -> *const c_char,
    print_system_info: unsafe extern "C" fn() -> *const c_char,
}

// SAFETY: The library handle and function pointers don't contain thread-local data
unsafe impl Send for WhisperLibrary {}
unsafe impl Sync for WhisperLibrary {}

impl WhisperLibrary {
    /// Load the whisper library from the given path
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        unsafe {
            let lib = Library::new(path.as_ref())
                .map_err(|e| format!("Failed to load whisper library: {}", e))?;

            // Load all required symbols - dereference immediately to get raw fn pointers
            let init_from_file = *lib
                .get::<unsafe extern "C" fn(*const c_char) -> WhisperContext>(
                    b"whisper_init_from_file\0",
                )
                .map_err(|e| format!("Failed to load whisper_init_from_file: {}", e))?;

            let free = *lib
                .get::<unsafe extern "C" fn(WhisperContext)>(b"whisper_free\0")
                .map_err(|e| format!("Failed to load whisper_free: {}", e))?;

            let full_default_params = *lib
                .get::<unsafe extern "C" fn(c_int) -> WhisperFullParams>(
                    b"whisper_full_default_params\0",
                )
                .map_err(|e| format!("Failed to load whisper_full_default_params: {}", e))?;

            let full = *lib
                .get::<unsafe extern "C" fn(WhisperContext, WhisperFullParams, *const c_float, c_int) -> c_int>(
                    b"whisper_full\0",
                )
                .map_err(|e| format!("Failed to load whisper_full: {}", e))?;

            let full_n_segments = *lib
                .get::<unsafe extern "C" fn(WhisperContext) -> c_int>(b"whisper_full_n_segments\0")
                .map_err(|e| format!("Failed to load whisper_full_n_segments: {}", e))?;

            let full_get_segment_text = *lib
                .get::<unsafe extern "C" fn(WhisperContext, c_int) -> *const c_char>(
                    b"whisper_full_get_segment_text\0",
                )
                .map_err(|e| format!("Failed to load whisper_full_get_segment_text: {}", e))?;

            let print_system_info = *lib
                .get::<unsafe extern "C" fn() -> *const c_char>(
                    b"whisper_print_system_info\0",
                )
                .map_err(|e| format!("Failed to load whisper_print_system_info: {}", e))?;

            Ok(Self {
                _lib: lib,
                init_from_file,
                free,
                full_default_params,
                full,
                full_n_segments,
                full_get_segment_text,
                print_system_info,
            })
        }
    }
}

/// Initialize the global whisper library
pub fn init_library() -> Result<(), String> {
    WHISPER_LIB.get_or_init(|| {
        // Try to find the library in various locations
        let lib_name = if cfg!(windows) {
            "whisper.dll"
        } else {
            "libwhisper.dylib"
        };

        // Search paths in order of preference:
        // 1. Next to the executable
        // 2. In the current directory
        // 3. System library paths (handled by libloading)
        let search_paths = [
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.join(lib_name))),
            Some(std::env::current_dir().unwrap_or_default().join(lib_name)),
            Some(std::path::PathBuf::from(lib_name)),
        ];

        for path in search_paths.iter().flatten() {
            if path.exists() {
                match WhisperLibrary::load(path) {
                    Ok(lib) => {
                        eprintln!("Loaded whisper library from: {}", path.display());
                        return Some(lib);
                    }
                    Err(e) => {
                        eprintln!("Failed to load whisper library from {}: {}", path.display(), e);
                    }
                }
            }
        }

        // Try loading from system path
        match WhisperLibrary::load(lib_name) {
            Ok(lib) => {
                eprintln!("Loaded whisper library from system path");
                Some(lib)
            }
            Err(e) => {
                eprintln!("Failed to load whisper library: {}", e);
                None
            }
        }
    });

    if WHISPER_LIB.get().and_then(|l| l.as_ref()).is_some() {
        Ok(())
    } else {
        Err("Whisper library not available".to_string())
    }
}

/// Get the loaded library or return an error
fn get_lib() -> Result<&'static WhisperLibrary, String> {
    WHISPER_LIB
        .get()
        .and_then(|l| l.as_ref())
        .ok_or_else(|| "Whisper library not loaded".to_string())
}

/// Safe wrapper around whisper context
pub struct Context {
    ptr: WhisperContext,
}

// SAFETY: WhisperContext is thread-safe according to whisper.cpp documentation
unsafe impl Send for Context {}

impl Context {
    /// Create a new context from a model file
    pub fn new<P: AsRef<Path>>(model_path: P) -> Result<Self, String> {
        let lib = get_lib()?;

        let path_str = model_path
            .as_ref()
            .to_str()
            .ok_or("Invalid model path")?;
        let c_path = CString::new(path_str).map_err(|e| format!("Invalid path: {}", e))?;

        let ptr = unsafe { (lib.init_from_file)(c_path.as_ptr()) };

        if ptr.is_null() {
            return Err(format!(
                "Failed to initialize whisper context from: {}",
                path_str
            ));
        }

        Ok(Self { ptr })
    }

    /// Run full transcription on audio samples
    pub fn full(&self, params: &WhisperFullParams, samples: &[f32]) -> Result<(), String> {
        let lib = get_lib()?;

        let result = unsafe {
            (lib.full)(
                self.ptr,
                params.clone(),
                samples.as_ptr(),
                samples.len() as c_int,
            )
        };

        if result != 0 {
            return Err(format!("Transcription failed with code: {}", result));
        }

        Ok(())
    }

    /// Get the number of segments in the transcription result
    pub fn full_n_segments(&self) -> Result<i32, String> {
        let lib = get_lib()?;
        Ok(unsafe { (lib.full_n_segments)(self.ptr) })
    }

    /// Get the text of a specific segment
    pub fn full_get_segment_text(&self, i_segment: i32) -> Result<String, String> {
        let lib = get_lib()?;

        let ptr = unsafe { (lib.full_get_segment_text)(self.ptr, i_segment) };

        if ptr.is_null() {
            return Err(format!("Failed to get segment {} text", i_segment));
        }

        let c_str = unsafe { CStr::from_ptr(ptr) };
        c_str
            .to_str()
            .map(|s| s.to_string())
            .map_err(|e| format!("Invalid UTF-8 in segment: {}", e))
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if let Ok(lib) = get_lib() {
            unsafe { (lib.free)(self.ptr) };
        }
    }
}

/// Get default parameters for the given sampling strategy
pub fn full_default_params(strategy: WhisperSamplingStrategy) -> Result<WhisperFullParams, String> {
    let lib = get_lib()?;
    Ok(unsafe { (lib.full_default_params)(strategy as c_int) })
}

/// Get whisper.cpp system info string
/// This includes information about available backends (CPU, CUDA, Metal, etc.)
pub fn get_system_info() -> Result<String, String> {
    let lib = get_lib()?;
    let ptr = unsafe { (lib.print_system_info)() };
    if ptr.is_null() {
        return Err("Failed to get system info".to_string());
    }
    let c_str = unsafe { CStr::from_ptr(ptr) };
    c_str
        .to_str()
        .map(|s| s.to_string())
        .map_err(|e| format!("Invalid UTF-8 in system info: {}", e))
}
