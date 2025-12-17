mod audio;
mod platform;
mod processor;
mod transcribe;

use audio::{AudioDevice, AudioSourceType, RecordingMode, RecordingState, generate_recording_filename, save_to_wav};
use platform::{AudioBackend, PlatformAudioDevice, create_backend};
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter, State};
use transcribe::Transcriber;

/// Detect if running on Wayland and set workaround env vars (Linux-specific)
#[cfg(target_os = "linux")]
fn configure_wayland_workarounds() {
    // Check for Wayland session
    let is_wayland = env::var("WAYLAND_DISPLAY").is_ok()
        || env::var("XDG_SESSION_TYPE")
            .map(|v| v.to_lowercase() == "wayland")
            .unwrap_or(false);

    if is_wayland {
        // WebKitGTK has compositing issues on Wayland
        // SAFETY: This is called before any threads are spawned
        unsafe {
            env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_wayland_workarounds() {
    // No-op on non-Linux platforms
}

struct AppState {
    recording: RecordingState,
    transcriber: Mutex<Transcriber>,
    /// Platform-agnostic audio backend
    audio_backend: Arc<Mutex<Option<Box<dyn AudioBackend>>>>,
    /// Flag to signal the audio processing thread to stop
    processing_active: Arc<Mutex<bool>>,
    /// Flag to enable/disable echo cancellation in the mixer
    aec_enabled: Arc<Mutex<bool>>,
    /// Recording mode - determines how multiple sources are combined
    recording_mode: Arc<Mutex<RecordingMode>>,
}

/// Convert platform device to frontend AudioDevice format
fn platform_device_to_audio_device(dev: &PlatformAudioDevice) -> AudioDevice {
    AudioDevice {
        id: dev.id.clone(),
        name: dev.name.clone(),
        source_type: dev.source_type,
    }
}

/// List all available audio sources (both input devices and system audio monitors)
#[tauri::command]
fn list_all_sources(state: State<AppState>) -> Result<Vec<AudioDevice>, String> {
    let backend_guard = state.audio_backend.lock().unwrap();
    if let Some(ref backend) = *backend_guard {
        let mut devices = Vec::new();
        
        // Add input devices
        for dev in backend.list_input_devices() {
            devices.push(platform_device_to_audio_device(&dev));
        }
        
        // Add system audio monitors
        for dev in backend.list_system_devices() {
            devices.push(platform_device_to_audio_device(&dev));
        }
        
        Ok(devices)
    } else {
        Err("Audio backend not available".to_string())
    }
}

/// Start the audio processing loop that receives samples from the backend
fn start_audio_processing_thread(
    recording: RecordingState,
    audio_backend: Arc<Mutex<Option<Box<dyn AudioBackend>>>>,
    processing_active: Arc<Mutex<bool>>,
    app_handle: AppHandle,
) {
    thread::spawn(move || {
        loop {
            // Check if we should stop
            if !*processing_active.lock().unwrap() {
                break;
            }

            // Try to receive audio from backend
            let samples = {
                let backend_guard = audio_backend.lock().unwrap();
                if let Some(ref backend) = *backend_guard {
                    backend.try_recv()
                } else {
                    None
                }
            };

            if let Some(audio_data) = samples {
                // Process the samples through the recording state
                recording.process_samples(
                    &audio_data.samples,
                    audio_data.channels as usize,
                    &app_handle,
                );
            } else {
                // No data available, sleep briefly
                thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    });
}

/// Start recording with up to two sources mixed together
/// source1_id and source2_id can be None to indicate no source
#[tauri::command]
fn start_recording(
    source1_id: Option<String>,
    source2_id: Option<String>,
    state: State<AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    // Need at least one source
    if source1_id.is_none() && source2_id.is_none() {
        return Err("At least one audio source must be selected".to_string());
    }

    let has_backend = state.audio_backend.lock().unwrap().is_some();
    
    if has_backend {
        // Initialize recording state
        let sample_rate = {
            let backend = state.audio_backend.lock().unwrap();
            backend.as_ref().map(|b| b.sample_rate()).unwrap_or(48000)
        };
        
        // Determine source type based on what's selected
        let source_type = match (source1_id.is_some(), source2_id.is_some()) {
            (true, true) => AudioSourceType::Mixed,
            (true, false) => AudioSourceType::Input, // Could be either, doesn't matter
            (false, true) => AudioSourceType::Input,
            (false, false) => unreachable!(),
        };
        
        state.recording.init_for_capture(sample_rate, 2, source_type);
        
        // Set recording flag
        {
            let state_arc = state.recording.get_state();
            let mut audio_state = state_arc.lock().unwrap();
            audio_state.is_recording = true;
            audio_state.is_monitoring = true;
            audio_state.recording_samples.clear();
            
            // Initialize visualization processor
            audio_state.visualization_processor = Some(
                crate::processor::VisualizationProcessor::new(sample_rate, 256)
            );
        }
        
        // Start capture with both sources
        {
            let backend = state.audio_backend.lock().unwrap();
            if let Some(ref backend) = *backend {
                backend.start_capture_sources(source1_id.clone(), source2_id.clone())?;
            }
        }
        
        // Start processing thread if not already running
        *state.processing_active.lock().unwrap() = true;
        start_audio_processing_thread(
            state.recording.clone(),
            Arc::clone(&state.audio_backend),
            Arc::clone(&state.processing_active),
            app_handle,
        );
        
        Ok(())
    } else {
        Err("Audio backend not available".to_string())
    }
}

#[tauri::command]
fn stop_recording(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    keep_monitoring: bool,
) -> Result<(), String> {
    let has_backend = state.audio_backend.lock().unwrap().is_some();
    
    if has_backend {
        // Stop capture if not keeping monitoring
        if !keep_monitoring {
            if let Some(ref backend) = *state.audio_backend.lock().unwrap() {
                backend.stop_capture()?;
            }
            *state.processing_active.lock().unwrap() = false;
        }
        
        // Extract recorded audio
        let (samples, sample_rate, channels) = {
            let state_arc = state.recording.get_state();
            let mut audio_state = state_arc.lock().unwrap();
            audio_state.is_recording = false;
            if !keep_monitoring {
                audio_state.is_monitoring = false;
                audio_state.visualization_processor = None;
            }
            let samples = std::mem::take(&mut audio_state.recording_samples);
            (samples, audio_state.sample_rate, audio_state.channels)
        };
        
        if samples.is_empty() {
            return Err("No audio recorded".to_string());
        }
        
        // Save raw audio to WAV file in ~/Documents/Recordings
        let filename = generate_recording_filename();
        let recordings_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Documents")
            .join("Recordings");
        
        // Create directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&recordings_dir) {
            eprintln!("Failed to create recordings directory: {}", e);
        }
        
        let output_path = recordings_dir.join(&filename);
        
        println!("Attempting to save {} samples to: {:?}", samples.len(), output_path);
        
        if let Err(e) = save_to_wav(&samples, sample_rate, channels, &output_path) {
            eprintln!("Failed to save WAV file: {}", e);
            // Continue with transcription even if save fails
        } else {
            let path_str = output_path.to_string_lossy().to_string();
            println!("Saved recording to: {}", path_str);
            // Emit event with saved file path
            let _ = app_handle.emit("recording-saved", path_str);
        }
        
        // Create raw audio for processing
        let raw_audio = audio::RawRecordedAudio {
            samples,
            sample_rate,
            channels,
        };
        
        // Get transcriber info
        let transcriber = state.transcriber.lock().unwrap();
        let model_available = transcriber.is_model_available();
        let model_path = transcriber.get_model_path().clone();
        drop(transcriber);
        
        // Process and transcribe in background thread
        thread::spawn(move || {
            let processed = match audio::process_recorded_audio(raw_audio) {
                Ok(samples) => samples,
                Err(e) => {
                    let _ = app_handle.emit("transcription-error", e);
                    return;
                }
            };
            
            if !model_available {
                let _ = app_handle.emit("transcription-error", "Model not available".to_string());
                return;
            }
            
            let mut transcriber = Transcriber::new();
            if model_path.exists() {
                match transcriber.transcribe(&processed) {
                    Ok(text) => {
                        let _ = app_handle.emit("transcription-complete", text);
                    }
                    Err(e) => {
                        let _ = app_handle.emit("transcription-error", e);
                    }
                }
            } else {
                let _ = app_handle.emit("transcription-error", "Model file not found".to_string());
            }
        });
        
        Ok(())
    } else {
        Err("Audio backend not available".to_string())
    }
}

#[tauri::command]
fn is_recording(state: State<AppState>) -> bool {
    state.recording.is_recording()
}

/// Start monitoring with up to two sources mixed together
#[tauri::command]
fn start_monitor(
    source1_id: Option<String>,
    source2_id: Option<String>,
    state: State<AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    // Need at least one source
    if source1_id.is_none() && source2_id.is_none() {
        return Err("At least one audio source must be selected".to_string());
    }

    let has_backend = state.audio_backend.lock().unwrap().is_some();
    
    if has_backend {
        // Initialize state
        let sample_rate = {
            let backend = state.audio_backend.lock().unwrap();
            backend.as_ref().map(|b| b.sample_rate()).unwrap_or(48000)
        };
        
        let source_type = match (source1_id.is_some(), source2_id.is_some()) {
            (true, true) => AudioSourceType::Mixed,
            _ => AudioSourceType::Input,
        };
        
        state.recording.init_for_capture(sample_rate, 2, source_type);
        
        // Set monitoring flag and create visualization processor
        {
            let state_arc = state.recording.get_state();
            let mut audio_state = state_arc.lock().unwrap();
            audio_state.is_monitoring = true;
            audio_state.visualization_processor = Some(
                crate::processor::VisualizationProcessor::new(sample_rate, 256)
            );
        }
        
        // Start capture with both sources
        {
            let backend = state.audio_backend.lock().unwrap();
            if let Some(ref backend) = *backend {
                backend.start_capture_sources(source1_id.clone(), source2_id.clone())?;
            }
        }
        
        // Start processing thread
        *state.processing_active.lock().unwrap() = true;
        start_audio_processing_thread(
            state.recording.clone(),
            Arc::clone(&state.audio_backend),
            Arc::clone(&state.processing_active),
            app_handle,
        );
        
        Ok(())
    } else {
        Err("Audio backend not available".to_string())
    }
}

#[tauri::command]
fn stop_monitor(state: State<AppState>) -> Result<(), String> {
    let has_backend = state.audio_backend.lock().unwrap().is_some();
    
    if has_backend {
        // Stop processing thread
        *state.processing_active.lock().unwrap() = false;
        
        // Stop capture
        if let Some(ref backend) = *state.audio_backend.lock().unwrap() {
            backend.stop_capture()?;
        }
        
        // Update state
        {
            let state_arc = state.recording.get_state();
            let mut audio_state = state_arc.lock().unwrap();
            audio_state.is_monitoring = false;
            audio_state.visualization_processor = None;
        }
        state.recording.mark_capture_stopped();
        
        Ok(())
    } else {
        Err("Audio backend not available".to_string())
    }
}

#[tauri::command]
fn is_monitoring(state: State<AppState>) -> bool {
    state.recording.is_monitoring()
}

#[tauri::command]
fn set_aec_enabled(enabled: bool, state: State<AppState>) {
    *state.aec_enabled.lock().unwrap() = enabled;
}

#[tauri::command]
fn is_aec_enabled(state: State<AppState>) -> bool {
    *state.aec_enabled.lock().unwrap()
}

#[tauri::command]
fn set_recording_mode(mode: RecordingMode, state: State<AppState>) {
    println!("set_recording_mode called with: {:?}", mode);
    *state.recording_mode.lock().unwrap() = mode;
}

#[tauri::command]
fn get_recording_mode(state: State<AppState>) -> RecordingMode {
    *state.recording_mode.lock().unwrap()
}

#[tauri::command]
fn transcribe(audio_data: Vec<f32>, state: State<AppState>) -> Result<String, String> {
    let mut transcriber = state.transcriber.lock().unwrap();
    transcriber.transcribe(&audio_data)
}

#[tauri::command]
fn check_model_status(state: State<AppState>) -> Result<ModelStatus, String> {
    let transcriber = state.transcriber.lock().unwrap();
    Ok(ModelStatus {
        available: transcriber.is_model_available(),
        path: transcriber.get_model_path().to_string_lossy().to_string(),
    })
}

#[tauri::command]
fn download_model(state: State<AppState>) -> Result<(), String> {
    let transcriber = state.transcriber.lock().unwrap();
    let model_path = transcriber.get_model_path().clone();
    drop(transcriber);
    
    transcribe::download_model(&model_path)
}

#[derive(serde::Serialize)]
struct ModelStatus {
    available: bool,
    path: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    configure_wayland_workarounds();
    
    // Create shared AEC enabled flag
    let aec_enabled = Arc::new(Mutex::new(false));
    
    // Create shared recording mode
    let recording_mode = Arc::new(Mutex::new(RecordingMode::Mixed));
    
    // Initialize platform-specific audio backend with shared flags
    let audio_backend = match create_backend(Arc::clone(&aec_enabled), Arc::clone(&recording_mode)) {
        Ok(backend) => {
            println!("Audio backend initialized");
            Some(backend)
        }
        Err(e) => {
            eprintln!("Failed to initialize audio backend: {}", e);
            None
        }
    };

    tauri::Builder::default()
        .manage(AppState {
            recording: RecordingState::new(),
            transcriber: Mutex::new(Transcriber::new()),
            audio_backend: Arc::new(Mutex::new(audio_backend)),
            processing_active: Arc::new(Mutex::new(false)),
            aec_enabled,
            recording_mode,
        })
        .invoke_handler(tauri::generate_handler![
            list_all_sources,
            start_recording,
            stop_recording,
            is_recording,
            start_monitor,
            stop_monitor,
            is_monitoring,
            set_aec_enabled,
            is_aec_enabled,
            set_recording_mode,
            get_recording_mode,
            transcribe,
            check_model_status,
            download_model,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
