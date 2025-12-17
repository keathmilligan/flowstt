//! Linux audio backend module
//!
//! Provides audio capture using PipeWire, supporting both input devices
//! (microphones) and system audio capture (sink monitors).

mod pipewire;

pub use pipewire::create_backend;
