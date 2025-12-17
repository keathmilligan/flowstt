use rustfft::{num_complex::Complex, FftPlanner};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// Audio processor trait for extensible audio analysis.
/// Processors must be fast and non-blocking as they run in the audio callback.
pub trait AudioProcessor: Send {
    /// Process a batch of audio samples.
    /// Samples are mono f32 values, typically in the range [-1.0, 1.0].
    /// The AppHandle can be used to emit events to the frontend.
    fn process(&mut self, samples: &[f32], app_handle: &AppHandle);
}

/// Event payload for speech detection events
#[derive(Clone, Serialize)]
pub struct SpeechEventPayload {
    /// Duration in milliseconds (for speech-ended: how long the speech lasted)
    pub duration_ms: Option<u64>,
}

/// Speech detection metrics for visualization
#[derive(Clone, Serialize)]
pub struct SpeechMetrics {
    /// RMS amplitude in decibels
    pub amplitude_db: f32,
    /// Zero-crossing rate (0.0 to 0.5)
    pub zcr: f32,
    /// Estimated spectral centroid in Hz
    pub centroid_hz: f32,
    /// Whether speech is currently detected
    pub is_speaking: bool,
    /// Whether voiced speech onset is pending
    pub is_voiced_pending: bool,
    /// Whether whisper speech onset is pending
    pub is_whisper_pending: bool,
    /// Whether current frame is classified as transient
    pub is_transient: bool,
}

/// Configuration for a speech detection mode (voiced or whisper)
#[derive(Clone)]
struct SpeechModeConfig {
    /// Minimum amplitude threshold in dB
    threshold_db: f32,
    /// ZCR range (min, max) - normalized as crossings per sample
    zcr_range: (f32, f32),
    /// Spectral centroid range in Hz (min, max)
    centroid_range: (f32, f32),
    /// Onset time in samples before confirming speech
    onset_samples: u32,
}

/// Speech detector that emits events when speech starts and ends.
/// 
/// Uses multi-feature analysis for robust speech detection:
/// - RMS amplitude for basic energy detection
/// - Zero-Crossing Rate (ZCR) to distinguish voiced speech from transients
/// - Spectral centroid approximation to identify speech-band frequency content
/// 
/// Implements dual-mode detection:
/// - **Voiced mode**: For normal speech (lower ZCR, speech-band centroid)
/// - **Whisper mode**: For soft/breathy speech (higher ZCR, broader centroid range)
/// 
/// Explicit transient rejection filters keyboard clicks and similar impulsive sounds.
pub struct SpeechDetector {
    /// Sample rate for time/frequency calculations
    sample_rate: u32,
    /// Voiced speech detection configuration
    voiced_config: SpeechModeConfig,
    /// Whisper speech detection configuration  
    whisper_config: SpeechModeConfig,
    /// Transient rejection: ZCR threshold (reject if above)
    transient_zcr_threshold: f32,
    /// Transient rejection: centroid threshold in Hz (reject if above, combined with ZCR)
    transient_centroid_threshold: f32,
    /// Hold time in samples before emitting speech-ended event
    hold_samples: u32,
    /// Current speech state (true = speaking, false = silent)
    is_speaking: bool,
    /// Whether we're in "pending voiced" state
    is_pending_voiced: bool,
    /// Whether we're in "pending whisper" state
    is_pending_whisper: bool,
    /// Counter for voiced onset time
    voiced_onset_count: u32,
    /// Counter for whisper onset time
    whisper_onset_count: u32,
    /// Counter for hold time during silence
    silence_sample_count: u32,
    /// Counter for speech duration (from confirmed start)
    speech_sample_count: u64,
    /// Whether we've initialized (first sample processed)
    initialized: bool,
    /// Last computed amplitude in dB (for metrics)
    last_amplitude_db: f32,
    /// Last computed ZCR (for metrics)
    last_zcr: f32,
    /// Last computed spectral centroid in Hz (for metrics)
    last_centroid_hz: f32,
    /// Whether last frame was classified as transient (for metrics)
    last_is_transient: bool,
}

impl SpeechDetector {
    /// Create a new speech detector with specified sample rate.
    /// Uses default dual-mode configuration optimized for speech detection.
    pub fn new(sample_rate: u32) -> Self {
        Self::with_defaults(sample_rate)
    }

    /// Create a speech detector with default dual-mode configuration.
    /// 
    /// Default parameters:
    /// - Voiced mode: -40dB threshold, ZCR 0.01-0.20, centroid 250-4000Hz, 100ms onset
    /// - Whisper mode: -50dB threshold, ZCR 0.10-0.40, centroid 400-6000Hz, 150ms onset
    /// - Transient rejection: ZCR > 0.40 AND centroid > 5500Hz
    /// - Hold time: 300ms
    pub fn with_defaults(sample_rate: u32) -> Self {
        let hold_samples = (sample_rate as u64 * 300 / 1000) as u32;
        
        Self {
            sample_rate,
            voiced_config: SpeechModeConfig {
                threshold_db: -40.0,
                zcr_range: (0.01, 0.20),
                centroid_range: (250.0, 4000.0),
                onset_samples: (sample_rate as u64 * 100 / 1000) as u32,
            },
            whisper_config: SpeechModeConfig {
                threshold_db: -50.0,
                zcr_range: (0.10, 0.40),
                centroid_range: (400.0, 6000.0),
                onset_samples: (sample_rate as u64 * 150 / 1000) as u32,
            },
            transient_zcr_threshold: 0.40,
            transient_centroid_threshold: 5500.0,
            hold_samples,
            is_speaking: false,
            is_pending_voiced: false,
            is_pending_whisper: false,
            voiced_onset_count: 0,
            whisper_onset_count: 0,
            silence_sample_count: 0,
            speech_sample_count: 0,
            initialized: false,
            last_amplitude_db: f32::NEG_INFINITY,
            last_zcr: 0.0,
            last_centroid_hz: 0.0,
            last_is_transient: false,
        }
    }

    /// Calculate RMS amplitude of samples
    fn calculate_rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_squares: f32 = samples.iter().map(|s| s * s).sum();
        (sum_squares / samples.len() as f32).sqrt()
    }

    /// Convert linear amplitude to decibels
    fn amplitude_to_db(amplitude: f32) -> f32 {
        if amplitude <= 0.0 {
            return f32::NEG_INFINITY;
        }
        20.0 * amplitude.log10()
    }

    /// Calculate Zero-Crossing Rate (ZCR) of samples.
    /// 
    /// ZCR is the rate at which the signal changes sign (crosses zero).
    /// Returns normalized value: crossings per sample (0.0 to 0.5 max).
    /// 
    /// Acoustic characteristics:
    /// - Voiced speech: ~0.02-0.15 (periodic vocal cord vibration)
    /// - Whispered speech: ~0.15-0.35 (breathy, more noise-like)
    /// - Clicks/transients: >0.35 (impulsive, high-frequency content)
    /// - Low rumble: <0.01 (slow oscillation)
    fn calculate_zcr(samples: &[f32]) -> f32 {
        if samples.len() < 2 {
            return 0.0;
        }
        
        let mut crossings = 0u32;
        for i in 1..samples.len() {
            // Count sign changes (one sample positive, next negative or vice versa)
            if (samples[i] >= 0.0) != (samples[i - 1] >= 0.0) {
                crossings += 1;
            }
        }
        
        // Normalize: crossings per sample
        crossings as f32 / (samples.len() - 1) as f32
    }

    /// Estimate spectral centroid using first-difference approximation.
    /// 
    /// This provides a frequency estimate without FFT by using:
    /// centroid ≈ sample_rate * mean(|diff(samples)|) / (2 * mean(|samples|))
    /// 
    /// The intuition: higher frequency signals have larger sample-to-sample
    /// differences relative to their amplitude.
    /// 
    /// Returns frequency estimate in Hz. Returns 0.0 when signal is too quiet
    /// (below -55dB) to avoid noise-induced jitter.
    /// 
    /// Acoustic characteristics:
    /// - Voiced speech: ~300-3500 Hz (fundamental + harmonics)
    /// - Whispered speech: ~500-5000 Hz (shifted up, more fricative)
    /// - Keyboard clicks: >5000 Hz (high-frequency transient)
    /// - Low rumble: <200 Hz
    fn estimate_spectral_centroid(&self, samples: &[f32], amplitude_db: f32) -> f32 {
        // Gate: don't compute centroid for very quiet signals (avoids jitter in silence)
        const CENTROID_GATE_DB: f32 = -55.0;
        if samples.len() < 2 || amplitude_db < CENTROID_GATE_DB {
            return 0.0;
        }
        
        // Calculate mean absolute difference (approximates high-frequency content)
        let mut diff_sum = 0.0f32;
        for i in 1..samples.len() {
            diff_sum += (samples[i] - samples[i - 1]).abs();
        }
        let mean_diff = diff_sum / (samples.len() - 1) as f32;
        
        // Calculate mean absolute amplitude
        let mean_abs: f32 = samples.iter().map(|s| s.abs()).sum::<f32>() / samples.len() as f32;
        
        // Avoid division by zero
        if mean_abs < 1e-10 {
            return 0.0;
        }
        
        // Approximate centroid frequency
        // Factor of 2 comes from Nyquist relationship
        self.sample_rate as f32 * mean_diff / (2.0 * mean_abs)
    }

    /// Check if features indicate a transient sound (keyboard click, etc.)
    /// Transients have both high ZCR AND high spectral centroid.
    fn is_transient(&self, zcr: f32, centroid: f32) -> bool {
        zcr > self.transient_zcr_threshold && centroid > self.transient_centroid_threshold
    }

    /// Check if features match voiced speech mode
    fn matches_voiced_mode(&self, db: f32, zcr: f32, centroid: f32) -> bool {
        db >= self.voiced_config.threshold_db
            && zcr >= self.voiced_config.zcr_range.0
            && zcr <= self.voiced_config.zcr_range.1
            && centroid >= self.voiced_config.centroid_range.0
            && centroid <= self.voiced_config.centroid_range.1
    }

    /// Check if features match whisper speech mode
    fn matches_whisper_mode(&self, db: f32, zcr: f32, centroid: f32) -> bool {
        db >= self.whisper_config.threshold_db
            && zcr >= self.whisper_config.zcr_range.0
            && zcr <= self.whisper_config.zcr_range.1
            && centroid >= self.whisper_config.centroid_range.0
            && centroid <= self.whisper_config.centroid_range.1
    }

    /// Convert sample count to milliseconds
    fn samples_to_ms(&self, samples: u64) -> u64 {
        samples * 1000 / self.sample_rate as u64
    }

    /// Reset all onset tracking state
    fn reset_onset_state(&mut self) {
        self.is_pending_voiced = false;
        self.is_pending_whisper = false;
        self.voiced_onset_count = 0;
        self.whisper_onset_count = 0;
    }

    /// Get the current speech detection metrics.
    /// Call this after process() to get the latest computed values.
    pub fn get_metrics(&self) -> SpeechMetrics {
        SpeechMetrics {
            amplitude_db: self.last_amplitude_db,
            zcr: self.last_zcr,
            centroid_hz: self.last_centroid_hz,
            is_speaking: self.is_speaking,
            is_voiced_pending: self.is_pending_voiced,
            is_whisper_pending: self.is_pending_whisper,
            is_transient: self.last_is_transient,
        }
    }
}

impl AudioProcessor for SpeechDetector {
    fn process(&mut self, samples: &[f32], app_handle: &AppHandle) {
        // Step 1: Calculate all features
        let rms = Self::calculate_rms(samples);
        let db = Self::amplitude_to_db(rms);
        let zcr = Self::calculate_zcr(samples);
        let centroid = self.estimate_spectral_centroid(samples, db);

        // Store metrics for later retrieval
        self.last_amplitude_db = db;
        self.last_zcr = zcr;
        self.last_centroid_hz = centroid;
        self.last_is_transient = self.is_transient(zcr, centroid);

        if !self.initialized {
            self.initialized = true;
            // Don't emit on first sample - wait for proper onset
            return;
        }

        // Step 2: Check for transient rejection (keyboard clicks, etc.)
        // Transients have both high ZCR AND high centroid
        if self.last_is_transient {
            // Reset onset timers - transient breaks any pending speech detection
            self.reset_onset_state();
            // Don't affect confirmed speech within hold time
            if !self.is_speaking {
                return;
            }
        }

        // Step 3: Check feature matching for both modes
        let is_voiced = self.matches_voiced_mode(db, zcr, centroid);
        let is_whisper = self.matches_whisper_mode(db, zcr, centroid);
        let is_speech_candidate = is_voiced || is_whisper;

        if is_speech_candidate {
            // Sound matching speech features detected
            self.silence_sample_count = 0;

            if self.is_speaking {
                // Continue confirmed speech
                self.speech_sample_count += samples.len() as u64;
            } else {
                // Handle onset accumulation based on which mode matches
                let samples_len = samples.len() as u32;

                if is_voiced {
                    // Accumulate voiced onset
                    if !self.is_pending_voiced {
                        self.is_pending_voiced = true;
                        self.voiced_onset_count = samples_len;
                    } else {
                        self.voiced_onset_count += samples_len;
                    }

                    // Check if voiced onset threshold reached
                    if self.voiced_onset_count >= self.voiced_config.onset_samples {
                        self.is_speaking = true;
                        self.speech_sample_count = self.voiced_onset_count as u64;
                        self.reset_onset_state();
                        let _ = app_handle.emit("speech-started", SpeechEventPayload { duration_ms: None });
                        println!("[SpeechDetector] Speech started (voiced mode)");
                        return;
                    }
                }

                if is_whisper {
                    // Accumulate whisper onset (can run in parallel with voiced)
                    if !self.is_pending_whisper {
                        self.is_pending_whisper = true;
                        self.whisper_onset_count = samples_len;
                    } else {
                        self.whisper_onset_count += samples_len;
                    }

                    // Check if whisper onset threshold reached (and voiced didn't already trigger)
                    if !self.is_speaking && self.whisper_onset_count >= self.whisper_config.onset_samples {
                        self.is_speaking = true;
                        self.speech_sample_count = self.whisper_onset_count as u64;
                        self.reset_onset_state();
                        let _ = app_handle.emit("speech-started", SpeechEventPayload { duration_ms: None });
                        println!("[SpeechDetector] Speech started (whisper mode)");
                    }
                }
            }
        } else {
            // No speech-like features detected
            if self.is_pending_voiced || self.is_pending_whisper {
                // Cancel pending speech - features didn't persist
                self.reset_onset_state();
            }
            
            if self.is_speaking {
                self.silence_sample_count += samples.len() as u32;

                // Check if hold time has elapsed
                if self.silence_sample_count >= self.hold_samples {
                    // Emit speech-ended with duration
                    let duration_ms = self.samples_to_ms(self.speech_sample_count);
                    self.is_speaking = false;
                    self.speech_sample_count = 0;
                    let _ = app_handle.emit("speech-ended", SpeechEventPayload { duration_ms: Some(duration_ms) });
                    println!("[SpeechDetector] Speech ended (duration: {}ms)", duration_ms);
                }
            }
        }
    }

}

// ============================================================================
// Visualization Processor
// ============================================================================

/// Payload for visualization data events
#[derive(Clone, Serialize)]
pub struct VisualizationPayload {
    /// Pre-downsampled waveform amplitudes
    pub waveform: Vec<f32>,
    /// Spectrogram column with RGB colors (present when FFT buffer fills)
    pub spectrogram: Option<SpectrogramColumn>,
    /// Speech detection metrics (present when speech processor is active)
    pub speech_metrics: Option<SpeechMetrics>,
}

/// A single column of spectrogram data ready for rendering
#[derive(Clone, Serialize)]
pub struct SpectrogramColumn {
    /// RGB triplets for each pixel row (height * 3 bytes)
    pub colors: Vec<u8>,
}

/// Color stop for gradient interpolation
struct ColorStop {
    position: f32,
    r: u8,
    g: u8,
    b: u8,
}

/// Visualization processor that computes render-ready waveform and spectrogram data.
/// 
/// This processor:
/// - Downsamples audio for waveform display using peak detection
/// - Computes FFT for frequency analysis
/// - Maps frequency magnitudes to colors using a heat map gradient
/// - Emits visualization-data events with pre-computed render data
pub struct VisualizationProcessor {
    /// Sample rate for frequency calculations
    sample_rate: u32,
    /// Target height for spectrogram output (pixels)
    output_height: usize,
    /// FFT size (must be power of 2)
    fft_size: usize,
    /// FFT planner/executor
    fft: Arc<dyn rustfft::Fft<f32>>,
    /// Pre-computed Hanning window
    hanning_window: Vec<f32>,
    /// Buffer for accumulating samples for FFT
    fft_buffer: Vec<f32>,
    /// Current write position in FFT buffer
    fft_write_index: usize,
    /// Pre-computed color lookup table (256 entries, RGB)
    color_lut: Vec<[u8; 3]>,
    /// Waveform accumulator for downsampling
    waveform_buffer: Vec<f32>,
    /// Target waveform output samples per emit
    waveform_target_samples: usize,
    /// Speech metrics to include in next visualization event
    pending_speech_metrics: Option<SpeechMetrics>,
}

impl VisualizationProcessor {
    /// Create a new visualization processor
    /// 
    /// # Arguments
    /// * `sample_rate` - Audio sample rate in Hz (typically 48000)
    /// * `output_height` - Target pixel height for spectrogram columns
    pub fn new(sample_rate: u32, output_height: usize) -> Self {
        let fft_size = 512;
        
        // Create FFT planner
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);
        
        // Pre-compute Hanning window
        let hanning_window: Vec<f32> = (0..fft_size)
            .map(|i| {
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (fft_size - 1) as f32).cos())
            })
            .collect();
        
        // Build color lookup table
        let color_lut = Self::build_color_lut();
        
        Self {
            sample_rate,
            output_height,
            fft_size,
            fft,
            hanning_window,
            fft_buffer: Vec::with_capacity(fft_size),
            fft_write_index: 0,
            color_lut,
            waveform_buffer: Vec::with_capacity(256),
            waveform_target_samples: 64, // Output ~64 samples per batch for smooth waveform
            pending_speech_metrics: None,
        }
    }

    /// Set speech metrics to include in the next visualization event
    pub fn set_speech_metrics(&mut self, metrics: SpeechMetrics) {
        self.pending_speech_metrics = Some(metrics);
    }
    
    /// Build the color lookup table matching the frontend gradient
    /// Gradient: dark blue -> blue -> cyan -> yellow-green -> orange -> red
    fn build_color_lut() -> Vec<[u8; 3]> {
        let stops = [
            ColorStop { position: 0.00, r: 10, g: 15, b: 26 },    // Background #0a0f1a
            ColorStop { position: 0.15, r: 0, g: 50, b: 200 },    // Blue
            ColorStop { position: 0.35, r: 0, g: 255, b: 150 },   // Cyan
            ColorStop { position: 0.60, r: 200, g: 255, b: 0 },   // Yellow-green
            ColorStop { position: 0.80, r: 255, g: 155, b: 0 },   // Orange
            ColorStop { position: 1.00, r: 255, g: 0, b: 0 },     // Red
        ];
        
        let mut lut = Vec::with_capacity(256);
        
        for i in 0..256 {
            let t_raw = i as f32 / 255.0;
            // Apply gamma for better visual spread (matching frontend)
            let t = t_raw.powf(0.7);
            
            // Find which segment we're in and interpolate
            let mut color = [255u8, 0, 0]; // Fallback to red
            
            for j in 0..stops.len() - 1 {
                let s1 = &stops[j];
                let s2 = &stops[j + 1];
                
                if t >= s1.position && t <= s2.position {
                    let s = (t - s1.position) / (s2.position - s1.position);
                    color[0] = (s1.r as f32 + s * (s2.r as f32 - s1.r as f32)).round() as u8;
                    color[1] = (s1.g as f32 + s * (s2.g as f32 - s1.g as f32)).round() as u8;
                    color[2] = (s1.b as f32 + s * (s2.b as f32 - s1.b as f32)).round() as u8;
                    break;
                }
            }
            
            lut.push(color);
        }
        
        lut
    }
    
    /// Convert normalized position (0-1) to fractional frequency bin using log scale
    fn position_to_freq_bin(&self, pos: f32, num_bins: usize) -> f32 {
        const MIN_FREQ: f32 = 20.0;    // 20 Hz minimum (human hearing)
        const MAX_FREQ: f32 = 24000.0; // 24 kHz (Nyquist at 48kHz)
        
        let min_log = MIN_FREQ.log10();
        let max_log = MAX_FREQ.log10();
        
        // Log interpolation
        let log_freq = min_log + pos * (max_log - min_log);
        let freq = 10.0f32.powf(log_freq);
        
        // Convert frequency to bin index
        // bin = freq / (sample_rate / fft_size) = freq * fft_size / sample_rate
        let bin_index = freq * self.fft_size as f32 / self.sample_rate as f32;
        bin_index.clamp(0.0, (num_bins - 1) as f32)
    }
    
    /// Get magnitude for a pixel row, with interpolation/averaging
    fn get_magnitude_for_pixel(&self, magnitudes: &[f32], y: usize, height: usize) -> f32 {
        let num_bins = magnitudes.len();
        
        // Get frequency range for this pixel (y=0 is top = high freq, y=height-1 is bottom = low freq)
        let pos1 = (height - 1 - y) as f32 / height as f32;
        let pos2 = (height - y) as f32 / height as f32;
        
        let bin1 = self.position_to_freq_bin(pos1, num_bins);
        let bin2 = self.position_to_freq_bin(pos2, num_bins);
        
        let bin_low = bin1.min(bin2).max(0.0);
        let bin_high = bin1.max(bin2).min((num_bins - 1) as f32);
        
        // If range spans less than one bin, interpolate
        if bin_high - bin_low < 1.0 {
            let bin_floor = bin_low.floor() as usize;
            let bin_ceil = (bin_floor + 1).min(num_bins - 1);
            let frac = bin_low - bin_floor as f32;
            return magnitudes[bin_floor] * (1.0 - frac) + magnitudes[bin_ceil] * frac;
        }
        
        // Otherwise, average all bins in range (weighted by overlap)
        let mut sum = 0.0f32;
        let mut weight = 0.0f32;
        
        let start_bin = bin_low.floor() as usize;
        let end_bin = bin_high.ceil() as usize;
        
        #[allow(clippy::needless_range_loop)]
        for b in start_bin..=end_bin.min(num_bins - 1) {
            let bin_start = b as f32;
            let bin_end = (b + 1) as f32;
            let overlap_start = bin_low.max(bin_start);
            let overlap_end = bin_high.min(bin_end);
            let overlap_weight = (overlap_end - overlap_start).max(0.0);
            
            if overlap_weight > 0.0 {
                sum += magnitudes[b] * overlap_weight;
                weight += overlap_weight;
            }
        }
        
        if weight > 0.0 { sum / weight } else { 0.0 }
    }
    
    /// Process FFT buffer and generate spectrogram column
    fn process_fft(&self) -> SpectrogramColumn {
        // Apply Hanning window and prepare complex buffer
        let mut complex_buffer: Vec<Complex<f32>> = self.fft_buffer
            .iter()
            .zip(self.hanning_window.iter())
            .map(|(&sample, &window)| Complex::new(sample * window, 0.0))
            .collect();
        
        // Pad if needed (shouldn't happen, but safety)
        complex_buffer.resize(self.fft_size, Complex::new(0.0, 0.0));
        
        // Perform FFT
        self.fft.process(&mut complex_buffer);
        
        // Compute magnitudes (only positive frequencies, first half)
        let num_bins = self.fft_size / 2;
        let magnitudes: Vec<f32> = complex_buffer[..num_bins]
            .iter()
            .map(|c| (c.re * c.re + c.im * c.im).sqrt() / self.fft_size as f32)
            .collect();
        
        // Find max magnitude for normalization
        let max_mag = magnitudes.iter().cloned().fold(0.001f32, f32::max);
        let ref_level = max_mag.max(0.05);
        
        // Generate colors for each pixel row
        let mut colors = Vec::with_capacity(self.output_height * 3);
        
        for y in 0..self.output_height {
            let magnitude = self.get_magnitude_for_pixel(&magnitudes, y, self.output_height);
            
            // Normalize with log scale (matching frontend)
            let normalized_db = (1.0 + magnitude / ref_level * 9.0).log10();
            let normalized = normalized_db.clamp(0.0, 1.0);
            
            // Look up color
            let color_idx = (normalized * 255.0).floor() as usize;
            let color = &self.color_lut[color_idx.min(255)];
            
            colors.push(color[0]);
            colors.push(color[1]);
            colors.push(color[2]);
        }
        
        SpectrogramColumn { colors }
    }
    
    /// Downsample waveform buffer using peak detection
    fn downsample_waveform(&self, samples: &[f32]) -> Vec<f32> {
        if samples.is_empty() {
            return Vec::new();
        }
        
        // Calculate window size to achieve target output samples
        let window_size = (samples.len() / self.waveform_target_samples).max(1);
        let output_count = samples.len().div_ceil(window_size);
        
        let mut output = Vec::with_capacity(output_count);
        
        for chunk in samples.chunks(window_size) {
            // Find peak (max absolute value) in this window, preserving sign
            let peak = chunk
                .iter()
                .max_by(|a, b| a.abs().partial_cmp(&b.abs()).unwrap())
                .copied()
                .unwrap_or(0.0);
            output.push(peak);
        }
        
        output
    }
}

impl AudioProcessor for VisualizationProcessor {
    fn process(&mut self, samples: &[f32], app_handle: &AppHandle) {
        // Accumulate samples for FFT
        for &sample in samples {
            if self.fft_write_index < self.fft_size {
                if self.fft_buffer.len() <= self.fft_write_index {
                    self.fft_buffer.push(sample);
                } else {
                    self.fft_buffer[self.fft_write_index] = sample;
                }
                self.fft_write_index += 1;
            }
        }
        
        // Accumulate samples for waveform
        self.waveform_buffer.extend_from_slice(samples);
        
        // Check if FFT buffer is full
        let spectrogram = if self.fft_write_index >= self.fft_size {
            let column = self.process_fft();
            self.fft_write_index = 0;
            Some(column)
        } else {
            None
        };
        
        // Downsample waveform
        let waveform = self.downsample_waveform(&self.waveform_buffer);
        self.waveform_buffer.clear();
        
        // Take speech metrics (will be None if not set)
        let speech_metrics = self.pending_speech_metrics.take();
        
        // Emit visualization data
        let payload = VisualizationPayload {
            waveform,
            spectrogram,
            speech_metrics,
        };
        
        let _ = app_handle.emit("visualization-data", payload);
    }
}
