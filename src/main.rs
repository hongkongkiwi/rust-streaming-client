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
mod validation;
mod streaming;
mod recovery;
mod encryption;
mod resource_manager;
mod diagnostics;
mod sentry_integration;
mod error_handling;
mod capabilities;
mod release_manager;

use config::Config;
use device::BodycamDevice;
use release_manager::{ReleaseManager, UpdateChannel};

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
    Stream {
        /// Streaming quality (low, medium, high, ultra)
        #[arg(short, long, default_value = "medium")]
        quality: String,
        
        /// Include audio in stream
        #[arg(short, long)]
        audio: bool,
    },
    
    /// Stop live streaming
    StopStream,
    
    /// Run basic diagnostics
    Diagnose,

    /// Run comprehensive diagnostics
    ComprehensiveDiagnose,

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
    
    /// Check for updates
    CheckUpdates {
        /// Update channel to check
        #[arg(short, long, default_value = "stable")]
        channel: String,
        
        /// Download if update is available
        #[arg(short, long)]
        download: bool,
        
        /// Apply update automatically
        #[arg(short, long)]
        apply: bool,
    },
    
    /// Update to latest version
    Update {
        /// Force update even if already up to date
        #[arg(short, long)]
        force: bool,
        
        /// Update channel to use
        #[arg(short, long, default_value = "stable")]
        channel: String,
    },
    
    /// Rollback to previous version
    Rollback {
        /// Force rollback without confirmation
        #[arg(short, long)]
        force: bool,
    },
    
    /// Show version information
    Version,
    
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
    
    // Initialize Sentry error tracking
    let sentry_config = sentry_integration::SentryConfig::from_config(&config);
    let _sentry_guard = sentry_integration::init_sentry(&sentry_config)?;
    
    // Set initial device context if available
    sentry_integration::set_device_context(
        config.device_id.as_deref(),
        config.site_id.as_deref(),
        config.tenant_id.as_deref(),
    );
    
    info!("Application configuration loaded and Sentry initialized");
    
    // Initialize device
    let mut device = BodycamDevice::new(config).await?;
    
    match cli.command {
        Commands::Register { name, site_id } => {
            sentry_integration::add_device_breadcrumb("register", Some(&format!("name: {}, site_id: {}", name, site_id)));
            match device.register(&name, &site_id).await {
                Ok(_) => {
                    info!("Device registered successfully");
                    sentry_integration::add_device_breadcrumb("register", Some("success"));
                }
                Err(e) => {
                    error!("Device registration failed: {}", e);
                    sentry_capture_error!(&e, "operation" => "device_register", "device_name" => name, "site_id" => site_id);
                    return Err(e);
                }
            }
        }
        Commands::Start { duration, incident_id } => {
            sentry_integration::add_device_breadcrumb("start_recording", 
                Some(&format!("duration: {:?}, incident_id: {:?}", duration, incident_id)));
            match device.start_recording(duration, incident_id).await {
                Ok(_) => {
                    info!("Recording started");
                    sentry_integration::add_device_breadcrumb("start_recording", Some("success"));
                }
                Err(e) => {
                    error!("Failed to start recording: {}", e);
                    sentry_capture_error!(&e, "operation" => "start_recording", "duration" => duration.unwrap_or(0), "incident_id" => incident_id.unwrap_or_default());
                    return Err(e);
                }
            }
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
            sentry_integration::add_device_breadcrumb("trigger_incident", 
                Some(&format!("type: {}, severity: {}", incident_type, severity)));
            match device.trigger_incident(&incident_type, &severity).await {
                Ok(incident_id) => {
                    info!("Incident triggered: {}", incident_id);
                    sentry_integration::add_device_breadcrumb("trigger_incident", Some("success"));
                }
                Err(e) => {
                    error!("Failed to trigger incident: {}", e);
                    sentry_capture_error!(&e, "operation" => "trigger_incident", "incident_type" => incident_type, "severity" => severity);
                    return Err(e);
                }
            }
        }
        Commands::Stream { quality, audio } => {
            let stream_id = device.start_streaming(Some(&quality), Some(audio)).await?;
            info!("Streaming started: {}", stream_id);
        }
        Commands::StopStream => {
            device.stop_streaming().await?;
            info!("Streaming stopped");
        }
        Commands::Diagnose => {
            let report = device.diagnose().await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::ComprehensiveDiagnose => {
            let comprehensive_report = device.run_comprehensive_diagnostics().await?;
            println!("{}", serde_json::to_string_pretty(&comprehensive_report)?);
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
        Commands::CheckUpdates { channel, download, apply } => {
            let channel = match channel.as_str() {
                "stable" => UpdateChannel::Stable,
                "beta" => UpdateChannel::Beta,
                "alpha" => UpdateChannel::Alpha,
                "development" => UpdateChannel::Development,
                _ => UpdateChannel::Stable,
            };

            let release_manager = ReleaseManager::new(
                &config_dir,
                "https://updates.patrolsight.com",
                env!("CARGO_PKG_VERSION"),
                channel,
            )?;

            match release_manager.check_for_updates().await? {
                Some(release) => {
                    println!("Update available: {} -> {}", 
                        release_manager.get_current_version(), 
                        release.version);
                    println!("Release date: {}", release.release_date);
                    println!("Size: {} bytes", release.size);
                    println!("Changelog:");
                    for change in &release.changelog {
                        println!("  - {}", change);
                    }

                    if download {
                        let download_path = release_manager.download_update(&release).await?;
                        println!("Downloaded to: {}", download_path.display());

                        if apply {
                            release_manager.apply_update(&download_path, &release).await?;
                            println!("Update applied. Restart required.");
                        }
                    }
                }
                None => {
                    println!("No updates available.");
                }
            }
        }
        Commands::Update { force, channel } => {
            let channel = match channel.as_str() {
                "stable" => UpdateChannel::Stable,
                "beta" => UpdateChannel::Beta,
                "alpha" => UpdateChannel::Alpha,
                "development" => UpdateChannel::Development,
                _ => UpdateChannel::Stable,
            };

            let release_manager = ReleaseManager::new(
                &config_dir,
                "https://updates.patrolsight.com",
                env!("CARGO_PKG_VERSION"),
                channel,
            )?;

            if !force {
                match release_manager.check_for_updates().await? {
                    Some(release) => {
                        let download_path = release_manager.download_update(&release).await?;
                        release_manager.apply_update(&download_path, &release).await?;
                        println!("Update applied. Restart required.");
                    }
                    None => {
                        println!("Already up to date.");
                    }
                }
            } else {
                println!("Force update requested...");
                // In a real implementation, this would fetch latest regardless
            }
        }
        Commands::Rollback { force } => {
            let release_manager = ReleaseManager::new(
                &config_dir,
                "https://updates.patrolsight.com",
                env!("CARGO_PKG_VERSION"),
                UpdateChannel::Stable,
            )?;

            if !force {
                print!("Are you sure you want to rollback? (y/N): ");
                use std::io::{self, Write};
                io::stdout().flush()?;
                
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Rollback cancelled.");
                    return Ok(());
                }
            }

            release_manager.rollback().await?;
            println!("Rollback completed. Restart required.");
        }
        Commands::Version => {
            println!("PatrolSight Client v{}", env!("CARGO_PKG_VERSION"));
            println!("Build date: {}", env!("BUILD_DATE", "unknown"));
            println!("Git commit: {}", env!("GIT_COMMIT", "unknown"));
            
            let release_manager = ReleaseManager::new(
                &config_dir,
                "https://updates.patrolsight.com",
                env!("CARGO_PKG_VERSION"),
                UpdateChannel::Stable,
            )?;
            
            println!("Current channel: {}", 
                match release_manager.get_update_channel() {
                    UpdateChannel::Stable => "stable",
                    UpdateChannel::Beta => "beta",
                    UpdateChannel::Alpha => "alpha",
                    UpdateChannel::Development => "development",
                });
        }
        Commands::Ui | _ => {
            if cli.headless {
                // Headless mode - run background services
                info!("Starting in headless mode");
                
                // Detect and report capabilities
                let detector = capabilities::CapabilityDetector::new(false);
                match detector.detect_capabilities().await {
                    Ok(caps) => {
                        info!("Device capabilities detected: {:#?}", caps);
                        
                        // Report capabilities to backend
                        if let Ok(json) = serde_json::to_string_pretty(&caps) {
                            info!("Capabilities JSON: {}", json);
                        }
                    }
                    Err(e) => {
                        error!("Failed to detect capabilities: {}", e);
                    }
                }
                
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
                // UI mode - use new Slint UI
                info!("Starting UI mode with comprehensive device capabilities");
                
                // Run the new Slint UI
                crate::ui::run_ui().await?;
            }
        }
    }
    
    Ok(())
}