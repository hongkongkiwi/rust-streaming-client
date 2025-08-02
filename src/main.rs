use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{info, error};
use tracing_subscriber;
use std::sync::Arc;
use tokio::sync::Mutex;

mod auth;
mod config;
mod device;
mod media;
mod hardware;
mod status;
mod incident;
mod buffer;
mod audio;
mod simulation;
mod camera;
mod ui;
mod gps;
mod integrity;
mod api;

use config::Config;
use device::BodycamDevice;

#[derive(Parser)]
#[command(name = "bodycam-client")]
#[command(about = "Body camera client for security monitoring platform")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    #[arg(short, long, default_value = "config.toml")]
    config: String,
    
    #[arg(short, long)]
    verbose: bool,
    
    #[arg(long)]
    headless: bool,
    
    #[arg(long)]
    config_dir: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Register this device with the platform
    Register {
        /// Device name
        name: String,
        /// Site ID where device is deployed
        site_id: String,
    },
    
    /// Start recording and streaming
    Start {
        /// Recording duration in seconds (0 for continuous)
        #[arg(short, long)]
        duration: Option<u64>,
        
        /// Incident ID to associate recording with
        #[arg(short, long)]
        incident_id: Option<String>,
    },
    
    /// Stop recording
    Stop,
    
    /// Get device status
    Status,
    
    /// Simulate incident detection
    TriggerIncident {
        /// Incident type
        #[arg(short, long)]
        incident_type: String,
        
        /// Incident severity (low, medium, high, critical)
        #[arg(short, long)]
        severity: String,
    },
    
    /// Stream live feed
    Stream,
    
    /// Run diagnostics
    Diagnose,

    /// Play audio file or TTS
    PlayAudio {
        #[arg(short, long)]
        source: String,
        
        #[arg(short, long)]
        volume: Option<f32>,
        
        #[arg(short, long)]
        loop_playback: Option<bool>,
        
        #[arg(short, long)]
        preset: Option<String>,
        
        #[arg(short, long)]
        tts_text: Option<String>,
    },

    /// Stop audio playback
    StopAudio,

    /// Get audio status
    AudioStatus,

    /// Start interactive simulation mode
    Simulate,
    
    /// Start UI mode (default)
    Ui,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .init();
    
    info!("Starting bodycam client");
    
use std::path::PathBuf;

    // Determine config directory
    let config_dir = cli.config_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    
    let config_path = config_dir.join(&cli.config);
    
    // Load configuration
    let config = Config::load(config_path.to_str().unwrap()).await?;
    
    // Initialize device
    let mut device = BodycamDevice::new(config).await?;
    
    match cli.command {
        Commands::Register { name, site_id } => {
            device.register(&name, &site_id).await?;
            info!("Device registered successfully");
        }
        Commands::Start { duration, incident_id } => {
            device.start_recording(duration, incident_id).await?;
            info!("Recording started");
        }
        Commands::Stop => {
            device.stop_recording().await?;
            info!("Recording stopped");
        }
        Commands::Status => {
            let status = device.get_status().await?;
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        Commands::TriggerIncident { incident_type, severity } => {
            let incident_id = device.trigger_incident(&incident_type, &severity).await?;
            info!("Incident triggered: {}", incident_id);
        }
        Commands::Stream => {
            device.start_streaming().await?;
            info!("Streaming started");
        }
        Commands::Diagnose => {
            let report = device.diagnose().await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::PlayAudio { source, volume, loop_playback, preset, tts_text } => {
            let audio_source = if let Some(text) = tts_text {
                crate::audio::AudioSource::TtsLocal {
                    text,
                    voice: Some("en".to_string()),
                    rate: Some(150),
                }
            } else if let Some(preset_id) = preset {
                crate::audio::AudioSource::PresetFile { file_id: preset_id }
            } else {
                crate::audio::AudioSource::CustomFile { file_path: source }
            };
            
            let playback_id = device.play_audio(
                audio_source,
                volume,
                loop_playback,
                crate::audio::AudioPriority::Normal,
            ).await?;
            
            info!("Audio playback started: {}", playback_id);
        }
        Commands::StopAudio => {
            device.stop_audio().await?;
            info!("Audio playback stopped");
        }
        Commands::AudioStatus => {
            let status = device.get_audio_status().await?;
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        Commands::Simulate => {
            if !device.config.simulation.enabled {
                return Err(anyhow::anyhow!("Simulation mode not enabled in config"));
            }
            
            let device_arc = Arc::new(Mutex::new(device));
            let mut sim_repl = simulation::SimulationRepl::new(device_arc);
            sim_repl.run().await?;
        }
        Commands::Ui | _ => {
            if cli.headless {
                // Headless mode - run background services
                info!("Starting in headless mode");
                
                // Keep device running
                let device_arc = Arc::new(Mutex::new(device));
                
                // Start status reporting
                tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                        let device = device_arc.lock().unwrap();
                        if let Ok(status) = device.get_status().await {
                            let _ = device.status_reporter.report_status(status).await;
                        }
                    }
                });
                
                // Keep running
                tokio::signal::ctrl_c().await?;
                info!("Shutting down headless mode");
            } else {
                // UI mode
                info!("Starting UI mode");
                
                let camera_manager = crate::camera::CameraManager::new()?;
                let ui = crate::ui::BodycamUI::new(
                    device.config.clone(),
                    device,
                    Some(config_path),
                )?;
                
                ui.run().await?;
            }
        }
    }
    
    Ok(())
}