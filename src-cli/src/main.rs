//! FlowSTT Command Line Interface
//!
//! This is the command-line interface for FlowSTT voice transcription.
//! It communicates with the background service via IPC.

mod client;

use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use flowstt_common::ipc::{Request, Response};
use flowstt_common::{AudioSourceType, RecordingMode};

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

            let response = client
                .request(Request::StartTranscribe {
                    source1_id: source1,
                    source2_id: source2,
                    aec_enabled: aec,
                    mode: recording_mode,
                    transcription_enabled: true,
                })
                .await
                .map_err(|e| e.to_string())?;

            match response {
                Response::Ok => {
                    if !cli.quiet {
                        println!("{}", "Transcription started".green());
                        println!("Press Ctrl+C to stop, or run 'flowstt stop'");
                    }

                    // Subscribe to events and stream transcription results
                    let subscribe_response = client
                        .request(Request::SubscribeEvents)
                        .await
                        .map_err(|e| e.to_string())?;

                    if !matches!(subscribe_response, Response::Subscribed) {
                        return Err("Failed to subscribe to events".into());
                    }

                    // Stream events until shutdown or Ctrl+C
                    loop {
                        // Read next event (blocking)
                        // TODO: Implement proper event streaming
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                        // Check if we should exit
                        // For now, just print status
                        let status_response = client
                            .request(Request::GetStatus)
                            .await
                            .map_err(|e| e.to_string())?;

                        if let Response::Status(status) = status_response {
                            if !status.active {
                                if !cli.quiet {
                                    println!("\n{}", "Transcription stopped".yellow());
                                }
                                break;
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
                        let active_str = if status.active {
                            "active".green().bold()
                        } else {
                            "inactive".dimmed()
                        };
                        println!("Transcription: {}", active_str);

                        if status.active {
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
            let response = client
                .request(Request::StopTranscribe)
                .await
                .map_err(|e| e.to_string())?;

            match response {
                Response::Ok => {
                    if !cli.quiet {
                        println!("{}", "Transcription stopped".green());
                    }
                }
                Response::Error { message } => {
                    if message.contains("not active") {
                        if !cli.quiet {
                            println!("{}", "Transcription is not active".yellow());
                        }
                    } else {
                        return Err(message);
                    }
                }
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
