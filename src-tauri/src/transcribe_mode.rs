//! Automatic transcription mode with continuous recording and speech-based segmentation.
//!
//! This module provides:
//! - `SegmentRingBuffer`: A ring buffer for continuous audio capture
//! - `TranscriptionQueue`: A bounded queue for async transcription processing
//! - `TranscribeState`: State management for transcribe mode

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};

use crate::audio::{generate_recording_filename, process_recorded_audio, save_to_wav, RawRecordedAudio};
use crate::transcribe::Transcriber;

/// Ring buffer capacity: 30 seconds at 48kHz stereo
/// 48000 * 30 * 2 = 2,880,000 samples
const RING_BUFFER_CAPACITY: usize = 48000 * 30 * 2;

/// Overflow threshold: 90% of buffer capacity
const OVERFLOW_THRESHOLD_PERCENT: usize = 90;

/// Maximum queue size for transcription segments
const MAX_QUEUE_SIZE: usize = 10;

// ============================================================================
// Segment Ring Buffer
// ============================================================================

/// A ring buffer for continuous audio capture during transcribe mode.
/// 
/// Provides continuous write without blocking, and segment extraction by copying
/// samples between indices. Handles wraparound correctly.
pub struct SegmentRingBuffer {
    /// The underlying buffer
    buffer: Vec<f32>,
    /// Current write position
    write_pos: usize,
    /// Capacity of the buffer
    capacity: usize,
    /// Total samples written (for tracking)
    total_written: u64,
}

impl SegmentRingBuffer {
    /// Create a new ring buffer with the specified capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0.0; capacity],
            write_pos: 0,
            capacity,
            total_written: 0,
        }
    }

    /// Create a ring buffer with default capacity (30 seconds at 48kHz stereo)
    pub fn with_default_capacity() -> Self {
        Self::new(RING_BUFFER_CAPACITY)
    }

    /// Write samples to the buffer, advancing write position and wrapping
    pub fn write(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.buffer[self.write_pos] = sample;
            self.write_pos = (self.write_pos + 1) % self.capacity;
            self.total_written += 1;
        }
    }

    /// Get current write position
    pub fn write_position(&self) -> usize {
        self.write_pos
    }

    /// Get buffer capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Calculate segment length from start_idx to current write_pos, handling wraparound
    pub fn segment_length(&self, start_idx: usize) -> usize {
        if self.write_pos >= start_idx {
            self.write_pos - start_idx
        } else {
            // Wraparound case: distance from start to end + distance from 0 to write_pos
            (self.capacity - start_idx) + self.write_pos
        }
    }

    /// Calculate a sample index from lookback offset (samples back from write_pos)
    pub fn index_from_lookback(&self, lookback_samples: usize) -> usize {
        if lookback_samples >= self.capacity {
            // Clamp to buffer size
            self.write_pos
        } else if lookback_samples <= self.write_pos {
            self.write_pos - lookback_samples
        } else {
            // Wraparound case
            self.capacity - (lookback_samples - self.write_pos)
        }
    }

    /// Check if segment length exceeds overflow threshold
    pub fn is_approaching_overflow(&self, start_idx: usize) -> bool {
        let segment_len = self.segment_length(start_idx);
        let threshold = (self.capacity * OVERFLOW_THRESHOLD_PERCENT) / 100;
        segment_len >= threshold
    }

    /// Extract segment from start_idx to current write_pos, handling wraparound
    /// Returns a new Vec with the copied samples
    pub fn extract_segment(&self, start_idx: usize) -> Vec<f32> {
        let segment_len = self.segment_length(start_idx);
        if segment_len == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(segment_len);

        if self.write_pos >= start_idx {
            // No wraparound: simple slice copy
            result.extend_from_slice(&self.buffer[start_idx..self.write_pos]);
        } else {
            // Wraparound: copy from start_idx to end, then from 0 to write_pos
            result.extend_from_slice(&self.buffer[start_idx..]);
            result.extend_from_slice(&self.buffer[..self.write_pos]);
        }

        result
    }

    /// Clear the buffer (reset write position but don't zero memory)
    pub fn clear(&mut self) {
        self.write_pos = 0;
        self.total_written = 0;
    }
}

// ============================================================================
// Transcription Queue
// ============================================================================

/// A segment queued for transcription
pub struct QueuedSegment {
    /// Audio samples (owned copy from ring buffer)
    pub samples: Vec<f32>,
    /// Sample rate
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u16,
    /// Path where WAV was saved (if successful)
    pub wav_path: Option<PathBuf>,
}

/// A bounded queue for audio segments awaiting transcription.
/// 
/// Provides thread-safe enqueue/dequeue operations with a worker thread
/// that processes segments sequentially and emits transcription results.
pub struct TranscriptionQueue {
    /// The queue of segments
    queue: Arc<Mutex<VecDeque<QueuedSegment>>>,
    /// Flag indicating worker should continue running
    worker_active: Arc<AtomicBool>,
    /// Count of segments currently in queue
    queue_count: Arc<AtomicUsize>,
}

impl TranscriptionQueue {
    /// Create a new transcription queue
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            worker_active: Arc::new(AtomicBool::new(false)),
            queue_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Enqueue a segment for transcription.
    /// Returns false if queue is full (segment was not added).
    pub fn enqueue(&self, segment: QueuedSegment) -> bool {
        let mut queue = self.queue.lock().unwrap();
        if queue.len() >= MAX_QUEUE_SIZE {
            // Queue is full, don't add
            return false;
        }
        queue.push_back(segment);
        self.queue_count.store(queue.len(), Ordering::SeqCst);
        true
    }

    /// Get current queue depth
    pub fn queue_depth(&self) -> usize {
        self.queue_count.load(Ordering::SeqCst)
    }

    /// Check if worker is active
    pub fn is_worker_active(&self) -> bool {
        self.worker_active.load(Ordering::SeqCst)
    }

    /// Start the transcription worker thread
    pub fn start_worker(&self, app_handle: AppHandle, model_path: PathBuf) {
        if self.worker_active.load(Ordering::SeqCst) {
            return; // Already running
        }

        self.worker_active.store(true, Ordering::SeqCst);

        let queue = Arc::clone(&self.queue);
        let worker_active = Arc::clone(&self.worker_active);
        let queue_count = Arc::clone(&self.queue_count);

        thread::spawn(move || {
            let mut transcriber = Transcriber::new();
            
            // Try to load model at start
            if model_path.exists() {
                if let Err(e) = transcriber.load_model() {
                    eprintln!("[TranscriptionQueue] Failed to load model: {}", e);
                }
            }

            loop {
                // Check if we should stop
                if !worker_active.load(Ordering::SeqCst) {
                    // Drain remaining queue before exiting
                    let remaining = {
                        let q = queue.lock().unwrap();
                        q.len()
                    };
                    if remaining == 0 {
                        break;
                    }
                    // Continue processing remaining items
                }

                // Try to get a segment from queue
                let segment = {
                    let mut q = queue.lock().unwrap();
                    let seg = q.pop_front();
                    queue_count.store(q.len(), Ordering::SeqCst);
                    seg
                };

                match segment {
                    Some(seg) => {
                        // Emit queue status update
                        let depth = queue_count.load(Ordering::SeqCst);
                        let _ = app_handle.emit("transcribe-queue-update", depth);

                        // Process the segment
                        let raw_audio = RawRecordedAudio {
                            samples: seg.samples,
                            sample_rate: seg.sample_rate,
                            channels: seg.channels,
                        };

                        // Convert to format suitable for Whisper
                        match process_recorded_audio(raw_audio) {
                            Ok(processed) => {
                                // Emit event that transcription is starting (GPU may be active)
                                let _ = app_handle.emit("transcription-started", ());
                                
                                // Transcribe
                                match transcriber.transcribe(&processed) {
                                    Ok(text) => {
                                        let _ = app_handle.emit("transcription-complete", text);
                                    }
                                    Err(e) => {
                                        let _ = app_handle.emit("transcription-error", e);
                                    }
                                }
                                
                                // Emit event that transcription finished (GPU no longer active)
                                let _ = app_handle.emit("transcription-finished", ());
                            }
                            Err(e) => {
                                let _ = app_handle.emit("transcription-error", e);
                            }
                        }
                    }
                    None => {
                        // No segment available, sleep briefly
                        thread::sleep(std::time::Duration::from_millis(50));
                    }
                }
            }

            println!("[TranscriptionQueue] Worker thread exiting");
        });
    }

    /// Stop the transcription worker (will drain remaining queue)
    pub fn stop_worker(&self) {
        self.worker_active.store(false, Ordering::SeqCst);
    }

    /// Clear the queue (discard pending segments)
    pub fn clear(&self) {
        let mut queue = self.queue.lock().unwrap();
        queue.clear();
        self.queue_count.store(0, Ordering::SeqCst);
    }
}

impl Default for TranscriptionQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Transcribe State
// ============================================================================

/// State for automatic transcription mode.
/// 
/// Manages the ring buffer, tracks speech segments, and coordinates
/// with the transcription queue.
pub struct TranscribeState {
    /// Ring buffer for continuous audio capture
    pub ring_buffer: SegmentRingBuffer,
    /// Whether transcribe mode is active
    pub is_active: bool,
    /// Whether we're currently inside a speech segment
    pub in_speech: bool,
    /// Ring buffer index where current speech segment started
    pub segment_start_idx: usize,
    /// Sample rate for the capture
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u16,
    /// Reference to the transcription queue
    pub transcription_queue: Arc<TranscriptionQueue>,
}

impl TranscribeState {
    /// Create a new transcribe state
    pub fn new(transcription_queue: Arc<TranscriptionQueue>) -> Self {
        Self {
            ring_buffer: SegmentRingBuffer::with_default_capacity(),
            is_active: false,
            in_speech: false,
            segment_start_idx: 0,
            sample_rate: 48000,
            channels: 2,
            transcription_queue,
        }
    }

    /// Initialize for capture with specified parameters
    pub fn init_for_capture(&mut self, sample_rate: u32, channels: u16) {
        self.sample_rate = sample_rate;
        self.channels = channels;
        self.ring_buffer.clear();
        self.in_speech = false;
        self.segment_start_idx = 0;
    }

    /// Activate transcribe mode
    pub fn activate(&mut self) {
        self.is_active = true;
        self.in_speech = false;
        self.segment_start_idx = 0;
    }

    /// Deactivate transcribe mode
    pub fn deactivate(&mut self) {
        self.is_active = false;
        self.in_speech = false;
    }

    /// Process incoming audio samples - writes to ring buffer and checks for overflow
    /// Returns Some(segment) if overflow extraction occurred
    pub fn process_samples(&mut self, samples: &[f32], app_handle: &AppHandle) -> Option<Vec<f32>> {
        if !self.is_active {
            return None;
        }

        // Check for overflow before writing (if in speech)
        let overflow_segment = if self.in_speech && self.ring_buffer.is_approaching_overflow(self.segment_start_idx) {
            // Extract current segment before it gets overwritten
            let segment = self.ring_buffer.extract_segment(self.segment_start_idx);
            
            // Update segment start to current write position
            self.segment_start_idx = self.ring_buffer.write_position();
            
            // Remain in speech state
            println!("[TranscribeState] Buffer overflow - extracted partial segment ({} samples)", segment.len());
            
            Some(segment)
        } else {
            None
        };

        // Write samples to ring buffer (always happens)
        self.ring_buffer.write(samples);

        // If we extracted a segment due to overflow, queue it
        if let Some(segment) = overflow_segment.clone() {
            self.queue_segment(segment, app_handle);
        }

        overflow_segment
    }

    /// Handle speech-started event: mark segment start including lookback
    pub fn on_speech_started(&mut self, lookback_samples: usize) {
        if !self.is_active {
            return;
        }

        self.in_speech = true;
        self.segment_start_idx = self.ring_buffer.index_from_lookback(lookback_samples);
        println!(
            "[TranscribeState] Speech started, segment_start_idx={}, lookback={}",
            self.segment_start_idx, lookback_samples
        );
    }

    /// Handle speech-ended event: extract segment and queue for transcription
    pub fn on_speech_ended(&mut self, app_handle: &AppHandle) -> Option<Vec<f32>> {
        if !self.is_active || !self.in_speech {
            return None;
        }

        // Extract the segment
        let segment = self.ring_buffer.extract_segment(self.segment_start_idx);
        
        self.in_speech = false;
        
        if segment.is_empty() {
            println!("[TranscribeState] Speech ended but segment is empty");
            return None;
        }

        println!("[TranscribeState] Speech ended, extracted {} samples", segment.len());

        // Queue the segment for transcription
        self.queue_segment(segment.clone(), app_handle);

        Some(segment)
    }

    /// Queue a segment for transcription (saves WAV and enqueues)
    fn queue_segment(&self, samples: Vec<f32>, app_handle: &AppHandle) {
        if samples.is_empty() {
            return;
        }

        // Save to WAV file
        let filename = generate_recording_filename();
        let recordings_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Documents")
            .join("Recordings");

        // Create directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&recordings_dir) {
            eprintln!("[TranscribeState] Failed to create recordings directory: {}", e);
        }

        let output_path = recordings_dir.join(&filename);
        let wav_path = match save_to_wav(&samples, self.sample_rate, self.channels, &output_path) {
            Ok(()) => {
                println!("[TranscribeState] Saved segment to: {:?}", output_path);
                let _ = app_handle.emit("recording-saved", output_path.to_string_lossy().to_string());
                Some(output_path)
            }
            Err(e) => {
                eprintln!("[TranscribeState] Failed to save WAV: {}", e);
                None
            }
        };

        // Create queued segment
        let queued = QueuedSegment {
            samples,
            sample_rate: self.sample_rate,
            channels: self.channels,
            wav_path,
        };

        // Enqueue for transcription
        if !self.transcription_queue.enqueue(queued) {
            eprintln!("[TranscribeState] Transcription queue is full, segment dropped");
        }

        // Emit queue update
        let depth = self.transcription_queue.queue_depth();
        let _ = app_handle.emit("transcribe-queue-update", depth);
    }

    /// Finalize any pending segment (called when transcribe mode is stopped)
    pub fn finalize(&mut self, app_handle: &AppHandle) -> Option<Vec<f32>> {
        if self.in_speech {
            // Extract and queue the in-progress segment
            self.on_speech_ended(app_handle)
        } else {
            None
        }
    }
}
