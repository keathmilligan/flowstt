use std::fs;
use std::io::Write;
use std::path::PathBuf;

const MODEL_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin";

// =============================================================================
// Linux implementation using whisper-rs
// =============================================================================
#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    pub struct TranscriberImpl {
        ctx: Option<WhisperContext>,
        model_path: PathBuf,
    }

    impl TranscriberImpl {
        pub fn new() -> Self {
            let model_path = get_default_model_path();
            Self {
                ctx: None,
                model_path,
            }
        }

        pub fn get_model_path(&self) -> &PathBuf {
            &self.model_path
        }

        pub fn is_model_available(&self) -> bool {
            self.model_path.exists()
        }

        pub fn load_model(&mut self) -> Result<(), String> {
            if self.ctx.is_some() {
                return Ok(());
            }

            if !self.model_path.exists() {
                return Err(format!(
                    "Whisper model not found at: {}\n\n\
                    Please download a model file:\n\
                    1. Visit: https://huggingface.co/ggerganov/whisper.cpp/tree/main\n\
                    2. Download 'ggml-base.en.bin' (or another model)\n\
                    3. Place it at: {}",
                    self.model_path.display(),
                    self.model_path.display()
                ));
            }

            let ctx = WhisperContext::new_with_params(
                self.model_path.to_str().unwrap(),
                WhisperContextParameters::default(),
            )
            .map_err(|e| format!("Failed to load Whisper model: {}", e))?;

            self.ctx = Some(ctx);
            Ok(())
        }

        pub fn transcribe(&mut self, audio_data: &[f32], _duration_ms: i32) -> Result<String, String> {
            self.load_model()?;

            let ctx = self.ctx.as_ref().unwrap();

            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_language(Some("en"));
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);

            let mut state = ctx
                .create_state()
                .map_err(|e| format!("Failed to create state: {}", e))?;

            state
                .full(params, audio_data)
                .map_err(|e| format!("Transcription failed: {}", e))?;

            let num_segments = state.full_n_segments();

            if num_segments == 0 {
                return Ok("(No speech detected)".to_string());
            }

            let mut result = String::new();
            for i in 0..num_segments {
                if let Some(segment) = state.get_segment(i) {
                    result.push_str(&segment.to_string());
                    result.push(' ');
                }
            }

            let trimmed = result.trim().to_string();
            if trimmed.is_empty() {
                Ok("(No speech detected)".to_string())
            } else {
                Ok(trimmed)
            }
        }
    }
}

// =============================================================================
// Windows/macOS implementation using FFI with prebuilt binaries
// =============================================================================
#[cfg(not(target_os = "linux"))]
mod platform {
    use super::*;
    use crate::whisper_ffi::{self, Context, WhisperSamplingStrategy};

    pub struct TranscriberImpl {
        ctx: Option<Context>,
        model_path: PathBuf,
        library_initialized: bool,
    }

    impl TranscriberImpl {
        pub fn new() -> Self {
            let model_path = get_default_model_path();
            Self {
                ctx: None,
                model_path,
                library_initialized: false,
            }
        }

        pub fn get_model_path(&self) -> &PathBuf {
            &self.model_path
        }

        pub fn is_model_available(&self) -> bool {
            self.model_path.exists()
        }

        fn ensure_library(&mut self) -> Result<(), String> {
            if !self.library_initialized {
                whisper_ffi::init_library()?;
                self.library_initialized = true;
            }
            Ok(())
        }

        pub fn load_model(&mut self) -> Result<(), String> {
            if self.ctx.is_some() {
                return Ok(());
            }

            self.ensure_library()?;

            if !self.model_path.exists() {
                return Err(format!(
                    "Whisper model not found at: {}\n\n\
                    Please download a model file:\n\
                    1. Visit: https://huggingface.co/ggerganov/whisper.cpp/tree/main\n\
                    2. Download 'ggml-base.en.bin' (or another model)\n\
                    3. Place it at: {}",
                    self.model_path.display(),
                    self.model_path.display()
                ));
            }

            let ctx = Context::new(&self.model_path)?;
            self.ctx = Some(ctx);
            Ok(())
        }

        pub fn transcribe(&mut self, audio_data: &[f32], _duration_ms: i32) -> Result<String, String> {
            self.load_model()?;

            let ctx = self.ctx.as_ref().unwrap();

            // Get default params with greedy strategy
            let params = whisper_ffi::full_default_params(WhisperSamplingStrategy::Greedy)?;

            // Run transcription
            ctx.full(&params, audio_data)?;

            let num_segments = ctx.full_n_segments()?;

            if num_segments == 0 {
                return Ok("(No speech detected)".to_string());
            }

            let mut result = String::new();
            for i in 0..num_segments {
                if let Ok(segment) = ctx.full_get_segment_text(i) {
                    result.push_str(&segment);
                    result.push(' ');
                }
            }

            let trimmed = result.trim().to_string();
            if trimmed.is_empty() {
                Ok("(No speech detected)".to_string())
            } else {
                Ok(trimmed)
            }
        }
    }
}

// =============================================================================
// Public API - same for all platforms
// =============================================================================

pub struct Transcriber {
    inner: platform::TranscriberImpl,
}

impl Transcriber {
    pub fn new() -> Self {
        Self {
            inner: platform::TranscriberImpl::new(),
        }
    }

    pub fn get_model_path(&self) -> &PathBuf {
        self.inner.get_model_path()
    }

    pub fn is_model_available(&self) -> bool {
        self.inner.is_model_available()
    }

    pub fn load_model(&mut self) -> Result<(), String> {
        self.inner.load_model()
    }

    pub fn transcribe(&mut self, audio_data: &[f32]) -> Result<String, String> {
        let duration_ms = (audio_data.len() * 1000 / 16000) as i32;
        self.inner.transcribe(audio_data, duration_ms)
    }
}

fn get_default_model_path() -> PathBuf {
    let cache_dir = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("."));
    cache_dir.join("whisper").join("ggml-base.en.bin")
}

pub fn download_model(model_path: &PathBuf) -> Result<(), String> {
    // Create parent directory if it doesn't exist
    if let Some(parent) = model_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    // Download the model
    let response = reqwest::blocking::get(MODEL_URL)
        .map_err(|e| format!("Failed to download model: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to download model: HTTP {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .map_err(|e| format!("Failed to read response: {}", e))?;

    // Write to file
    let mut file = fs::File::create(model_path)
        .map_err(|e| format!("Failed to create file: {}", e))?;

    file.write_all(&bytes)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(())
}
