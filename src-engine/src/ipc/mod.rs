//! IPC server for client communication.

pub mod handlers;
pub(crate) mod server;

pub use server::{broadcast_event, register_event_callback, run_server, EventCallback};
