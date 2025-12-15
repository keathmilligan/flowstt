//! PipeWire-based audio capture backend
//!
//! This module provides audio capture from input devices and system audio (sink monitors)
//! using PipeWire directly. It integrates with the existing audio processing pipeline.
//! When capturing from multiple sources, they are mixed together before being sent
//! to the processing pipeline.

use pipewire::{
    context::Context,
    main_loop::MainLoop,
    properties::properties,
    spa::{
        param::audio::{AudioFormat, AudioInfoRaw},
        pod::Pod,
        utils::Direction,
    },
    stream::{Stream, StreamFlags},
    types::ObjectType,
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::mem;
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crate::audio::AudioSourceType;

/// Audio device information from PipeWire
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PwAudioDevice {
    /// PipeWire node ID
    pub id: u32,
    /// Human-readable device name
    pub name: String,
    /// Source type (Input or System)
    pub source_type: AudioSourceType,
}

/// Commands sent to the PipeWire thread
#[derive(Debug)]
enum PwCommand {
    /// Start capturing from up to two sources (mixed together)
    StartCaptureSources {
        source1_id: Option<u32>,
        source2_id: Option<u32>,
    },
    /// Stop all capture
    StopCapture,
    /// Shutdown the PipeWire thread
    Shutdown,
}

/// Audio samples received from PipeWire (already mixed if multiple sources)
pub struct PwAudioSamples {
    pub samples: Vec<f32>,
    pub channels: u16,
}

/// Handle to the PipeWire audio backend
pub struct PipeWireBackend {
    /// Channel to send commands to PipeWire thread
    cmd_tx: mpsc::Sender<PwCommand>,
    /// Channel to receive audio samples
    audio_rx: mpsc::Receiver<PwAudioSamples>,
    /// Cached input devices
    input_devices: Arc<Mutex<Vec<PwAudioDevice>>>,
    /// Cached system devices
    system_devices: Arc<Mutex<Vec<PwAudioDevice>>>,
    /// Thread handle
    _thread_handle: JoinHandle<()>,
    /// Sample rate from PipeWire
    sample_rate: Arc<Mutex<u32>>,
}

impl PipeWireBackend {
    /// Create and start the PipeWire backend
    pub fn new() -> Result<Self, String> {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (audio_tx, audio_rx) = mpsc::channel();
        let input_devices = Arc::new(Mutex::new(Vec::new()));
        let system_devices = Arc::new(Mutex::new(Vec::new()));
        let sample_rate = Arc::new(Mutex::new(48000u32));

        let input_devices_clone = Arc::clone(&input_devices);
        let system_devices_clone = Arc::clone(&system_devices);
        let sample_rate_clone = Arc::clone(&sample_rate);

        let thread_handle = thread::spawn(move || {
            if let Err(e) = run_pipewire_thread(
                cmd_rx,
                audio_tx,
                input_devices_clone,
                system_devices_clone,
                sample_rate_clone,
            ) {
                eprintln!("PipeWire thread error: {}", e);
            }
        });

        // Give PipeWire a moment to enumerate devices
        thread::sleep(std::time::Duration::from_millis(200));

        Ok(Self {
            cmd_tx,
            audio_rx,
            input_devices,
            system_devices,
            _thread_handle: thread_handle,
            sample_rate,
        })
    }

    /// Get list of input devices
    pub fn list_input_devices(&self) -> Vec<PwAudioDevice> {
        self.input_devices.lock().unwrap().clone()
    }

    /// Get list of system audio devices (sink monitors)
    pub fn list_system_devices(&self) -> Vec<PwAudioDevice> {
        self.system_devices.lock().unwrap().clone()
    }

    /// Get current sample rate
    pub fn sample_rate(&self) -> u32 {
        *self.sample_rate.lock().unwrap()
    }

    /// Start audio capture from up to two sources (mixed together)
    pub fn start_capture_sources(
        &self,
        source1_id: Option<u32>,
        source2_id: Option<u32>,
    ) -> Result<(), String> {
        self.cmd_tx
            .send(PwCommand::StartCaptureSources {
                source1_id,
                source2_id,
            })
            .map_err(|e| format!("Failed to send start command: {}", e))
    }

    /// Stop audio capture
    pub fn stop_capture(&self) -> Result<(), String> {
        self.cmd_tx
            .send(PwCommand::StopCapture)
            .map_err(|e| format!("Failed to send stop command: {}", e))
    }

    /// Try to receive audio samples (non-blocking)
    pub fn try_recv(&self) -> Option<PwAudioSamples> {
        self.audio_rx.try_recv().ok()
    }

    /// Shutdown the backend
    pub fn shutdown(&self) {
        let _ = self.cmd_tx.send(PwCommand::Shutdown);
    }
}

/// Mixer state for combining audio from multiple streams
struct AudioMixer {
    /// Buffer for stream 1 samples
    buffer1: Vec<f32>,
    /// Buffer for stream 2 samples
    buffer2: Vec<f32>,
    /// Number of active streams (1 or 2)
    num_streams: usize,
    /// Channels per stream
    channels: u16,
    /// Output sender
    output_tx: mpsc::Sender<PwAudioSamples>,
}

impl AudioMixer {
    fn new(output_tx: mpsc::Sender<PwAudioSamples>) -> Self {
        Self {
            buffer1: Vec::new(),
            buffer2: Vec::new(),
            num_streams: 0,
            channels: 2,
            output_tx,
        }
    }

    fn set_num_streams(&mut self, num: usize) {
        self.num_streams = num;
        self.buffer1.clear();
        self.buffer2.clear();
    }

    fn set_channels(&mut self, channels: u16) {
        self.channels = channels;
    }

    /// Add samples from stream 1
    fn push_stream1(&mut self, samples: &[f32]) {
        if self.num_streams == 1 {
            // Only one stream - send directly
            let _ = self.output_tx.send(PwAudioSamples {
                samples: samples.to_vec(),
                channels: self.channels,
            });
        } else {
            // Two streams - buffer and mix
            self.buffer1.extend_from_slice(samples);
            self.try_mix_and_send();
        }
    }

    /// Add samples from stream 2
    fn push_stream2(&mut self, samples: &[f32]) {
        if self.num_streams <= 1 {
            return; // Shouldn't happen, but ignore
        }
        self.buffer2.extend_from_slice(samples);
        self.try_mix_and_send();
    }

    /// Try to mix available samples and send
    fn try_mix_and_send(&mut self) {
        // Mix the minimum available samples from both buffers
        let mix_count = std::cmp::min(self.buffer1.len(), self.buffer2.len());
        
        if mix_count == 0 {
            return;
        }
        
        // Mix with 0.5 gain each to prevent clipping
        let mixed: Vec<f32> = self.buffer1.iter()
            .zip(self.buffer2.iter())
            .take(mix_count)
            .map(|(&s1, &s2)| (s1 + s2) * 0.5)
            .collect();
        
        // Remove processed samples from buffers
        self.buffer1.drain(0..mix_count);
        self.buffer2.drain(0..mix_count);
        
        // Send mixed output
        let _ = self.output_tx.send(PwAudioSamples {
            samples: mixed,
            channels: self.channels,
        });
    }
}

/// Held stream state - keeps stream and listener alive
struct ActiveStream {
    _stream: Stream,
    // The listener is leaked (forgotten) to keep it alive
}

/// Internal state for the PipeWire thread
struct PwThreadState {
    /// Audio mixer for combining streams
    mixer: Rc<RefCell<AudioMixer>>,
    /// Active streams (kept alive)
    streams: Vec<ActiveStream>,
    /// Sample rate (updated from param_changed)
    sample_rate: Arc<Mutex<u32>>,
    /// Set of sink (system audio) device IDs
    sink_ids: Rc<RefCell<std::collections::HashSet<u32>>>,
}

/// Run the PipeWire main loop thread
fn run_pipewire_thread(
    cmd_rx: mpsc::Receiver<PwCommand>,
    audio_tx: mpsc::Sender<PwAudioSamples>,
    input_devices: Arc<Mutex<Vec<PwAudioDevice>>>,
    system_devices: Arc<Mutex<Vec<PwAudioDevice>>>,
    sample_rate: Arc<Mutex<u32>>,
) -> Result<(), String> {
    // Initialize PipeWire
    pipewire::init();

    let mainloop = MainLoop::new(None).map_err(|e| format!("Failed to create main loop: {}", e))?;
    let context = Context::new(&mainloop).map_err(|e| format!("Failed to create context: {}", e))?;
    let core = context
        .connect(None)
        .map_err(|e| format!("Failed to connect to PipeWire: {}", e))?;
    let registry = core
        .get_registry()
        .map_err(|e| format!("Failed to get registry: {}", e))?;

    // Device maps for enumeration
    let input_map: Rc<RefCell<HashMap<u32, PwAudioDevice>>> = Rc::new(RefCell::new(HashMap::new()));
    let system_map: Rc<RefCell<HashMap<u32, PwAudioDevice>>> = Rc::new(RefCell::new(HashMap::new()));

    // Setup registry listener for device discovery
    let input_map_clone = Rc::clone(&input_map);
    let system_map_clone = Rc::clone(&system_map);
    let input_devices_clone = Arc::clone(&input_devices);
    let system_devices_clone = Arc::clone(&system_devices);

    let _registry_listener = registry
        .add_listener_local()
        .global(move |global| {
            if global.type_ == ObjectType::Node {
                if let Some(props) = &global.props {
                    let media_class = props.get("media.class").unwrap_or("");
                    let node_name = props.get("node.name").unwrap_or("Unknown");
                    let node_desc = props.get("node.description").unwrap_or(node_name);

                    if media_class == "Audio/Source" {
                        // Input device (microphone)
                        let device = PwAudioDevice {
                            id: global.id,
                            name: node_desc.to_string(),
                            source_type: AudioSourceType::Input,
                        };
                        input_map_clone.borrow_mut().insert(global.id, device);
                        // Update shared list
                        let devices: Vec<_> = input_map_clone.borrow().values().cloned().collect();
                        *input_devices_clone.lock().unwrap() = devices;
                    } else if media_class == "Audio/Sink" {
                        // Output device - we can capture its monitor
                        let device = PwAudioDevice {
                            id: global.id,
                            name: format!("{} (Monitor)", node_desc),
                            source_type: AudioSourceType::System,
                        };
                        system_map_clone.borrow_mut().insert(global.id, device);
                        // Update shared list
                        let devices: Vec<_> = system_map_clone.borrow().values().cloned().collect();
                        *system_devices_clone.lock().unwrap() = devices;
                    }
                }
            }
        })
        .global_remove({
            let input_map = Rc::clone(&input_map);
            let system_map = Rc::clone(&system_map);
            let input_devices = Arc::clone(&input_devices);
            let system_devices = Arc::clone(&system_devices);
            move |id| {
                if input_map.borrow_mut().remove(&id).is_some() {
                    let devices: Vec<_> = input_map.borrow().values().cloned().collect();
                    *input_devices.lock().unwrap() = devices;
                }
                if system_map.borrow_mut().remove(&id).is_some() {
                    let devices: Vec<_> = system_map.borrow().values().cloned().collect();
                    *system_devices.lock().unwrap() = devices;
                }
            }
        })
        .register();

    // Create mixer
    let mixer = Rc::new(RefCell::new(AudioMixer::new(audio_tx)));

    // Thread state - share system_map to know which IDs are sinks
    let state = Rc::new(RefCell::new(PwThreadState {
        mixer: Rc::clone(&mixer),
        streams: Vec::new(),
        sample_rate: Arc::clone(&sample_rate),
        sink_ids: Rc::new(RefCell::new(std::collections::HashSet::new())),
    }));
    
    // Keep sink_ids in sync with system_map
    let sink_ids_for_state = Rc::clone(&state.borrow().sink_ids);
    let system_map_for_sync = Rc::clone(&system_map);

    // Setup command receiver using a timer that polls the channel
    let mainloop_weak = mainloop.downgrade();
    let core_ref = Rc::new(core);
    let core_for_timer = Rc::clone(&core_ref);
    let state_for_timer = Rc::clone(&state);
    let mixer_for_timer = Rc::clone(&mixer);

    // Create a timer source to poll for commands
    let timer_source = mainloop.loop_().add_timer({
        move |_elapsed| {
            // Update sink_ids from system_map
            {
                let mut sink_ids = sink_ids_for_state.borrow_mut();
                sink_ids.clear();
                for id in system_map_for_sync.borrow().keys() {
                    sink_ids.insert(*id);
                }
            }
            
            // Poll for commands
            while let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    PwCommand::StartCaptureSources {
                        source1_id,
                        source2_id,
                    } => {
                        // First, check which sources are sinks (before borrowing state mutably)
                        let is_sink1 = source1_id.map(|id| {
                            state_for_timer.borrow().sink_ids.borrow().contains(&id)
                        }).unwrap_or(false);
                        let is_sink2 = source2_id.map(|id| {
                            state_for_timer.borrow().sink_ids.borrow().contains(&id)
                        }).unwrap_or(false);
                        
                        let mut state = state_for_timer.borrow_mut();
                        // Clear existing streams
                        state.streams.clear();
                        
                        // Count how many streams we'll have
                        let num_streams = source1_id.is_some() as usize + source2_id.is_some() as usize;
                        mixer_for_timer.borrow_mut().set_num_streams(num_streams);

                        // Create stream for source1 if specified
                        if let Some(id) = source1_id {
                            let mixer_clone = Rc::clone(&mixer_for_timer);
                            match create_capture_stream(
                                &core_for_timer,
                                Some(id),
                                is_sink1,
                                1, // stream index
                                mixer_clone,
                                Arc::clone(&state.sample_rate),
                            ) {
                                Ok(stream) => state.streams.push(stream),
                                Err(e) => eprintln!("Failed to create stream for source1: {}", e),
                            }
                        }
                        
                        // Create stream for source2 if specified
                        if let Some(id) = source2_id {
                            let mixer_clone = Rc::clone(&mixer_for_timer);
                            match create_capture_stream(
                                &core_for_timer,
                                Some(id),
                                is_sink2,
                                2, // stream index
                                mixer_clone,
                                Arc::clone(&state.sample_rate),
                            ) {
                                Ok(stream) => state.streams.push(stream),
                                Err(e) => eprintln!("Failed to create stream for source2: {}", e),
                            }
                        }
                    }
                    PwCommand::StopCapture => {
                        state_for_timer.borrow_mut().streams.clear();
                        mixer_for_timer.borrow_mut().set_num_streams(0);
                    }
                    PwCommand::Shutdown => {
                        if let Some(mainloop) = mainloop_weak.upgrade() {
                            mainloop.quit();
                        }
                    }
                }
            }
        }
    });

    // Set timer to fire every 10ms
    timer_source.update_timer(
        Some(std::time::Duration::from_millis(10)),
        Some(std::time::Duration::from_millis(10)),
    );

    // Run the main loop (blocks until quit)
    mainloop.run();

    Ok(())
}

/// Create an audio format pod for stream connection
fn create_audio_format_pod() -> Vec<u8> {
    let mut audio_info = AudioInfoRaw::new();
    audio_info.set_format(AudioFormat::F32LE);
    // Leave rate and channels unset to accept native graph format
    
    let obj = pipewire::spa::pod::Object {
        type_: pipewire::spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: pipewire::spa::param::ParamType::EnumFormat.as_raw(),
        properties: audio_info.into(),
    };
    
    pipewire::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pipewire::spa::pod::Value::Object(obj),
    )
    .unwrap()
    .0
    .into_inner()
}

/// Create a capture stream that sends samples to the mixer
fn create_capture_stream(
    core: &pipewire::core::Core,
    device_id: Option<u32>,
    capture_sink: bool,
    stream_index: usize, // 1 or 2
    mixer: Rc<RefCell<AudioMixer>>,
    sample_rate: Arc<Mutex<u32>>,
) -> Result<ActiveStream, String> {
    let stream_name = if capture_sink { 
        format!("stt-system-capture-{}", stream_index)
    } else { 
        format!("stt-input-capture-{}", stream_index)
    };
    
    let props = if capture_sink {
        properties! {
            *pipewire::keys::MEDIA_TYPE => "Audio",
            *pipewire::keys::MEDIA_CATEGORY => "Capture",
            *pipewire::keys::MEDIA_ROLE => "Music",
            *pipewire::keys::STREAM_CAPTURE_SINK => "true",
        }
    } else {
        properties! {
            *pipewire::keys::MEDIA_TYPE => "Audio",
            *pipewire::keys::MEDIA_CATEGORY => "Capture",
            *pipewire::keys::MEDIA_ROLE => "Music",
        }
    };

    let stream = Stream::new(core, &stream_name, props)
        .map_err(|e| format!("Failed to create stream: {}", e))?;

    // Track format info from param_changed
    let format_info: Rc<RefCell<AudioInfoRaw>> = Rc::new(RefCell::new(AudioInfoRaw::default()));
    let format_info_for_param = Rc::clone(&format_info);
    let sample_rate_for_param = Arc::clone(&sample_rate);
    let mixer_for_param = Rc::clone(&mixer);
    
    let format_info_for_process = Rc::clone(&format_info);
    let mixer_for_process = mixer;

    let listener = stream
        .add_local_listener_with_user_data(())
        .param_changed(move |_stream, _user_data, id, param| {
            let Some(param) = param else { return };
            
            if id != pipewire::spa::param::ParamType::Format.as_raw() {
                return;
            }

            // Parse the format
            if let Ok((media_type, media_subtype)) = 
                pipewire::spa::param::format_utils::parse_format(param) 
            {
                use pipewire::spa::param::format::{MediaType, MediaSubtype};
                if media_type != MediaType::Audio || media_subtype != MediaSubtype::Raw {
                    return;
                }

                if format_info_for_param.borrow_mut().parse(param).is_ok() {
                    let rate = format_info_for_param.borrow().rate();
                    let channels = format_info_for_param.borrow().channels();
                    println!("Stream {} format: rate={}, channels={}", stream_index, rate, channels);
                    *sample_rate_for_param.lock().unwrap() = rate;
                    mixer_for_param.borrow_mut().set_channels(channels as u16);
                }
            }
        })
        .state_changed(move |_stream, _user_data, old, new| {
            println!("Stream {} state: {:?} -> {:?}", stream_index, old, new);
        })
        .process(move |stream, _user_data| {
            if let Some(mut buffer) = stream.dequeue_buffer() {
                let datas = buffer.datas_mut();
                if datas.is_empty() {
                    return;
                }

                let data = &mut datas[0];
                // Get chunk info first
                let chunk_size = data.chunk().size() as usize;
                let n_samples = chunk_size / mem::size_of::<f32>();
                
                if n_samples == 0 {
                    return;
                }

                if let Some(samples_data) = data.data() {
                    // Convert bytes to f32 samples
                    let samples: Vec<f32> = samples_data[..chunk_size]
                        .chunks_exact(4)
                        .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                        .collect();

                    if !samples.is_empty() {
                        // Send to appropriate mixer buffer based on stream index
                        let mut mixer = mixer_for_process.borrow_mut();
                        if stream_index == 1 {
                            mixer.push_stream1(&samples);
                        } else {
                            mixer.push_stream2(&samples);
                        }
                    }
                }
            }
        })
        .register()
        .map_err(|e| format!("Failed to register stream listener: {}", e))?;

    // Create audio format parameters
    let format_pod = create_audio_format_pod();
    let mut params = [Pod::from_bytes(&format_pod).unwrap()];

    // Connect to device (or default if None)
    let flags = StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS;
    
    stream
        .connect(Direction::Input, device_id, flags, &mut params)
        .map_err(|e| format!("Failed to connect stream: {}", e))?;

    // Leak the listener to keep it alive - it will be cleaned up when stream is dropped
    std::mem::forget(listener);

    Ok(ActiveStream { _stream: stream })
}
