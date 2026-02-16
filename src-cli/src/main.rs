//! FlowSTT Command Line Interface
//!
//! This is the command-line interface for FlowSTT voice transcription.
//! It communicates with the background service via IPC.

mod client;

use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use flowstt_common::config::Config;
use flowstt_common::ipc::{EventType, Request, Response};
use flowstt_common::{AudioSourceType, ConfigValues, HotkeyCombination, KeyCode, RecordingMode, TranscriptionMode};

use client::Client;

#[derive(Parser)]
#[command(name = "flowstt")]
#[command(author = "FlowSTT")]
#[command(version)]
#[command(about = "Voice transcription CLI", long_about = None)]
struct Cli {
    /// Output format
    #[arg(long, default_value = "text")]
    format: OutputFormat,

    /// Suppress non-essential output
    #[arg(short, long)]
    quiet: bool,

    /// Increase verbosity
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Subcommand)]
enum Commands {
    /// List available audio devices
    #[command(alias = "ls")]
    List {
        /// Filter by source type
        #[arg(short, long)]
        source: Option<SourceFilter>,
    },

    /// Start transcription
    Transcribe {
        /// Primary audio source ID (use 'list' to see available devices)
        #[arg(short = '1', long)]
        source1: Option<String>,

        /// Secondary audio source ID for mixing or AEC
        #[arg(short = '2', long)]
        source2: Option<String>,

        /// Enable acoustic echo cancellation
        #[arg(long)]
        aec: bool,

        /// Recording mode (mix or echo-cancel)
        #[arg(short, long, default_value = "mixed")]
        mode: RecordingModeArg,
    },

    /// Get current transcription status
    Status,

    /// Stop transcription
    Stop,

    /// Show Whisper model status
    Model {
        #[command(subcommand)]
        action: Option<ModelAction>,
    },

    /// Show GPU/CUDA acceleration status
    Gpu,

    /// Read or write persisted configuration values
    #[command(alias = "cfg")]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Toggle between Automatic and Push-to-Talk transcription modes
    ToggleAuto,

    /// Run interactive first-time setup wizard
    Setup,

    /// Ping the service
    Ping,

    /// Stop the background service
    Shutdown,

    /// Show version information
    Version,
}

#[derive(Clone, ValueEnum)]
enum SourceFilter {
    Input,
    System,
}

#[derive(Clone, ValueEnum)]
enum RecordingModeArg {
    Mixed,
    EchoCancel,
}

#[derive(Subcommand)]
enum ModelAction {
    /// Download the Whisper model
    Download,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Display all persisted configuration values
    Show,

    /// Get the value of a configuration key
    Get {
        /// Configuration key (transcription_mode, ptt_hotkeys)
        key: String,
    },

    /// Set the value of a configuration key
    Set {
        /// Configuration key (transcription_mode, ptt_hotkeys)
        key: String,

        /// Value to set (e.g. "automatic", "push_to_talk", or JSON for ptt_hotkeys)
        value: String,
    },
}

/// Valid configuration key names.
const VALID_CONFIG_KEYS: &[&str] = &["transcription_mode", "ptt_hotkeys", "auto_toggle_hotkeys"];

/// Error with an associated exit code.
struct CliError {
    message: String,
    exit_code: i32,
}

impl CliError {
    fn new(message: impl Into<String>, exit_code: i32) -> Self {
        Self {
            message: message.into(),
            exit_code,
        }
    }

    fn general(message: impl Into<String>) -> Self {
        Self::new(message, 1)
    }

    fn usage(message: impl Into<String>) -> Self {
        Self::new(message, 64)
    }
}

impl From<String> for CliError {
    fn from(message: String) -> Self {
        Self::general(message)
    }
}

impl From<&str> for CliError {
    fn from(message: &str) -> Self {
        Self::general(message.to_string())
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        eprintln!("{}: {}", "Error".red().bold(), e.message);
        std::process::exit(e.exit_code);
    }
}

async fn run(cli: Cli) -> Result<(), CliError> {
    let mut client = Client::new();

    // Handle version separately (doesn't need service)
    if matches!(cli.command, Commands::Version) {
        println!("flowstt {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Handle config commands (can work offline)
    if let Commands::Config { ref action } = cli.command {
        return handle_config(&mut client, action, &cli).await;
    }

    // Handle setup command
    if matches!(cli.command, Commands::Setup) {
        return handle_setup(&mut client, &cli).await;
    }

    // Connect to service (spawn if needed)
    client
        .connect_or_spawn()
        .await
        .map_err(|e| format!("Failed to connect to service: {}", e))?;

    match cli.command {
        Commands::List { source } => {
            let source_type = source.map(|s| match s {
                SourceFilter::Input => AudioSourceType::Input,
                SourceFilter::System => AudioSourceType::System,
            });

            let response = client
                .request(Request::ListDevices { source_type })
                .await
                .map_err(|e| e.to_string())?;

            match response {
                Response::Devices { devices } => {
                    if matches!(cli.format, OutputFormat::Json) {
                        println!("{}", serde_json::to_string_pretty(&devices).unwrap());
                    } else if devices.is_empty() {
                        println!("No audio devices found");
                    } else {
                        println!(
                            "{} {} found:\n",
                            devices.len().to_string().green().bold(),
                            if devices.len() == 1 {
                                "device"
                            } else {
                                "devices"
                            }
                        );
                        for device in devices {
                            let source_badge = match device.source_type {
                                AudioSourceType::Input => "[input]".cyan(),
                                AudioSourceType::System => "[system]".magenta(),
                                AudioSourceType::Mixed => "[mixed]".yellow(),
                            };
                            println!("  {} {}", source_badge, device.name);
                            println!("    ID: {}", device.id.dimmed());
                        }
                    }
                }
                Response::Error { message } => return Err(message.into()),
                _ => return Err("Unexpected response".into()),
            }
        }

        Commands::Transcribe {
            source1,
            source2,
            aec,
            mode,
        } => {
            if source1.is_none() && source2.is_none() {
                return Err(
                    "At least one audio source is required. Use 'flowstt list' to see devices."
                        .into(),
                );
            }

            let recording_mode = match mode {
                RecordingModeArg::Mixed => RecordingMode::Mixed,
                RecordingModeArg::EchoCancel => RecordingMode::EchoCancel,
            };

            // Set AEC and recording mode first
            if aec {
                let _ = client
                    .request(Request::SetAecEnabled { enabled: true })
                    .await;
            }
            let _ = client
                .request(Request::SetRecordingMode {
                    mode: recording_mode,
                })
                .await;

            // Set sources - this starts capture automatically
            let response = client
                .request(Request::SetSources {
                    source1_id: source1,
                    source2_id: source2,
                })
                .await
                .map_err(|e| e.to_string())?;

            match response {
                Response::Ok => {
                    if !cli.quiet {
                        println!("{}", "Transcription started".green());
                        println!("Press Ctrl+C to stop, or run 'flowstt stop'");
                    }

                    // Create a dedicated event client (separate connection)
                    let mut event_client = Client::new();
                    event_client
                        .connect_or_spawn()
                        .await
                        .map_err(|e| format!("Failed to connect event client: {}", e))?;

                    event_client
                        .subscribe_events()
                        .await
                        .map_err(|e| format!("Failed to subscribe: {}", e))?;

                    // Set up Ctrl+C handler
                    let shutdown = tokio::signal::ctrl_c();
                    tokio::pin!(shutdown);

                    // Stream events until Ctrl+C or capture stops
                    loop {
                        tokio::select! {
                            _ = &mut shutdown => {
                                if !cli.quiet {
                                    eprintln!("\n{}", "Interrupted".yellow());
                                }
                                break;
                            }
                            event_result = event_client.read_event() => {
                                match event_result {
                                    Ok(Response::Event { event }) => {
                                        match event {
                                            EventType::TranscriptionComplete(result) => {
                                                if matches!(cli.format, OutputFormat::Json) {
                                                    println!("{}", serde_json::to_string(&result).unwrap());
                                                } else {
                                                    println!("{}", result.text);
                                                }
                                            }
                                            EventType::SpeechStarted => {
                                                if cli.verbose {
                                                    eprintln!("{}", "[speech started]".dimmed());
                                                }
                                            }
                                            EventType::SpeechEnded { duration_ms } => {
                                                if cli.verbose {
                                                    eprintln!("{}", format!("[speech ended: {}ms]", duration_ms).dimmed());
                                                }
                                            }
                                            EventType::CaptureStateChanged { capturing, error } => {
                                                if !capturing {
                                                    if let Some(err) = error {
                                                        eprintln!("{}: {}", "Capture error".red(), err);
                                                    } else if !cli.quiet {
                                                        eprintln!("{}", "Capture stopped".yellow());
                                                    }
                                                    break;
                                                }
                                            }
                                            EventType::Shutdown => {
                                                if !cli.quiet {
                                                    eprintln!("{}", "Service shutting down".yellow());
                                                }
                                                break;
                                            }
                                            // Ignore other events (visualization, PTT, etc.)
                                            _ => {}
                                        }
                                    }
                                    Ok(_) => {
                                        // Non-event response in stream, ignore
                                    }
                                    Err(e) => {
                                        eprintln!("{}: {}", "Event stream error".red(), e);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                Response::Error { message } => return Err(message.into()),
                _ => return Err("Unexpected response".into()),
            }
        }

        Commands::Status => {
            let response = client
                .request(Request::GetStatus)
                .await
                .map_err(|e| e.to_string())?;

            match response {
                Response::Status(status) => {
                    if matches!(cli.format, OutputFormat::Json) {
                        println!("{}", serde_json::to_string_pretty(&status).unwrap());
                    } else {
                        let capture_str = if status.capturing {
                            "capturing".green().bold()
                        } else {
                            "idle".dimmed()
                        };
                        println!("Capture: {}", capture_str);

                        let mode_str = match status.transcription_mode {
                            TranscriptionMode::Automatic => "automatic",
                            TranscriptionMode::PushToTalk => "push-to-talk",
                        };
                        println!("Mode: {}", mode_str);

                        if let Some(ref source) = status.source1_id {
                            println!("Source: {}", source.dimmed());
                        }
                        if let Some(ref source2) = status.source2_id {
                            println!("Source 2: {}", source2.dimmed());
                        }

                        if let Some(error) = &status.error {
                            println!("Error: {}", error.red());
                        }

                        if status.capturing {
                            let speech_str = if status.in_speech {
                                "speaking".green()
                            } else {
                                "silent".dimmed()
                            };
                            println!("Speech: {}", speech_str);
                            println!("Queue depth: {}", status.queue_depth);
                        }
                    }
                }
                Response::Error { message } => return Err(message.into()),
                _ => return Err("Unexpected response".into()),
            }
        }

        Commands::Stop => {
            // Clear sources to stop capture
            let response = client
                .request(Request::SetSources {
                    source1_id: None,
                    source2_id: None,
                })
                .await
                .map_err(|e| e.to_string())?;

            match response {
                Response::Ok => {
                    if !cli.quiet {
                        println!("{}", "Capture stopped".green());
                    }
                }
                Response::Error { message } => return Err(message.into()),
                _ => return Err("Unexpected response".into()),
            }
        }

        Commands::Model { action } => {
            match action {
                Some(ModelAction::Download) => {
                    if !cli.quiet {
                        println!("Downloading Whisper model...");
                    }

                    let response = client
                        .request(Request::DownloadModel)
                        .await
                        .map_err(|e| e.to_string())?;

                    match response {
                        Response::Ok => {
                            if !cli.quiet {
                                println!("{}", "Model download started".green());
                            }
                        }
                        Response::Error { message } => {
                            if message.contains("already downloaded") {
                                println!("{}", "Model already downloaded".yellow());
                            } else {
                                return Err(message.into());
                            }
                        }
                        _ => return Err("Unexpected response".into()),
                    }
                }
                None => {
                    // Show model status
                    let response = client
                        .request(Request::GetModelStatus)
                        .await
                        .map_err(|e| e.to_string())?;

                    match response {
                        Response::ModelStatus(status) => {
                            if matches!(cli.format, OutputFormat::Json) {
                                println!("{}", serde_json::to_string_pretty(&status).unwrap());
                            } else {
                                let available_str = if status.available {
                                    "available".green().bold()
                                } else {
                                    "not available".red()
                                };
                                println!("Model: {}", available_str);
                                println!("Path: {}", status.path.dimmed());

                                if !status.available {
                                    println!(
                                        "\nRun {} to download the model",
                                        "'flowstt model download'".cyan()
                                    );
                                }
                            }
                        }
                        Response::Error { message } => return Err(message.into()),
                        _ => return Err("Unexpected response".into()),
                    }
                }
            }
        }

        Commands::Gpu => {
            let response = client
                .request(Request::GetCudaStatus)
                .await
                .map_err(|e| e.to_string())?;

            match response {
                Response::CudaStatus(status) => {
                    if matches!(cli.format, OutputFormat::Json) {
                        println!("{}", serde_json::to_string_pretty(&status).unwrap());
                    } else {
                        let build_str = if status.build_enabled {
                            "enabled".green()
                        } else {
                            "disabled".dimmed()
                        };
                        let runtime_str = if status.runtime_available {
                            "available".green().bold()
                        } else {
                            "not available".dimmed()
                        };

                        println!("GPU Acceleration");
                        println!("  Build: {}", build_str);
                        println!("  Runtime: {}", runtime_str);
                        println!("\nSystem Info:");
                        println!("  {}", status.system_info.dimmed());
                    }
                }
                Response::Error { message } => return Err(message.into()),
                _ => return Err("Unexpected response".into()),
            }
        }

        Commands::Ping => match client.ping().await {
            Ok(true) => {
                if matches!(cli.format, OutputFormat::Json) {
                    println!(r#"{{"status": "ok"}}"#);
                } else {
                    println!("{}", "pong".green());
                }
            }
            Ok(false) => return Err("Service not responding".into()),
            Err(e) => return Err(e.to_string().into()),
        },

        Commands::Shutdown => {
            let response = client
                .request(Request::Shutdown)
                .await
                .map_err(|e| e.to_string())?;

            match response {
                Response::Ok => {
                    if !cli.quiet {
                        println!("{}", "Service shutdown initiated".green());
                    }
                }
                Response::Error { message } => return Err(message.into()),
                _ => return Err("Unexpected response".into()),
            }
        }

        Commands::ToggleAuto => {
            let response = client
                .request(Request::ToggleAutoMode)
                .await
                .map_err(|e| e.to_string())?;

            match response {
                Response::Ok => {
                    // Get the new mode from state
                    let status_response = client
                        .request(Request::GetPttStatus)
                        .await
                        .map_err(|e| e.to_string())?;
                    if let Response::PttStatus(status) = status_response {
                        let mode_str = match status.mode {
                            TranscriptionMode::Automatic => "Automatic",
                            TranscriptionMode::PushToTalk => "Push-to-Talk",
                        };
                        if !cli.quiet {
                            println!("{} transcription mode: {}", "Toggled".green().bold(), mode_str);
                        }
                        if matches!(cli.format, OutputFormat::Json) {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&serde_json::json!({
                                    "mode": status.mode
                                }))
                                .map_err(|e| e.to_string())?
                            );
                        }
                    }
                }
                Response::Error { message } => return Err(message.into()),
                _ => return Err("Unexpected response".into()),
            }
        }

        Commands::Setup => {
            // Already handled above
            unreachable!()
        }

        Commands::Config { .. } => {
            // Already handled above
            unreachable!()
        }

        Commands::Version => {
            // Already handled above
            unreachable!()
        }
    }

    Ok(())
}

/// Handle config subcommands. Tries IPC first, falls back to direct file access.
async fn handle_config(
    client: &mut Client,
    action: &ConfigAction,
    cli: &Cli,
) -> Result<(), CliError> {
    match action {
        ConfigAction::Show => handle_config_show(client, cli).await,
        ConfigAction::Get { key } => handle_config_get(client, key, cli).await,
        ConfigAction::Set { key, value } => handle_config_set(client, key, value, cli).await,
    }
}

/// Retrieve config values from the service or fall back to the config file.
async fn get_config_values(client: &mut Client) -> Result<ConfigValues, CliError> {
    // Try connecting to the service
    if client.connect().await.is_ok() {
        let response = client
            .request(Request::GetConfig)
            .await
            .map_err(|e| e.to_string())?;
        match response {
            Response::ConfigValues(values) => return Ok(values),
            Response::Error { message } => return Err(CliError::general(message)),
            _ => return Err(CliError::general("Unexpected response from service")),
        }
    }

    // Service not running -- read from disk
    let config = Config::load();
    Ok(ConfigValues {
        transcription_mode: config.transcription_mode,
        ptt_hotkeys: config.ptt_hotkeys,
        auto_toggle_hotkeys: config.auto_toggle_hotkeys,
        auto_paste_enabled: config.auto_paste_enabled,
        auto_paste_delay_ms: config.auto_paste_delay_ms,
    })
}

/// Validate that a config key name is recognized.
fn validate_config_key(key: &str) -> Result<(), CliError> {
    if VALID_CONFIG_KEYS.contains(&key) {
        Ok(())
    } else {
        Err(CliError::usage(format!(
            "Unknown configuration key '{}'. Valid keys: {}",
            key,
            VALID_CONFIG_KEYS.join(", ")
        )))
    }
}

/// Format hotkeys for human-readable display.
fn format_hotkeys_display(hotkeys: &[HotkeyCombination]) -> String {
    if hotkeys.is_empty() {
        "(none)".to_string()
    } else {
        hotkeys
            .iter()
            .map(|h| h.display())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Handle `config show` -- display all config values.
async fn handle_config_show(client: &mut Client, cli: &Cli) -> Result<(), CliError> {
    let values = get_config_values(client).await?;

    if matches!(cli.format, OutputFormat::Json) {
        println!(
            "{}",
            serde_json::to_string_pretty(&values).map_err(|e| e.to_string())?
        );
    } else {
        let mode_str = match values.transcription_mode {
            TranscriptionMode::Automatic => "automatic",
            TranscriptionMode::PushToTalk => "push_to_talk",
        };
        println!("{}: {}", "transcription_mode".bold(), mode_str);
        println!(
            "{}: {}",
            "ptt_hotkeys".bold(),
            format_hotkeys_display(&values.ptt_hotkeys)
        );
        println!(
            "{}: {}",
            "auto_toggle_hotkeys".bold(),
            format_hotkeys_display(&values.auto_toggle_hotkeys)
        );
    }

    Ok(())
}

/// Handle `config get <key>` -- display a single config value.
async fn handle_config_get(
    client: &mut Client,
    key: &str,
    cli: &Cli,
) -> Result<(), CliError> {
    validate_config_key(key)?;

    let values = get_config_values(client).await?;

    match key {
        "transcription_mode" => {
            if matches!(cli.format, OutputFormat::Json) {
                println!(
                    "{}",
                    serde_json::to_value(values.transcription_mode)
                        .map_err(|e| e.to_string())?
                );
            } else {
                let mode_str = match values.transcription_mode {
                    TranscriptionMode::Automatic => "automatic",
                    TranscriptionMode::PushToTalk => "push_to_talk",
                };
                println!("{}", mode_str);
            }
        }
        "ptt_hotkeys" => {
            if matches!(cli.format, OutputFormat::Json) {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&values.ptt_hotkeys)
                        .map_err(|e| e.to_string())?
                );
            } else {
                println!("{}", format_hotkeys_display(&values.ptt_hotkeys));
            }
        }
        "auto_toggle_hotkeys" => {
            if matches!(cli.format, OutputFormat::Json) {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&values.auto_toggle_hotkeys)
                        .map_err(|e| e.to_string())?
                );
            } else {
                println!("{}", format_hotkeys_display(&values.auto_toggle_hotkeys));
            }
        }
        _ => unreachable!(), // validate_config_key already checked
    }

    Ok(())
}

/// Handle `config set <key> <value>` -- update a config value.
async fn handle_config_set(
    client: &mut Client,
    key: &str,
    value: &str,
    cli: &Cli,
) -> Result<(), CliError> {
    validate_config_key(key)?;

    // Try connecting to the service first
    let service_available = client.connect().await.is_ok();

    match key {
        "transcription_mode" => {
            let mode = match value {
                "automatic" => TranscriptionMode::Automatic,
                "push_to_talk" => TranscriptionMode::PushToTalk,
                _ => {
                    return Err(CliError::usage(format!(
                        "Invalid value '{}' for transcription_mode. Expected: automatic, push_to_talk",
                        value
                    )));
                }
            };

            if service_available {
                let response = client
                    .request(Request::SetTranscriptionMode { mode })
                    .await
                    .map_err(|e| e.to_string())?;
                match response {
                    Response::Ok => {}
                    Response::Error { message } => return Err(CliError::general(message)),
                    _ => return Err(CliError::general("Unexpected response")),
                }
            } else {
                // Offline: write directly to config file
                let mut config = Config::load();
                config.transcription_mode = mode;
                config
                    .save()
                    .map_err(|e| CliError::general(format!("Failed to save config: {}", e)))?;
            }

            if !cli.quiet {
                println!(
                    "{} transcription_mode = {}",
                    "Set".green().bold(),
                    value
                );
            }
        }
        "ptt_hotkeys" => {
            let hotkeys: Vec<HotkeyCombination> =
                serde_json::from_str(value).map_err(|e| {
                    CliError::usage(format!(
                        "Invalid JSON for ptt_hotkeys: {}\nExpected format: {}",
                        e,
                        r#"'[{"keys":["left_control","left_alt"]}]'"#
                    ))
                })?;

            if service_available {
                let response = client
                    .request(Request::SetPushToTalkHotkeys {
                        hotkeys: hotkeys.clone(),
                    })
                    .await
                    .map_err(|e| e.to_string())?;
                match response {
                    Response::Ok => {}
                    Response::Error { message } => return Err(CliError::general(message)),
                    _ => return Err(CliError::general("Unexpected response")),
                }
            } else {
                // Offline: write directly to config file
                let mut config = Config::load();
                config.ptt_hotkeys = hotkeys.clone();
                config
                    .save()
                    .map_err(|e| CliError::general(format!("Failed to save config: {}", e)))?;
            }

            if !cli.quiet {
                println!(
                    "{} ptt_hotkeys = {}",
                    "Set".green().bold(),
                    format_hotkeys_display(&hotkeys)
                );
            }
        }
        "auto_toggle_hotkeys" => {
            let hotkeys: Vec<HotkeyCombination> = if value == "null" || value == "none" || value == "[]" {
                vec![]
            } else {
                serde_json::from_str(value).map_err(|e| {
                    CliError::usage(format!(
                        "Invalid JSON for auto_toggle_hotkeys: {}\nExpected format: {} or []",
                        e,
                        r#"[{"keys":["f13"]}]"#
                    ))
                })?
            };

            if service_available {
                let response = client
                    .request(Request::SetAutoToggleHotkeys {
                        hotkeys: hotkeys.clone(),
                    })
                    .await
                    .map_err(|e| e.to_string())?;
                match response {
                    Response::Ok => {}
                    Response::Error { message } => return Err(CliError::general(message)),
                    _ => return Err(CliError::general("Unexpected response")),
                }
            } else {
                // Offline: write directly to config file
                let mut config = Config::load();
                config.auto_toggle_hotkeys = hotkeys.clone();
                config
                    .save()
                    .map_err(|e| CliError::general(format!("Failed to save config: {}", e)))?;
            }

            if !cli.quiet {
                println!(
                    "{} auto_toggle_hotkeys = {}",
                    "Set".green().bold(),
                    format_hotkeys_display(&hotkeys)
                );
            }
        }
        _ => unreachable!(), // validate_config_key already checked
    }

    Ok(())
}

/// Handle the `setup` interactive wizard command.
async fn handle_setup(client: &mut Client, _cli: &Cli) -> Result<(), CliError> {
    use std::io::{self, BufRead, IsTerminal, Write};

    // TTY detection
    if !std::io::stdin().is_terminal() {
        return Err(CliError::new(
            "Setup requires an interactive terminal (TTY)",
            1,
        ));
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    // Check if already configured
    if !Config::needs_setup() {
        print!(
            "{}: Setup has already been completed. Run again? [y/N] ",
            "Warning".yellow().bold()
        );
        stdout.flush().unwrap();
        let mut answer = String::new();
        stdin.lock().read_line(&mut answer).unwrap();
        if !answer.trim().eq_ignore_ascii_case("y") {
            println!("Setup cancelled.");
            return Ok(());
        }
    }

    println!("\n{}\n", "FlowSTT Setup".bold());

    // Connect to service (needed for model download and device listing)
    client
        .connect_or_spawn()
        .await
        .map_err(|e| format!("Failed to connect to service: {}", e))?;

    // --- Step 1: Model Download ---
    println!("{}", "Step 1: Speech Model".bold());
    let model_response = client
        .request(Request::GetModelStatus)
        .await
        .map_err(|e| e.to_string())?;

    match model_response {
        Response::ModelStatus(status) if status.available => {
            println!("  Model: {}", "already downloaded".green());
            println!("  Path: {}", status.path.dimmed());
        }
        _ => {
            print!("  Download Whisper model (~145 MB)? [Y/n] ");
            stdout.flush().unwrap();
            let mut answer = String::new();
            stdin.lock().read_line(&mut answer).unwrap();
            if answer.trim().is_empty() || answer.trim().eq_ignore_ascii_case("y") {
                println!("  Downloading...");
                let response = client
                    .request(Request::DownloadModel)
                    .await
                    .map_err(|e| e.to_string())?;
                match response {
                    Response::Ok => {
                        // Wait for download to complete by polling model status
                        loop {
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            if let Ok(Response::ModelStatus(s)) =
                                client.request(Request::GetModelStatus).await
                            {
                                if s.available {
                                    println!("  {}", "Download complete!".green());
                                    break;
                                }
                            }
                            print!(".");
                            stdout.flush().unwrap();
                        }
                    }
                    Response::Error { message } => {
                        println!("  Download failed: {}", message.red());
                    }
                    _ => {}
                }
            } else {
                println!("  Skipping model download.");
            }
        }
    }

    // --- Step 2: Device Selection ---
    println!("\n{}", "Step 2: Microphone".bold());
    let device_response = client
        .request(Request::ListDevices {
            source_type: Some(AudioSourceType::Input),
        })
        .await
        .map_err(|e| e.to_string())?;

    let mut selected_device_id: Option<String> = None;

    match device_response {
        Response::Devices { devices } if !devices.is_empty() => {
            println!("  Available input devices:");
            for (i, device) in devices.iter().enumerate() {
                println!("    {}: {}", (i + 1).to_string().cyan(), device.name);
            }
            print!("  Select device [1-{}]: ", devices.len());
            stdout.flush().unwrap();
            let mut answer = String::new();
            stdin.lock().read_line(&mut answer).unwrap();
            if let Ok(idx) = answer.trim().parse::<usize>() {
                if idx >= 1 && idx <= devices.len() {
                    selected_device_id = Some(devices[idx - 1].id.clone());
                    println!("  Selected: {}", devices[idx - 1].name.green());
                }
            }
            if selected_device_id.is_none() {
                println!("  {}", "No device selected, skipping.".yellow());
            }
        }
        _ => {
            println!("  {}", "No input devices found.".yellow());
        }
    }

    // --- Step 3: Transcription Mode ---
    println!("\n{}", "Step 3: Transcription Mode".bold());
    println!("  1: {} - always listening, VAD-triggered", "Automatic".cyan());
    println!(
        "  2: {} - hold a key to transcribe (default)",
        "Push-to-Talk".cyan()
    );
    print!("  Select mode [1-2, default=2]: ");
    stdout.flush().unwrap();
    let mut answer = String::new();
    stdin.lock().read_line(&mut answer).unwrap();

    let mode = match answer.trim() {
        "1" => TranscriptionMode::Automatic,
        _ => TranscriptionMode::PushToTalk,
    };

    let mode_name = match mode {
        TranscriptionMode::Automatic => "Automatic",
        TranscriptionMode::PushToTalk => "Push-to-Talk",
    };
    println!("  Selected: {}", mode_name.green());

    let mut hotkey = HotkeyCombination::single(KeyCode::default());

    if mode == TranscriptionMode::PushToTalk {
        print!(
            "  PTT key [default=RightAlt, or type key name e.g. f5, left_control]: "
        );
        stdout.flush().unwrap();
        let mut key_answer = String::new();
        stdin.lock().read_line(&mut key_answer).unwrap();
        let key_str = key_answer.trim();
        if !key_str.is_empty() {
            // Try to parse the key name via serde
            let key_json = format!("\"{}\"", key_str);
            match serde_json::from_str::<KeyCode>(&key_json) {
                Ok(key) => {
                    hotkey = HotkeyCombination::single(key);
                    println!("  PTT key: {}", key_str.green());
                }
                Err(_) => {
                    println!(
                        "  {}: Unknown key '{}', using RightAlt",
                        "Warning".yellow(),
                        key_str
                    );
                }
            }
        } else {
            println!("  PTT key: {}", "RightAlt".green());
        }
    }

    // --- Step 4: Auto-mode Toggle Hotkey ---
    let mut toggle_hotkeys: Vec<HotkeyCombination> = vec![HotkeyCombination::single(KeyCode::F13)];
    
    println!("\n{}", "Step 4: Auto-mode Toggle Hotkey".bold());
    println!("  This hotkey toggles between Automatic and Push-to-Talk modes.");
    print!("  Toggle key [default=F13, or type key name, or 'none' to disable]: ");
    stdout.flush().unwrap();
    let mut toggle_answer = String::new();
    stdin.lock().read_line(&mut toggle_answer).unwrap();
    let toggle_str = toggle_answer.trim();
    
    if toggle_str.eq_ignore_ascii_case("none") || toggle_str.eq_ignore_ascii_case("disabled") {
        toggle_hotkeys = vec![];
        println!("  Toggle hotkey: {}", "disabled".yellow());
    } else if !toggle_str.is_empty() {
        let key_json = format!("\"{}\"", toggle_str);
        match serde_json::from_str::<KeyCode>(&key_json) {
            Ok(key) => {
                toggle_hotkeys = vec![HotkeyCombination::single(key)];
                println!("  Toggle hotkey: {}", toggle_str.green());
            }
            Err(_) => {
                println!(
                    "  {}: Unknown key '{}', using F13",
                    "Warning".yellow(),
                    toggle_str
                );
            }
        }
    } else {
        println!("  Toggle hotkey: {}", "F13".green());
    }

    // --- Save config ---
    println!("\n{}", "Saving configuration...".bold());
    let config = Config {
        transcription_mode: mode,
        ptt_hotkeys: vec![hotkey],
        auto_toggle_hotkeys: toggle_hotkeys,
        ..Config::default_with_hotkeys()
    };
    config
        .save()
        .map_err(|e| CliError::general(format!("Failed to save config: {}", e)))?;

    // Configure service with chosen device
    if let Some(ref device_id) = selected_device_id {
        let _ = client
            .request(Request::SetSources {
                source1_id: Some(device_id.clone()),
                source2_id: None,
            })
            .await;
    }

    // Set mode on service
    let _ = client
        .request(Request::SetTranscriptionMode { mode })
        .await;

    println!("\n{}", "Setup complete!".green().bold());
    println!(
        "  Config saved to: {}",
        Config::config_path().display().to_string().dimmed()
    );

    Ok(())
}
