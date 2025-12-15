mod audio;
mod pipewire_audio;
mod processor;
mod transcribe;

use audio::{AudioDevice, AudioSourceType, RecordingState};
use pipewire_audio::{PipeWireBackend, PwAudioDevice};
use std::env;
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter, State};
use transcribe::Transcriber;

/// Detect if running on Wayland and set workaround env vars
fn configure_wayland_workarounds() {
    // Check for Wayland session
    let is_wayland = env::var("WAYLAND_DISPLAY").is_ok()
        || env::var("XDG_SESSION_TYPE")
            .map(|v| v.to_lowercase() == "wayland")
            .unwrap_or(false);

    if is_wayland {
        // WebKitGTK has compositing issues on Wayland
        env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }
}

struct AppState {
    recording: RecordingState,
    transcriber: Mutex<Transcriber>,
    pipewire: Arc<Mutex<Option<PipeWireBackend>>>,
    /// Flag to signal the audio processing thread to stop
    processing_active: Arc<Mutex<bool>>,
}

// Implement Send + Sync for AppState since all fields use Arc<Mutex<_>>
unsafe impl Send for AppState {}
unsafe impl Sync for AppState {}

/// Convert PipeWire device to frontend AudioDevice format
fn pw_device_to_audio_device(pw_dev: &PwAudioDevice) -> AudioDevice {
    AudioDevice {
        id: pw_dev.id.to_string(),
        name: pw_dev.name.clone(),
        source_type: pw_dev.source_type,
    }
}

/// List all available audio sources (both input devices and system audio monitors)
#[tauri::command]
fn list_all_sources(state: State<AppState>) -> Result<Vec<AudioDevice>, String> {
    let pw_guard = state.pipewire.lock().unwrap();
    if let Some(ref pw) = *pw_guard {
        let mut devices = Vec::new();
        
        // Add input devices
        for dev in pw.list_input_devices() {
            devices.push(pw_device_to_audio_device(&dev));
        }
        
        // Add system audio monitors
        for dev in pw.list_system_devices() {
            devices.push(pw_device_to_audio_device(&dev));
        }
        
        Ok(devices)
    } else {
        // Fallback to cpal if PipeWire not available
        audio::list_devices()
    }
}

/// Start the audio processing loop that receives samples from PipeWire
fn start_audio_processing_thread(
    recording: RecordingState,
    pipewire: Arc<Mutex<Option<PipeWireBackend>>>,
    processing_active: Arc<Mutex<bool>>,
    app_handle: AppHandle,
) {
    thread::spawn(move || {
        loop {
            // Check if we should stop
            if !*processing_active.lock().unwrap() {
                break;
            }

            // Try to receive audio from PipeWire
            let samples = {
                let pw_guard = pipewire.lock().unwrap();
                if let Some(ref pw) = *pw_guard {
                    pw.try_recv()
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

    let has_pipewire = state.pipewire.lock().unwrap().is_some();
    
    if has_pipewire {
        let source1: Option<u32> = source1_id.as_ref().and_then(|s| s.parse().ok());
        let source2: Option<u32> = source2_id.as_ref().and_then(|s| s.parse().ok());
        
        // Initialize recording state
        let sample_rate = {
            let pw = state.pipewire.lock().unwrap();
            pw.as_ref().map(|p| p.sample_rate()).unwrap_or(48000)
        };
        
        // Determine source type based on what's selected
        let source_type = match (source1.is_some(), source2.is_some()) {
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
        
        // Start PipeWire capture with both sources
        {
            let pw = state.pipewire.lock().unwrap();
            if let Some(ref pw) = *pw {
                pw.start_capture_sources(source1, source2)?;
            }
        }
        
        // Start processing thread if not already running
        *state.processing_active.lock().unwrap() = true;
        start_audio_processing_thread(
            state.recording.clone(),
            Arc::clone(&state.pipewire),
            Arc::clone(&state.processing_active),
            app_handle,
        );
        
        Ok(())
    } else {
        Err("PipeWire not available".to_string())
    }
}

#[tauri::command]
fn stop_recording(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    keep_monitoring: bool,
) -> Result<(), String> {
    let has_pipewire = state.pipewire.lock().unwrap().is_some();
    
    if has_pipewire {
        // Stop PipeWire capture if not keeping monitoring
        if !keep_monitoring {
            if let Some(ref pw) = *state.pipewire.lock().unwrap() {
                pw.stop_capture()?;
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
        Err("PipeWire not available".to_string())
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

    let has_pipewire = state.pipewire.lock().unwrap().is_some();
    
    if has_pipewire {
        let source1: Option<u32> = source1_id.as_ref().and_then(|s| s.parse().ok());
        let source2: Option<u32> = source2_id.as_ref().and_then(|s| s.parse().ok());
        
        // Initialize state
        let sample_rate = {
            let pw = state.pipewire.lock().unwrap();
            pw.as_ref().map(|p| p.sample_rate()).unwrap_or(48000)
        };
        
        let source_type = match (source1.is_some(), source2.is_some()) {
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
        
        // Start PipeWire capture with both sources
        {
            let pw = state.pipewire.lock().unwrap();
            if let Some(ref pw) = *pw {
                pw.start_capture_sources(source1, source2)?;
            }
        }
        
        // Start processing thread
        *state.processing_active.lock().unwrap() = true;
        start_audio_processing_thread(
            state.recording.clone(),
            Arc::clone(&state.pipewire),
            Arc::clone(&state.processing_active),
            app_handle,
        );
        
        Ok(())
    } else {
        Err("PipeWire not available".to_string())
    }
}

#[tauri::command]
fn stop_monitor(state: State<AppState>) -> Result<(), String> {
    let has_pipewire = state.pipewire.lock().unwrap().is_some();
    
    if has_pipewire {
        // Stop processing thread
        *state.processing_active.lock().unwrap() = false;
        
        // Stop PipeWire capture
        if let Some(ref pw) = *state.pipewire.lock().unwrap() {
            pw.stop_capture()?;
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
        Err("PipeWire not available".to_string())
    }
}

#[tauri::command]
fn is_monitoring(state: State<AppState>) -> bool {
    state.recording.is_monitoring()
}

#[tauri::command]
fn set_processing_enabled(enabled: bool, state: State<AppState>) {
    state.recording.set_processing_enabled(enabled);
}

#[tauri::command]
fn is_processing_enabled(state: State<AppState>) -> bool {
    state.recording.is_processing_enabled()
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
    
    // Try to initialize PipeWire backend
    let pipewire = match PipeWireBackend::new() {
        Ok(pw) => {
            println!("PipeWire audio backend initialized");
            Some(pw)
        }
        Err(e) => {
            eprintln!("Failed to initialize PipeWire, falling back to cpal: {}", e);
            None
        }
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            recording: RecordingState::new(),
            transcriber: Mutex::new(Transcriber::new()),
            pipewire: Arc::new(Mutex::new(pipewire)),
            processing_active: Arc::new(Mutex::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            list_all_sources,
            start_recording,
            stop_recording,
            is_recording,
            start_monitor,
            stop_monitor,
            is_monitoring,
            set_processing_enabled,
            is_processing_enabled,
            transcribe,
            check_model_status,
            download_model,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
