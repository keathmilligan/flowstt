//! Configuration persistence for FlowSTT service.
//!
//! This module re-exports the shared Config from flowstt-common and adds
//! service-specific logging via tracing.

pub use flowstt_common::config::Config;

use tracing::info;

/// Load configuration with tracing output.
pub fn load_config() -> Config {
    let path = Config::config_path();
    let config = Config::load();
    info!("Loaded config from {:?}", path);
    config
}

/// Save configuration with tracing output.
pub fn save_config(config: &Config) -> std::io::Result<()> {
    let path = Config::config_path();
    config.save()?;
    info!("Saved config to {:?}", path);
    Ok(())
}
