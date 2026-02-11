//! FlowSTT Command Line Interface
//!
//! This is the command-line interface for FlowSTT voice transcription.
//! It communicates with the background service via IPC.

mod client;

use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use flowstt_common::ipc::{EventType, Request, Response};
use flowstt_common::{AudioSourceType, RecordingMode, TranscriptionMode};

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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        eprintln!("{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), String> {
    let mut client = Client::new();

    // Handle version separately (doesn't need service)
    if matches!(cli.command, Commands::Version) {
        println!("flowstt {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
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
                Response::Error { message } => return Err(message),
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
                Response::Error { message } => return Err(message),
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
                Response::Error { message } => return Err(message),
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
                Response::Error { message } => return Err(message),
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
                                return Err(message);
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
                        Response::Error { message } => return Err(message),
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
                Response::Error { message } => return Err(message),
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
            Err(e) => return Err(e.to_string()),
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
                Response::Error { message } => return Err(message),
                _ => return Err("Unexpected response".into()),
            }
        }

        Commands::Version => {
            // Already handled above
            unreachable!()
        }
    }

    Ok(())
}
