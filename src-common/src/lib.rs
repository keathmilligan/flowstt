//! FlowSTT Common Library
//!
//! Shared types and IPC protocol for communication between the FlowSTT CLI,
//! service, and GUI components.

use std::sync::OnceLock;

pub mod config;
pub mod ipc;
pub mod security;
pub mod types;

pub use config::ThemeMode;
pub use types::*;

static RUNTIME_MODE: OnceLock<RuntimeMode> = OnceLock::new();

pub fn runtime_mode() -> RuntimeMode {
    *RUNTIME_MODE.get_or_init(detect_runtime_mode)
}

fn detect_runtime_mode() -> RuntimeMode {
    if let Ok(mode) = std::env::var("FLOWSTT_RUNTIME_MODE") {
        match mode.to_lowercase().as_str() {
            "development" | "dev" => return RuntimeMode::Development,
            "production" | "prod" => return RuntimeMode::Production,
            _ => {}
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        let path_str = exe_path.to_string_lossy();
        if path_str.contains("/target/debug/") || path_str.contains("\\target\\debug\\") {
            return RuntimeMode::Development;
        }
        if is_running_from_project_target(&exe_path) {
            return RuntimeMode::Development;
        }
    }

    RuntimeMode::Production
}

fn is_running_from_project_target(exe_path: &std::path::Path) -> bool {
    let path_str = exe_path.to_string_lossy();
    if !path_str.contains("/target/release/") && !path_str.contains("\\target\\release\\") {
        return false;
    }
    let mut path = exe_path.parent();
    while let Some(dir) = path {
        if dir.join("Cargo.toml").exists() {
            return true;
        }
        path = dir.parent();
    }
    false
}
