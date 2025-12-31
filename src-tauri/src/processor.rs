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
    /// Lookback audio samples from true speech start (for speech-started only)
    pub lookback_samples: Option<Vec<f32>>,
    /// Lookback offset in milliseconds (how far back the true start was found)
    pub lookback_offset_ms: Option<u32>,
}

/// Event payload for word break detection events
#[derive(Clone, Serialize)]
pub struct WordBreakPayload {
    /// Timestamp offset in milliseconds from speech start
    pub offset_ms: u32,
    /// Duration of the detected gap in milliseconds
    pub gap_duration_ms: u32,
}

/// Speech state change detected by the speech detector
#[derive(Clone, Debug)]
pub enum SpeechStateChange {
    /// No change in speech state
    None,
    /// Speech started with lookback sample count
    Started { lookback_samples: usize },
    /// Speech ended with duration in milliseconds
    Ended { duration_ms: u64 },
}

/// Word break event detected during speech
#[derive(Clone, Debug)]
pub struct WordBreakEvent {
    /// Offset from speech start in milliseconds
    pub offset_ms: u32,
    /// Duration of the gap in milliseconds
    pub gap_duration_ms: u32,
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
    /// Whether this is lookback-determined speech (retroactively identified)
    pub is_lookback_speech: bool,
    /// Lookback offset in milliseconds (when speech was just confirmed)
    pub lookback_offset_ms: Option<u32>,
    /// Whether a word break (inter-word gap) is currently detected
    pub is_word_break: bool,
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
/// 
/// Includes lookback functionality to capture the true start of speech by maintaining
/// a ring buffer of recent audio samples and analyzing them retroactively.
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
    /// Grace samples allowed during onset (brief dips don't reset counters)
    onset_grace_samples: u32,
    /// Current grace counter for voiced onset
    voiced_grace_count: u32,
    /// Current grace counter for whisper onset
    whisper_grace_count: u32,
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
    
    // Lookback ring buffer fields
    /// Ring buffer for recent audio samples (for lookback analysis)
    lookback_buffer: Vec<f32>,
    /// Current write position in the ring buffer
    lookback_write_index: usize,
    /// Capacity of the lookback buffer in samples
    lookback_capacity: usize,
    /// Whether the lookback buffer has been filled at least once
    lookback_filled: bool,
    /// Lookback threshold in dB (more sensitive than detection threshold)
    lookback_threshold_db: f32,
    /// Last lookback offset in milliseconds (for metrics, set when speech confirmed)
    last_lookback_offset_ms: Option<u32>,
    /// Last state change detected during process() - for transcribe mode integration
    last_state_change: SpeechStateChange,
    
    // Word break detection fields
    /// Word break threshold ratio (amplitude must drop below this fraction of recent average)
    word_break_threshold_ratio: f32,
    /// Minimum gap duration in samples for word break (15ms)
    min_word_break_samples: u32,
    /// Maximum gap duration in samples for word break (200ms)
    max_word_break_samples: u32,
    /// Window size in samples for tracking recent speech amplitude (100ms)
    recent_speech_window_samples: u32,
    /// Running sum of recent speech amplitude (linear, not dB)
    recent_speech_amplitude_sum: f32,
    /// Count of samples in recent speech amplitude window
    recent_speech_amplitude_count: u32,
    /// Whether we're currently in a word break gap
    in_word_break: bool,
    /// Sample count of current word break gap
    word_break_sample_count: u32,
    /// Sample count at start of current word break (for offset calculation)
    word_break_start_speech_samples: u64,
    /// Whether last frame was a word break (for metrics)
    last_is_word_break: bool,
    /// Last word break event detected (for transcribe mode integration)
    last_word_break_event: Option<WordBreakEvent>,
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
    /// - Voiced mode: -42dB threshold, ZCR 0.01-0.30, centroid 200-5500Hz, 80ms onset
    /// - Whisper mode: -52dB threshold, ZCR 0.08-0.45, centroid 300-7000Hz, 120ms onset
    /// - Transient rejection: ZCR > 0.45 AND centroid > 6500Hz
    /// - Hold time: 300ms
    /// - Onset grace period: 30ms (brief dips in features don't reset onset counters)
    /// - Lookback buffer: 200ms (covers max onset time + margin)
    /// - Lookback threshold: -55dB (more sensitive to catch speech starts)
    pub fn with_defaults(sample_rate: u32) -> Self {
        let hold_samples = (sample_rate as u64 * 300 / 1000) as u32;
        // 200ms lookback buffer
        let lookback_capacity = (sample_rate as u64 * 200 / 1000) as usize;
        
        Self {
            sample_rate,
            voiced_config: SpeechModeConfig {
                threshold_db: -42.0,           // Slightly more sensitive
                zcr_range: (0.01, 0.30),       // Allow higher ZCR for fricatives in speech
                centroid_range: (200.0, 5500.0), // Wider range for varied speech
                onset_samples: (sample_rate as u64 * 80 / 1000) as u32, // Faster onset (80ms)
            },
            whisper_config: SpeechModeConfig {
                threshold_db: -52.0,           // Slightly more sensitive
                zcr_range: (0.08, 0.45),       // Broader ZCR range
                centroid_range: (300.0, 7000.0), // Wider range
                onset_samples: (sample_rate as u64 * 120 / 1000) as u32, // Faster onset (120ms)
            },
            transient_zcr_threshold: 0.45,      // Slightly higher to not reject breathy speech
            transient_centroid_threshold: 6500.0, // Higher to avoid rejecting high-pitched speech
            hold_samples,
            is_speaking: false,
            is_pending_voiced: false,
            is_pending_whisper: false,
            voiced_onset_count: 0,
            whisper_onset_count: 0,
            silence_sample_count: 0,
            speech_sample_count: 0,
            onset_grace_samples: (sample_rate as u64 * 30 / 1000) as u32, // 30ms grace period
            voiced_grace_count: 0,
            whisper_grace_count: 0,
            initialized: false,
            last_amplitude_db: f32::NEG_INFINITY,
            last_zcr: 0.0,
            last_centroid_hz: 0.0,
            last_is_transient: false,
            // Lookback buffer initialization
            lookback_buffer: vec![0.0; lookback_capacity],
            lookback_write_index: 0,
            lookback_capacity,
            lookback_filled: false,
            lookback_threshold_db: -55.0, // More sensitive than detection thresholds
            last_lookback_offset_ms: None,
            last_state_change: SpeechStateChange::None,
            
            // Word break detection initialization
            word_break_threshold_ratio: 0.5, // 50% of recent average
            min_word_break_samples: (sample_rate as u64 * 15 / 1000) as u32, // 15ms
            max_word_break_samples: (sample_rate as u64 * 200 / 1000) as u32, // 200ms
            recent_speech_window_samples: (sample_rate as u64 * 100 / 1000) as u32, // 100ms
            recent_speech_amplitude_sum: 0.0,
            recent_speech_amplitude_count: 0,
            in_word_break: false,
            word_break_sample_count: 0,
            word_break_start_speech_samples: 0,
            last_is_word_break: false,
            last_word_break_event: None,
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
        self.voiced_grace_count = 0;
        self.whisper_grace_count = 0;
    }

    /// Add samples to the lookback ring buffer
    fn push_to_lookback_buffer(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.lookback_buffer[self.lookback_write_index] = sample;
            self.lookback_write_index = (self.lookback_write_index + 1) % self.lookback_capacity;
            if self.lookback_write_index == 0 {
                self.lookback_filled = true;
            }
        }
    }

    /// Get the contents of the lookback buffer in chronological order (oldest to newest)
    fn get_lookback_buffer_contents(&self) -> Vec<f32> {
        if !self.lookback_filled {
            // Buffer hasn't wrapped yet, return from start to write index
            return self.lookback_buffer[..self.lookback_write_index].to_vec();
        }
        // Buffer has wrapped, return in chronological order
        let mut result = Vec::with_capacity(self.lookback_capacity);
        // Second part (older samples): from write_index to end
        result.extend_from_slice(&self.lookback_buffer[self.lookback_write_index..]);
        // First part (newer samples): from start to write_index
        result.extend_from_slice(&self.lookback_buffer[..self.lookback_write_index]);
        result
    }

    /// Find the true start of speech by scanning backward through the lookback buffer.
    /// Returns (lookback_samples, lookback_offset_ms) where:
    /// - lookback_samples: Audio from the true start to current position
    /// - lookback_offset_ms: How far back the true start was found
    fn find_lookback_start(&self) -> (Vec<f32>, u32) {
        let buffer = self.get_lookback_buffer_contents();
        if buffer.is_empty() {
            return (Vec::new(), 0);
        }

        // Scan backward from the end to find where amplitude first exceeded threshold
        // Use small chunks for finer resolution (128 samples ≈ 2.7ms at 48kHz)
        const CHUNK_SIZE: usize = 128;
        // Add margin before detected start to catch the very beginning (20ms)
        let margin_samples = (self.sample_rate as usize * 20) / 1000;
        let threshold_linear = 10.0f32.powf(self.lookback_threshold_db / 20.0);
        
        let mut first_above_threshold_idx = buffer.len(); // Default to end if nothing found
        
        // Scan from end to beginning in chunks
        let mut pos = buffer.len();
        while pos > 0 {
            let chunk_start = pos.saturating_sub(CHUNK_SIZE);
            let chunk = &buffer[chunk_start..pos];
            
            // Use peak amplitude instead of RMS for more sensitivity to transients
            let peak = chunk.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            
            if peak >= threshold_linear {
                // This chunk has audio above threshold
                // Continue scanning to find where it started
                first_above_threshold_idx = chunk_start;
            } else if first_above_threshold_idx < buffer.len() {
                // We found a quiet region after finding speech, stop here
                break;
            }
            
            pos = chunk_start;
        }
        
        // Apply margin: go back further to catch the very beginning
        let start_with_margin = first_above_threshold_idx.saturating_sub(margin_samples);
        
        // Extract samples from the found start point (with margin) to the end
        let lookback_samples = buffer[start_with_margin..].to_vec();
        
        // Calculate offset in milliseconds
        let samples_before = buffer.len() - start_with_margin;
        let offset_ms = (samples_before as u64 * 1000 / self.sample_rate as u64) as u32;
        
        (lookback_samples, offset_ms)
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
            is_lookback_speech: false, // Will be set by visualization layer based on delay buffer
            lookback_offset_ms: self.last_lookback_offset_ms,
            is_word_break: self.last_is_word_break,
        }
    }

    /// Get the last speech state change detected during process().
    /// Returns the state change and resets it to None for the next call.
    pub fn take_state_change(&mut self) -> SpeechStateChange {
        std::mem::replace(&mut self.last_state_change, SpeechStateChange::None)
    }

    /// Get the last speech state change without resetting it.
    pub fn peek_state_change(&self) -> &SpeechStateChange {
        &self.last_state_change
    }

    /// Take the last word break event, resetting it to None.
    pub fn take_word_break_event(&mut self) -> Option<WordBreakEvent> {
        self.last_word_break_event.take()
    }

    /// Peek at the last word break event without taking it.
    pub fn peek_word_break_event(&self) -> Option<&WordBreakEvent> {
        self.last_word_break_event.as_ref()
    }
    
    /// Update the running average of speech amplitude (call only during confirmed speech)
    fn update_speech_amplitude_average(&mut self, rms: f32, sample_count: u32) {
        self.recent_speech_amplitude_sum += rms * sample_count as f32;
        self.recent_speech_amplitude_count += sample_count;
        
        // If we've exceeded the window, scale down to approximate sliding window
        if self.recent_speech_amplitude_count > self.recent_speech_window_samples {
            let scale = self.recent_speech_window_samples as f32 / self.recent_speech_amplitude_count as f32;
            self.recent_speech_amplitude_sum *= scale;
            self.recent_speech_amplitude_count = self.recent_speech_window_samples;
        }
    }
    
    /// Get the recent average speech amplitude (linear)
    fn get_recent_speech_amplitude(&self) -> f32 {
        if self.recent_speech_amplitude_count == 0 {
            return 0.0;
        }
        self.recent_speech_amplitude_sum / self.recent_speech_amplitude_count as f32
    }
    
    /// Reset word break detection state (call when speech ends)
    fn reset_word_break_state(&mut self) {
        self.in_word_break = false;
        self.word_break_sample_count = 0;
        self.word_break_start_speech_samples = 0;
        self.recent_speech_amplitude_sum = 0.0;
        self.recent_speech_amplitude_count = 0;
        self.last_is_word_break = false;
        self.last_word_break_event = None;
    }
}

impl AudioProcessor for SpeechDetector {
    fn process(&mut self, samples: &[f32], app_handle: &AppHandle) {
        // Reset state change at start of each process call
        self.last_state_change = SpeechStateChange::None;
        // Reset word break event (will be set if a word break is detected this frame)
        self.last_word_break_event = None;
        
        // Step 0: Add samples to lookback buffer (always, for retroactive analysis)
        self.push_to_lookback_buffer(samples);
        
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
        // Clear lookback offset by default (only set when speech is confirmed)
        self.last_lookback_offset_ms = None;
        // Clear word break flag by default (set below if in a valid word break)
        self.last_is_word_break = false;

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

        let samples_len = samples.len() as u32;

        if is_speech_candidate {
            // Sound matching speech features detected
            self.silence_sample_count = 0;

            if self.is_speaking {
                // Continue confirmed speech
                self.speech_sample_count += samples.len() as u64;
                
                // Update running average of speech amplitude for word break detection
                self.update_speech_amplitude_average(rms, samples_len);
                
                // Check if we were in a word break that just ended
                if self.in_word_break {
                    // Word break ended - check if it was valid (within duration bounds)
                    if self.word_break_sample_count >= self.min_word_break_samples 
                        && self.word_break_sample_count <= self.max_word_break_samples 
                    {
                        // Valid word break detected - emit event
                        let gap_duration_ms = self.samples_to_ms(self.word_break_sample_count as u64) as u32;
                        let offset_ms = self.samples_to_ms(self.word_break_start_speech_samples) as u32;
                        
                        let _ = app_handle.emit("word-break", WordBreakPayload {
                            offset_ms,
                            gap_duration_ms,
                        });
                        
                        // Store for transcribe mode integration
                        self.last_word_break_event = Some(WordBreakEvent {
                            offset_ms,
                            gap_duration_ms,
                        });
                        
                        println!("[SpeechDetector] Word break detected (offset: {}ms, gap: {}ms)", offset_ms, gap_duration_ms);
                    }
                    // Reset word break tracking
                    self.in_word_break = false;
                    self.word_break_sample_count = 0;
                }
            } else {
                // Handle onset accumulation based on which mode matches
                if is_voiced {
                    // Accumulate voiced onset, reset grace counter
                    self.voiced_grace_count = 0;
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
                        
                        // Perform lookback analysis to find true speech start
                        let (lookback_samples, lookback_offset_ms) = self.find_lookback_start();
                        self.last_lookback_offset_ms = Some(lookback_offset_ms);
                        
                        // Record state change for transcribe mode
                        let lookback_sample_count = lookback_samples.len();
                        self.last_state_change = SpeechStateChange::Started { 
                            lookback_samples: lookback_sample_count 
                        };
                        
                        let _ = app_handle.emit("speech-started", SpeechEventPayload { 
                            duration_ms: None,
                            lookback_samples: Some(lookback_samples),
                            lookback_offset_ms: Some(lookback_offset_ms),
                        });
                        println!("[SpeechDetector] Speech started (voiced mode, lookback: {}ms)", lookback_offset_ms);
                        return;
                    }
                }

                if is_whisper {
                    // Accumulate whisper onset (can run in parallel with voiced), reset grace counter
                    self.whisper_grace_count = 0;
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
                        
                        // Perform lookback analysis to find true speech start
                        let (lookback_samples, lookback_offset_ms) = self.find_lookback_start();
                        self.last_lookback_offset_ms = Some(lookback_offset_ms);
                        
                        // Record state change for transcribe mode
                        let lookback_sample_count = lookback_samples.len();
                        self.last_state_change = SpeechStateChange::Started { 
                            lookback_samples: lookback_sample_count 
                        };
                        
                        let _ = app_handle.emit("speech-started", SpeechEventPayload { 
                            duration_ms: None,
                            lookback_samples: Some(lookback_samples),
                            lookback_offset_ms: Some(lookback_offset_ms),
                        });
                        println!("[SpeechDetector] Speech started (whisper mode, lookback: {}ms)", lookback_offset_ms);
                    }
                }
            }
        } else {
            // No speech-like features detected - use grace period before resetting onset
            if self.is_pending_voiced {
                self.voiced_grace_count += samples_len;
                if self.voiced_grace_count >= self.onset_grace_samples {
                    // Grace period exceeded, reset voiced onset
                    self.is_pending_voiced = false;
                    self.voiced_onset_count = 0;
                    self.voiced_grace_count = 0;
                }
            }
            
            if self.is_pending_whisper {
                self.whisper_grace_count += samples_len;
                if self.whisper_grace_count >= self.onset_grace_samples {
                    // Grace period exceeded, reset whisper onset
                    self.is_pending_whisper = false;
                    self.whisper_onset_count = 0;
                    self.whisper_grace_count = 0;
                }
            }
            
            if self.is_speaking {
                self.silence_sample_count += samples_len;
                
                // Word break detection: check if amplitude dropped below threshold
                let recent_avg = self.get_recent_speech_amplitude();
                let threshold = recent_avg * self.word_break_threshold_ratio;
                
                if recent_avg > 0.0 && rms < threshold {
                    // Amplitude is below word break threshold
                    if !self.in_word_break {
                        // Start tracking a potential word break
                        self.in_word_break = true;
                        self.word_break_sample_count = samples_len;
                        self.word_break_start_speech_samples = self.speech_sample_count;
                    } else {
                        // Continue tracking word break
                        self.word_break_sample_count += samples_len;
                    }
                    
                    // Mark as word break if within valid duration range
                    if self.word_break_sample_count >= self.min_word_break_samples
                        && self.word_break_sample_count <= self.max_word_break_samples
                    {
                        self.last_is_word_break = true;
                    }
                }

                // Check if hold time has elapsed
                if self.silence_sample_count >= self.hold_samples {
                    // Emit speech-ended with duration
                    let duration_ms = self.samples_to_ms(self.speech_sample_count);
                    self.is_speaking = false;
                    self.speech_sample_count = 0;
                    
                    // Reset word break state when speech ends
                    self.reset_word_break_state();
                    
                    // Record state change for transcribe mode
                    self.last_state_change = SpeechStateChange::Ended { duration_ms };
                    
                    let _ = app_handle.emit("speech-ended", SpeechEventPayload { 
                        duration_ms: Some(duration_ms),
                        lookback_samples: None,
                        lookback_offset_ms: None,
                    });
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
