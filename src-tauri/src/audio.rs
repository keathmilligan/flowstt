use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, StreamConfig};
use rubato::{FftFixedIn, Resampler};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::AppHandle;

#[allow(unused_imports)]
use crate::processor::{AudioProcessor, SilenceDetector, SpeechDetector, VisualizationProcessor};

/// Audio source type for capture
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum AudioSourceType {
    /// Microphone or other input device
    Input,
    /// System audio (monitor/loopback)
    System,
    /// Mixed input and system audio
    Mixed,
}

impl Default for AudioSourceType {
    fn default() -> Self {
        AudioSourceType::Input
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub source_type: AudioSourceType,
}

/// Raw recorded audio data before processing
pub struct RawRecordedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Shared state for audio stream
pub struct AudioStreamState {
    // Recording state
    pub recording_samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub is_recording: bool,
    
    // Monitoring state
    pub is_monitoring: bool,
    
    // Visualization processor (always runs when monitoring)
    pub visualization_processor: Option<VisualizationProcessor>,
    
    // Speech processing state (controlled by toggle)
    pub is_processing_enabled: bool,
    pub speech_processor: Option<Box<dyn AudioProcessor>>,
    
    // Stream control
    pub stream_active: bool,
    
    // Source type for current capture
    pub source_type: AudioSourceType,
}

/// Mixer state for combining input and system audio in Mixed mode
struct MixerState {
    input_buffer: Vec<f32>,
    system_buffer: Vec<f32>,
    input_channels: u16,
    system_channels: u16,
}

/// Thread-safe audio state that can be shared with Tauri
#[derive(Clone)]
pub struct RecordingState {
    state: Arc<Mutex<AudioStreamState>>,
    stop_signal: Arc<Mutex<bool>>,
    current_device_id: Arc<Mutex<Option<String>>>,
    // For Mixed mode: secondary device (system audio)
    secondary_device_id: Arc<Mutex<Option<String>>>,
    secondary_stop_signal: Arc<Mutex<bool>>,
    // Mixer for combining streams in Mixed mode
    mixer: Arc<Mutex<MixerState>>,
}

impl RecordingState {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(AudioStreamState {
                recording_samples: Vec::new(),
                sample_rate: 0,
                channels: 0,
                is_recording: false,
                is_monitoring: false,
                visualization_processor: None, // Created when monitoring starts with known sample rate
                is_processing_enabled: false,
                speech_processor: None, // Created when processing is enabled with known sample rate
                stream_active: false,
                source_type: AudioSourceType::Input,
            })),
            stop_signal: Arc::new(Mutex::new(false)),
            current_device_id: Arc::new(Mutex::new(None)),
            secondary_device_id: Arc::new(Mutex::new(None)),
            secondary_stop_signal: Arc::new(Mutex::new(false)),
            mixer: Arc::new(Mutex::new(MixerState {
                input_buffer: Vec::new(),
                system_buffer: Vec::new(),
                input_channels: 0,
                system_channels: 0,
            })),
        }
    }

    pub fn is_recording(&self) -> bool {
        self.state.lock().unwrap().is_recording
    }

    pub fn is_monitoring(&self) -> bool {
        self.state.lock().unwrap().is_monitoring
    }

    pub fn is_processing_enabled(&self) -> bool {
        self.state.lock().unwrap().is_processing_enabled
    }

    pub fn set_processing_enabled(&self, enabled: bool) {
        let mut state = self.state.lock().unwrap();
        state.is_processing_enabled = enabled;
        // Reset processor state when enabling
        if enabled {
            // Use current sample rate if available, otherwise default to 48000
            let sample_rate = if state.sample_rate > 0 { state.sample_rate } else { 48000 };
            state.speech_processor = Some(Box::new(SpeechDetector::new(sample_rate)));
        }
    }

    /// Initialize for PipeWire capture with given sample rate and channels
    pub fn init_for_capture(&self, sample_rate: u32, channels: u16, source_type: AudioSourceType) {
        let mut state = self.state.lock().unwrap();
        state.sample_rate = sample_rate;
        state.channels = channels;
        state.source_type = source_type;
        state.stream_active = true;
    }

    /// Mark capture as stopped
    pub fn mark_capture_stopped(&self) {
        let mut state = self.state.lock().unwrap();
        state.stream_active = false;
    }

    /// Process incoming audio samples from PipeWire
    /// This is called from the audio processing thread
    pub fn process_samples(&self, samples: &[f32], channels: usize, app_handle: &AppHandle) {
        process_audio_samples(samples, channels, &self.state, app_handle);
    }

    /// Get internal state for advanced operations
    pub fn get_state(&self) -> Arc<Mutex<AudioStreamState>> {
        Arc::clone(&self.state)
    }
}

/// List available input devices (microphones)
pub fn list_devices() -> Result<Vec<AudioDevice>, String> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|e| format!("Failed to enumerate devices: {}", e))?;

    let mut result = Vec::new();
    for (index, device) in devices.enumerate() {
        let name = device
            .name()
            .unwrap_or_else(|_| format!("Unknown Device {}", index));
        
        // Skip monitor sources from input device list
        if is_monitor_source(&name) {
            continue;
        }
        
        result.push(AudioDevice {
            id: index.to_string(),
            name,
            source_type: AudioSourceType::Input,
        });
    }

    Ok(result)
}

/// Check if a device name indicates a monitor/loopback source
fn is_monitor_source(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    name_lower.contains("monitor") || name_lower.contains("loopback")
}

/// List available system audio devices (monitor/loopback sources)
/// Note: System audio capture on Linux requires additional setup.
/// cpal's ALSA backend doesn't directly support PipeWire/PulseAudio monitor sources.
pub fn list_system_audio_devices() -> Result<Vec<AudioDevice>, String> {
    // Check if cpal can see any monitor sources directly (unlikely with ALSA backend)
    let host = cpal::default_host();
    if let Ok(devices) = host.input_devices() {
        let mut result = Vec::new();
        for (index, device) in devices.enumerate() {
            let name = device
                .name()
                .unwrap_or_else(|_| format!("Unknown Device {}", index));
            
            if is_monitor_source(&name) {
                let display_name = make_monitor_display_name(&name);
                result.push(AudioDevice {
                    id: index.to_string(),
                    name: display_name,
                    source_type: AudioSourceType::System,
                });
            }
        }
        if !result.is_empty() {
            return Ok(result);
        }
    }
    
    // System audio not available through cpal/ALSA
    // Return empty list - UI will show appropriate message
    Ok(vec![])
}

/// Create a user-friendly display name for monitor sources
fn make_monitor_display_name(name: &str) -> String {
    // PulseAudio/PipeWire format: "Monitor of <output device name>"
    // or "<output>.monitor"
    if let Some(stripped) = name.strip_prefix("Monitor of ") {
        return stripped.to_string();
    }
    if let Some(stripped) = name.strip_suffix(".monitor") {
        // Parse ALSA-style names like "alsa_output.pci-0000_00_1f.3.analog-stereo"
        // or "alsa_output.usb-Kingston_HyperX_QuadCast_S_4100-00.analog-stereo"
        
        // Try to extract device description from the name
        if let Some(rest) = stripped.strip_prefix("alsa_output.") {
            // Split by dots to get parts
            let parts: Vec<&str> = rest.split('.').collect();
            
            // Check for USB device (has manufacturer/model in name)
            if parts.len() >= 2 && parts[0].starts_with("usb-") {
                // Extract device name from USB identifier like "usb-Kingston_HyperX_QuadCast_S_4100-00"
                let usb_part = parts[0].strip_prefix("usb-").unwrap_or(parts[0]);
                // Take the part before the serial number (last segment after -)
                let device_name = usb_part.rsplitn(2, '-').last().unwrap_or(usb_part);
                let clean_name = device_name.replace('_', " ");
                let output_type = parts.last().unwrap_or(&"output").replace('-', " ");
                return format!("{} ({})", clean_name, output_type);
            }
            
            // For PCI devices, use the output type with a generic name
            if parts.len() >= 2 && parts[0].starts_with("pci-") {
                let output_type = parts.last().unwrap_or(&"output");
                let friendly_type = match *output_type {
                    "analog-stereo" => "Speakers",
                    "hdmi-stereo" => "HDMI",
                    "hdmi-stereo-extra1" => "HDMI 2",
                    "hdmi-stereo-extra2" => "HDMI 3",
                    _ => output_type,
                };
                return friendly_type.to_string();
            }
        }
        
        // Fallback: use last part
        let parts: Vec<&str> = stripped.split('.').collect();
        if let Some(last_part) = parts.last() {
            return last_part.replace('-', " ").replace('_', " ");
        }
    }
    // Final fallback
    name.to_string()
}

fn get_device_by_id(device_id: &str) -> Result<Device, String> {
    let host = cpal::default_host();
    let index: usize = device_id
        .parse()
        .map_err(|_| "Invalid device ID".to_string())?;

    let devices = host
        .input_devices()
        .map_err(|e| format!("Failed to enumerate devices: {}", e))?;

    devices
        .enumerate()
        .find(|(i, _)| *i == index)
        .map(|(_, d)| d)
        .ok_or_else(|| "Device not found".to_string())
}

/// Start the audio stream if not already running
fn ensure_stream_running(
    device_id: &str,
    state: &RecordingState,
    app_handle: AppHandle,
) -> Result<(), String> {
    let needs_start = {
        let audio_state = state.state.lock().unwrap();
        !audio_state.stream_active
    };

    if !needs_start {
        // Check if device changed
        let current_device = state.current_device_id.lock().unwrap();
        if current_device.as_deref() != Some(device_id) {
            return Err("Cannot change device while stream is active".to_string());
        }
        return Ok(());
    }

    let device = get_device_by_id(device_id)?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("Failed to get default config: {}", e))?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let sample_format = config.sample_format();

    // Initialize state
    {
        let mut audio_state = state.state.lock().unwrap();
        audio_state.sample_rate = sample_rate;
        audio_state.channels = channels;
        audio_state.stream_active = true;
    }

    // Store current device
    {
        let mut current = state.current_device_id.lock().unwrap();
        *current = Some(device_id.to_string());
    }

    // Reset stop signal
    {
        let mut stop = state.stop_signal.lock().unwrap();
        *stop = false;
    }

    let state_clone = Arc::clone(&state.state);
    let stop_signal = Arc::clone(&state.stop_signal);

    // Spawn audio stream thread
    thread::spawn(move || {
        let stream_config: StreamConfig = config.into();
        let err_fn = |err| eprintln!("Audio stream error: {}", err);

        let state_for_callback = Arc::clone(&state_clone);
        let app_for_callback = app_handle.clone();
        let channels_for_callback = channels;

        let stream_result = match sample_format {
            SampleFormat::F32 => device.build_input_stream(
                &stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    process_audio_samples(
                        data,
                        channels_for_callback as usize,
                        &state_for_callback,
                        &app_for_callback,
                    );
                },
                err_fn,
                None,
            ),
            SampleFormat::I16 => {
                let state_for_i16 = Arc::clone(&state_clone);
                let app_for_i16 = app_handle.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let float_samples: Vec<f32> =
                            data.iter().map(|&s| s as f32 / 32768.0).collect();
                        process_audio_samples(
                            &float_samples,
                            channels_for_callback as usize,
                            &state_for_i16,
                            &app_for_i16,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::U16 => {
                let state_for_u16 = Arc::clone(&state_clone);
                let app_for_u16 = app_handle.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        let float_samples: Vec<f32> = data
                            .iter()
                            .map(|&s| (s as f32 - 32768.0) / 32768.0)
                            .collect();
                        process_audio_samples(
                            &float_samples,
                            channels_for_callback as usize,
                            &state_for_u16,
                            &app_for_u16,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            _ => {
                eprintln!("Unsupported sample format: {:?}", sample_format);
                if let Ok(mut s) = state_clone.lock() {
                    s.stream_active = false;
                }
                return;
            }
        };

        let stream = match stream_result {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to build stream: {}", e);
                if let Ok(mut s) = state_clone.lock() {
                    s.stream_active = false;
                    s.is_monitoring = false;
                    s.is_recording = false;
                }
                return;
            }
        };

        if let Err(e) = stream.play() {
            eprintln!("Failed to start stream: {}", e);
            if let Ok(mut s) = state_clone.lock() {
                s.stream_active = false;
                s.is_monitoring = false;
                s.is_recording = false;
            }
            return;
        }

        // Wait for stop signal
        loop {
            thread::sleep(std::time::Duration::from_millis(10));
            if *stop_signal.lock().unwrap() {
                break;
            }
        }

        // Mark stream as inactive
        if let Ok(mut s) = state_clone.lock() {
            s.stream_active = false;
        }

        // Stream is dropped here when thread ends
    });

    Ok(())
}

/// Stop the audio stream if neither monitoring nor recording
fn maybe_stop_stream(state: &RecordingState) {
    let should_stop = {
        let audio_state = state.state.lock().unwrap();
        audio_state.stream_active && !audio_state.is_monitoring && !audio_state.is_recording
    };

    if should_stop {
        // Signal the stream thread to stop
        {
            let mut stop = state.stop_signal.lock().unwrap();
            *stop = true;
        }

        // Clear device (don't wait - let it stop asynchronously)
        {
            let mut current = state.current_device_id.lock().unwrap();
            *current = None;
        }
    }
}

/// Stop the secondary stream (used in Mixed mode)
fn maybe_stop_secondary_stream(state: &RecordingState) {
    // Signal the secondary stream thread to stop
    {
        let mut stop = state.secondary_stop_signal.lock().unwrap();
        *stop = true;
    }

    // Clear secondary device
    {
        let mut secondary = state.secondary_device_id.lock().unwrap();
        *secondary = None;
    }
    
    // Clear mixer buffers
    {
        let mut mixer = state.mixer.lock().unwrap();
        mixer.input_buffer.clear();
        mixer.system_buffer.clear();
    }
}

/// Start mixed mode streams (input + system audio)
/// Note: Mixed mode is not currently supported on Linux as system audio capture
/// requires PipeWire/PulseAudio integration which is not available through cpal.
#[allow(unused_variables)]
fn ensure_mixed_streams_running(
    input_device_id: &str,
    system_device_id: &str,
    state: &RecordingState,
    app_handle: AppHandle,
) -> Result<(), String> {
    Err("Mixed mode is not currently supported. System audio capture requires additional setup on Linux.".to_string())
}

/// Process input samples in mixed mode - add to mixer buffer
fn process_mixed_input_samples(
    samples: &[f32],
    mixer: &Arc<Mutex<MixerState>>,
    state: &Arc<Mutex<AudioStreamState>>,
    app_handle: &AppHandle,
) {
    if let Ok(mut mixer_state) = mixer.try_lock() {
        // Convert to mono and add to input buffer
        let mono: Vec<f32> = if mixer_state.input_channels > 1 {
            samples
                .chunks(mixer_state.input_channels as usize)
                .map(|chunk| chunk.iter().sum::<f32>() / mixer_state.input_channels as f32)
                .collect()
        } else {
            samples.to_vec()
        };
        mixer_state.input_buffer.extend(mono);
        
        // Try to mix if both buffers have enough samples
        try_mix_and_process(&mut mixer_state, state, app_handle);
    }
}

/// Process system samples in mixed mode - add to mixer buffer
fn process_mixed_system_samples(
    samples: &[f32],
    mixer: &Arc<Mutex<MixerState>>,
    state: &Arc<Mutex<AudioStreamState>>,
    app_handle: &AppHandle,
) {
    if let Ok(mut mixer_state) = mixer.try_lock() {
        // Convert to mono and add to system buffer
        let mono: Vec<f32> = if mixer_state.system_channels > 1 {
            samples
                .chunks(mixer_state.system_channels as usize)
                .map(|chunk| chunk.iter().sum::<f32>() / mixer_state.system_channels as f32)
                .collect()
        } else {
            samples.to_vec()
        };
        mixer_state.system_buffer.extend(mono);
        
        // Try to mix if both buffers have enough samples
        try_mix_and_process(&mut mixer_state, state, app_handle);
    }
}

/// Mix available samples from both buffers and send to processing
fn try_mix_and_process(
    mixer: &mut MixerState,
    state: &Arc<Mutex<AudioStreamState>>,
    app_handle: &AppHandle,
) {
    // Mix the minimum available samples from both buffers
    let mix_count = std::cmp::min(mixer.input_buffer.len(), mixer.system_buffer.len());
    
    if mix_count == 0 {
        return;
    }
    
    // Mix with 0.5 gain each to prevent clipping
    let mixed: Vec<f32> = mixer.input_buffer.iter()
        .zip(mixer.system_buffer.iter())
        .take(mix_count)
        .map(|(&input, &system)| (input * 0.5) + (system * 0.5))
        .collect();
    
    // Remove processed samples from buffers
    mixer.input_buffer.drain(0..mix_count);
    mixer.system_buffer.drain(0..mix_count);
    
    // Process the mixed audio (already mono)
    if let Ok(mut audio_state) = state.try_lock() {
        // Record samples if recording
        if audio_state.is_recording {
            audio_state.recording_samples.extend(&mixed);
        }

        // Run visualization processor if monitoring
        if audio_state.is_monitoring {
            if let Some(ref mut viz_processor) = audio_state.visualization_processor {
                viz_processor.process(&mixed, app_handle);
            }
        }

        // Run speech processor if enabled
        if audio_state.is_monitoring && audio_state.is_processing_enabled {
            if let Some(ref mut processor) = audio_state.speech_processor {
                processor.process(&mixed, app_handle);
            }
        }
    }
}

/// Process samples for both recording and visualization
fn process_audio_samples(
    samples: &[f32],
    channels: usize,
    state: &Arc<Mutex<AudioStreamState>>,
    app_handle: &AppHandle,
) {
    // Try to lock without blocking - if we can't get the lock, skip this batch
    if let Ok(mut audio_state) = state.try_lock() {
        // Record samples if recording
        if audio_state.is_recording {
            audio_state.recording_samples.extend_from_slice(samples);
        }

        // Convert to mono if needed (used for visualization and processing)
        let mono_samples: Vec<f32> = if channels > 1 {
            samples
                .chunks(channels)
                .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
                .collect()
        } else {
            samples.to_vec()
        };

        // Run visualization processor if monitoring (always runs, independent of processing toggle)
        if audio_state.is_monitoring {
            if let Some(ref mut viz_processor) = audio_state.visualization_processor {
                viz_processor.process(&mono_samples, app_handle);
            }
        }

        // Run speech processor if enabled and monitoring is active
        if audio_state.is_monitoring && audio_state.is_processing_enabled {
            if let Some(ref mut processor) = audio_state.speech_processor {
                processor.process(&mono_samples, app_handle);
            }
        }
    }
}

/// Default output height for spectrogram (matches frontend canvas)
const SPECTROGRAM_HEIGHT: usize = 256;

/// Start monitoring audio (visualization only)
pub fn start_monitor(
    device_id: &str,
    source_type: AudioSourceType,
    system_device_id: Option<&str>,
    state: &RecordingState,
    app_handle: AppHandle,
) -> Result<(), String> {
    {
        let audio_state = state.state.lock().unwrap();
        if audio_state.is_monitoring {
            return Err("Already monitoring".to_string());
        }
    }

    // Ensure stream is running based on source type
    match source_type {
        AudioSourceType::Input | AudioSourceType::System => {
            ensure_stream_running(device_id, state, app_handle)?;
        }
        AudioSourceType::Mixed => {
            let system_id = system_device_id.ok_or("System device ID required for Mixed mode")?;
            ensure_mixed_streams_running(device_id, system_id, state, app_handle)?;
        }
    }

    // Enable monitoring and create visualization processor
    {
        let mut audio_state = state.state.lock().unwrap();
        let sample_rate = audio_state.sample_rate;
        audio_state.visualization_processor = Some(VisualizationProcessor::new(sample_rate, SPECTROGRAM_HEIGHT));
        audio_state.is_monitoring = true;
        audio_state.source_type = source_type;
    }

    Ok(())
}

/// Stop monitoring
pub fn stop_monitor(state: &RecordingState) -> Result<(), String> {
    let source_type = {
        let mut audio_state = state.state.lock().unwrap();
        audio_state.is_monitoring = false;
        audio_state.visualization_processor = None;
        audio_state.source_type
    };

    // Stop stream if nothing else needs it
    maybe_stop_stream(state);
    
    // Also stop secondary stream for Mixed mode
    if source_type == AudioSourceType::Mixed {
        maybe_stop_secondary_stream(state);
    }

    Ok(())
}

/// Start recording (also enables visualization if not already monitoring)
pub fn start_recording(
    device_id: &str,
    source_type: AudioSourceType,
    system_device_id: Option<&str>,
    state: &RecordingState,
    app_handle: AppHandle,
) -> Result<(), String> {
    {
        let audio_state = state.state.lock().unwrap();
        if audio_state.is_recording {
            return Err("Already recording".to_string());
        }
    }

    // Ensure stream is running based on source type
    match source_type {
        AudioSourceType::Input | AudioSourceType::System => {
            ensure_stream_running(device_id, state, app_handle)?;
        }
        AudioSourceType::Mixed => {
            let system_id = system_device_id.ok_or("System device ID required for Mixed mode")?;
            ensure_mixed_streams_running(device_id, system_id, state, app_handle)?;
        }
    }

    // Enable recording (and monitoring if not already)
    {
        let mut audio_state = state.state.lock().unwrap();
        audio_state.recording_samples.clear();
        audio_state.is_recording = true;
        audio_state.source_type = source_type;
        // Also enable monitoring for visualization during recording
        if !audio_state.is_monitoring {
            let sample_rate = audio_state.sample_rate;
            audio_state.visualization_processor = Some(VisualizationProcessor::new(sample_rate, SPECTROGRAM_HEIGHT));
            audio_state.is_monitoring = true;
        }
    }

    Ok(())
}

/// Stop recording and extract raw audio samples (fast, non-blocking)
/// Returns the raw samples that need to be processed separately
pub fn stop_recording(state: &RecordingState, keep_monitoring: bool) -> Result<RawRecordedAudio, String> {
    // Extract recorded audio and stop recording - this is fast
    let (samples, sample_rate, channels) = {
        let mut audio_state = state.state.lock().unwrap();
        audio_state.is_recording = false;
        
        // If not keeping monitoring, stop it now
        if !keep_monitoring {
            audio_state.is_monitoring = false;
            audio_state.visualization_processor = None;
        }
        
        let samples = std::mem::take(&mut audio_state.recording_samples);
        (samples, audio_state.sample_rate, audio_state.channels)
    };

    // Stop stream if nothing else needs it (non-blocking)
    maybe_stop_stream(state);

    if samples.is_empty() {
        return Err("No audio recorded".to_string());
    }

    Ok(RawRecordedAudio {
        samples,
        sample_rate,
        channels,
    })
}

/// Process raw recorded audio into format suitable for transcription
/// This is CPU-intensive and should be called in a separate thread/task
pub fn process_recorded_audio(raw: RawRecordedAudio) -> Result<Vec<f32>, String> {
    // Convert to mono if stereo
    let mono_samples = if raw.channels > 1 {
        convert_to_mono(&raw.samples, raw.channels as usize)
    } else {
        raw.samples
    };

    // Resample to 16kHz for Whisper
    resample_to_16khz(&mono_samples, raw.sample_rate)
}

fn convert_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    samples
        .chunks(channels)
        .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
        .collect()
}

fn resample_to_16khz(samples: &[f32], source_rate: u32) -> Result<Vec<f32>, String> {
    const TARGET_RATE: u32 = 16000;

    if source_rate == TARGET_RATE {
        return Ok(samples.to_vec());
    }

    let chunk_size = 1024;
    let mut resampler = FftFixedIn::<f32>::new(
        source_rate as usize,
        TARGET_RATE as usize,
        chunk_size,
        2,
        1, // mono
    )
    .map_err(|e| format!("Failed to create resampler: {}", e))?;

    let mut output = Vec::new();

    for chunk in samples.chunks(chunk_size) {
        let mut padded_chunk = chunk.to_vec();
        // Pad last chunk if needed
        if padded_chunk.len() < chunk_size {
            padded_chunk.resize(chunk_size, 0.0);
        }

        let input = vec![padded_chunk];
        let result = resampler
            .process(&input, None)
            .map_err(|e| format!("Resample error: {}", e))?;

        if !result.is_empty() {
            output.extend(&result[0]);
        }
    }

    Ok(output)
}
