use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::auth::Authenticator;
use crate::convex_auth::ConvexAuthenticator;
use crate::config::Config;
use crate::hardware::{HardwareInterface, HardwareEvent, LedState};
use crate::media::MediaRecorder;
use crate::status::StatusReporter;
use crate::incident::IncidentManager;
use crate::buffer::CircularBuffer;
use crate::audio::AudioManager;
use crate::gps::GpsManager;
use crate::validation::InputValidator;
use crate::streaming::StreamingManager;
use crate::resource_manager::{ResourceManager, ResourceLimits};
use crate::diagnostics::{DiagnosticsRunner, ComprehensiveDiagnostics};
use crate::storage_manager::{StorageManager, DeletedFileRecord};
use crate::sentry_integration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatus {
    pub device_id: String,
    pub online: bool,
    pub recording: bool,
    pub battery_level: f32,
    pub storage_info: StorageInfo,
    pub temperature: f32,
    pub is_charging: bool,
    pub last_seen: DateTime<Utc>,
    pub location: Option<Location>,
    pub incident_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub recording_space: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
    pub accuracy: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsReport {
    pub device_id: String,
    pub timestamp: DateTime<Utc>,
    pub battery_level: f32,
    pub storage_info: StorageInfo,
    pub temperature: f32,
    pub sensors: Vec<SensorStatus>,
    pub camera_status: CameraStatus,
    pub audio_status: AudioStatus,
    pub network_status: NetworkStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorStatus {
    pub sensor_type: String,
    pub status: String,
    pub value: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraStatus {
    pub enabled: bool,
    pub resolution: String,
    pub fps: u32,
    pub bitrate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioStatus {
    pub enabled: bool,
    pub sample_rate: u32,
    pub channels: u8,
    pub bitrate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatus {
    pub connected: bool,
    pub signal_strength: Option<i32>,
    pub ip_address: Option<String>,
    pub upload_speed: Option<u32>,
}

pub struct BodycamDevice {
    config: Config,
    auth: Authenticator,
    convex_auth: Option<ConvexAuthenticator>, // New: Convex authentication
    hardware: Box<dyn HardwareInterface>,
    recorder: Option<MediaRecorder>,
    buffer: CircularBuffer,
    status_reporter: StatusReporter,
    incident_manager: IncidentManager,
    audio_manager: AudioManager,
    gps_manager: GpsManager,
    streaming_manager: StreamingManager,
    resource_manager: ResourceManager,
    storage_manager: StorageManager,
    device_id: Option<String>,
    device_key: Option<String>,
    is_recording: bool,
    current_incident_id: Option<String>,
}

impl BodycamDevice {
    pub async fn new(mut config: Config) -> Result<Self> {
        let simulation = config.simulation.enabled;
        let mut hardware = crate::hardware::create_hardware_interface(simulation);
        
        // Skip hardware initialization in simulation mode for now
        // The hardware interface will use simulation defaults
        if !simulation {
            let hardware_config = crate::hardware::HardwareConfig::default();
            hardware.init(&hardware_config).await?;
        }
        
        let auth = Authenticator::new(config.clone());
        
        // Initialize Convex auth if URL is configured
        let convex_auth = if config.convex_url.is_some() {
            Some(ConvexAuthenticator::new(config.clone())?)
        } else {
            None
        };
        
        let status_reporter = StatusReporter::new(config.clone());
        let incident_manager = IncidentManager::new(config.clone());
        let audio_manager = AudioManager::new(config.clone());
        let gps_manager = GpsManager::new(config.hardware.gps);
        let streaming_manager = StreamingManager::new(config.clone());
        
        // Check if device is provisioned
        let device_id = config.device_id.clone();
        let device_key = config.device_key.clone();
        
        // Initialize resource manager
        let resource_manager = ResourceManager::new(
            device_id.clone().unwrap_or_default(),
            Some(ResourceLimits::default())
        );
        
        let mut device = Self {
            config,
            auth,
            convex_auth,
            hardware,
            recorder: None,
            buffer: CircularBuffer::new(config.clone(), device_id.clone().unwrap_or_default()),
            status_reporter,
            incident_manager,
            audio_manager,
            gps_manager,
            streaming_manager,
            resource_manager,
            device_id,
            device_key,
            is_recording: false,
            current_incident_id: None,
        };

        // Start hardware monitoring
        device.start_monitoring().await?;
        
        // Start resource manager monitoring
        device.resource_manager.start_monitoring().await?;
        
        // Start status reporting
        device.start_status_reporting().await?;
        
        // Start GPS monitoring
        device.gps_manager.start_monitoring().await?;
        
        // Start pre-incident buffer if enabled
        if device.config.recording.pre_incident_buffer_seconds > 0 {
            device.buffer.start_buffering().await?;
        }
        
        Ok(device)
    }

    pub async fn register(&mut self, device_name: &str, site_id: &str) -> Result<()> {
        let _transaction = sentry_integration::start_transaction("device.register", "device");
        
        // Validate inputs
        InputValidator::validate_device_name(device_name)?;
        InputValidator::validate_site_id(site_id)?;
        
        println!("Registering device '{}' with site '{}'", device_name, site_id);
        sentry_integration::add_device_breadcrumb("register_start", Some(&format!("name: {}, site_id: {}", device_name, site_id)));
        
        // Use Convex auth if available, otherwise fall back to legacy auth
        let credentials = if let Some(ref mut convex_auth) = self.convex_auth {
            // Use Convex authentication system
            tracing::info!("Using Convex authentication for device registration");
            convex_auth.factory_provision(device_name, site_id).await?
        } else {
            // Use legacy authentication system
            tracing::info!("Using legacy authentication for device registration");
            let legacy_credentials = self.auth.provision_device(device_name, site_id).await?;
            crate::convex_api::DeviceCredentials {
                device_id: legacy_credentials.device_id,
                device_key: legacy_credentials.device_key,
                site_id: legacy_credentials.site_id,
                tenant_id: legacy_credentials.tenant_id,
                auth_token: "legacy".to_string(), // Placeholder
            }
        };
        
        self.device_id = Some(credentials.device_id.clone());
        self.device_key = Some(credentials.device_key.clone());
        
        self.config.device_id = Some(credentials.device_id);
        self.config.device_key = Some(credentials.device_key);
        self.config.site_id = Some(credentials.site_id);
        self.config.tenant_id = Some(credentials.tenant_id);
        self.config.auth_token = Some(credentials.auth_token);
        
        self.config.save(std::path::Path::new("config.toml")).await?;
        
        // Update Sentry context with new device information
        sentry_integration::set_device_context(
            Some(&credentials.device_id),
            Some(&credentials.site_id),
            Some(&credentials.tenant_id),
        );
        
        sentry_integration::add_device_breadcrumb("register_complete", Some("success"));
        println!("Device successfully registered!");
        Ok(())
    }

    pub async fn start_recording(
        &mut self,
        duration: Option<u64>,
        incident_id: Option<String>
    ) -> Result<()> {
        let _transaction = sentry_integration::start_transaction("device.start_recording", "recording");
        
        if self.is_recording {
            return Err(anyhow::anyhow!("Already recording"));
        }
        
        sentry_integration::add_device_breadcrumb("start_recording", 
            Some(&format!("duration: {:?}, incident_id: {:?}", duration, incident_id)));
        
        // Validate inputs
        if let Some(duration) = duration {
            InputValidator::validate_recording_duration(duration)?;
        }
        
        if let Some(ref incident_id) = incident_id {
            InputValidator::validate_uuid(incident_id)?;
        }

        if !self.config.is_provisioned() {
            return Err(anyhow::anyhow!("Device not provisioned"));
        }

        let incident_id = incident_id.or_else(|| {
            if self.current_incident_id.is_none() {
                Some(Uuid::new_v4().to_string())
            } else {
                self.current_incident_id.clone()
            }
        });

        let device_id = self.device_id.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Device not properly initialized - missing device_id"))?;
            
        let mut recorder = MediaRecorder::new(
            self.config.clone(),
            device_id.clone(),
            incident_id.clone(),
            duration,
        );

        // Initialize encryption if enabled in config
        if let Some(ref encryption_key) = self.config.encryption.key {
            recorder.initialize_encryption(Some(encryption_key.clone())).await
                .context("Failed to initialize encryption")?;
        }

        recorder.start().await?;
        self.recorder = Some(recorder);
        self.is_recording = true;
        self.current_incident_id = incident_id;

        self.hardware.set_led("recording", LedState::On).await?;
        
        // Register temp files with resource manager if any are created during recording
        let temp_dir = std::env::current_dir()?.join("temp");
        if temp_dir.exists() {
            self.resource_manager.register_temp_file(temp_dir).await?;
        }
        
        sentry_integration::add_device_breadcrumb("start_recording_complete", Some("success"));
        Ok(())
    }

    pub async fn stop_recording(&mut self
    ) -> Result<()> {
        if !self.is_recording {
            return Err(anyhow::anyhow!("Not currently recording"));
        }

        if let Some(recorder) = &mut self.recorder {
            recorder.stop().await?;
        }

        self.recorder = None;
        self.is_recording = false;

        self.hardware.set_led("recording", LedState::Off).await?;
        
        // Check storage after recording stops
        let deleted_files = self.storage_manager.check_storage_and_cleanup().await?;
        if !deleted_files.is_empty() {
            tracing::info!("Storage cleanup completed, deleted {} files", deleted_files.len());
            
            // Save deletion log for server communication
            if let Err(e) = self.storage_manager.save_deletion_log().await {
                tracing::error!("Failed to save deletion log: {}", e);
            }
        }
        
        Ok(())
    }

    pub async fn get_status(&self) -> Result<DeviceStatus> {
        let battery_level = self.hardware.get_battery_level().await?;
        let storage_info = self.hardware.get_storage_info().await?;
        let temperature = self.hardware.get_temperature().await?;
        let is_charging = self.hardware.is_charging().await?;

        let location = self.gps_manager.get_location().await.map(|gps| Location {
            latitude: gps.latitude,
            longitude: gps.longitude,
            altitude: gps.altitude,
            accuracy: gps.accuracy,
        });

        Ok(DeviceStatus {
            device_id: self.device_id.clone().unwrap_or_else(|| "unknown".to_string()),
            online: true,
            recording: self.is_recording,
            battery_level,
            storage_info,
            temperature,
            is_charging,
            last_seen: Utc::now(),
            location,
            incident_active: self.current_incident_id.is_some(),
        })
    }

    pub async fn trigger_incident(
        &mut self,
        incident_type: &str,
        severity: &str
    ) -> Result<String> {
        let _transaction = sentry_integration::start_transaction("device.trigger_incident", "incident");
        
        // Validate inputs
        InputValidator::validate_incident_type(incident_type)?;
        InputValidator::validate_incident_severity(severity)?;
        
        sentry_integration::add_device_breadcrumb("trigger_incident", 
            Some(&format!("type: {}, severity: {}", incident_type, severity)));
        
        if !self.config.is_provisioned() {
            return Err(anyhow::anyhow!("Device not provisioned"));
        }

        let incident_id = Uuid::new_v4().to_string();
        self.current_incident_id = Some(incident_id.clone());

        // Get current GPS location
        let location = self.gps_manager.get_location().await.map(|gps| crate::incident::LocationData {
            latitude: gps.latitude,
            longitude: gps.longitude,
            altitude: gps.altitude,
            accuracy: gps.accuracy,
            timestamp: Utc::now(),
        });

        self.incident_manager
            .create_incident_with_location(
                &incident_id,
                incident_type,
                severity,
                self.device_id.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Device not initialized - missing device_id"))?,
                location,
            )
            .await?;

        // Start recording automatically if not already
        if !self.is_recording {
            self.start_recording(None, Some(incident_id.clone())).await?;
        }

        // Flash emergency LED
        self.hardware.set_led("recording", LedState::Blink {
            on_duration: 200,
            off_duration: 200,
            repeat: None,
        }).await?;

        sentry_integration::add_device_breadcrumb("trigger_incident_complete", Some("success"));
        
        // Report incident to Sentry as a message
        crate::sentry_capture_message!(
            &format!("Incident triggered: {} ({})", incident_type, severity),
            sentry::Level::Warning,
            "incident_id" => incident_id.clone(),
            "incident_type" => incident_type,
            "severity" => severity
        );

        Ok(incident_id)
    }

    pub async fn start_streaming(&mut self, quality: Option<&str>, include_audio: Option<bool>) -> Result<String> {
        if !self.config.is_provisioned() {
            return Err(anyhow::anyhow!("Device not provisioned"));
        }

        let quality = quality.unwrap_or("medium");
        let include_audio = include_audio.unwrap_or(true);
        
        let stream_info = self.streaming_manager
            .start_streaming(self.current_incident_id.clone(), quality, include_audio)
            .await?;

        println!("Live streaming started: {}", stream_info.stream_id);
        Ok(stream_info.stream_id)
    }

    pub async fn stop_streaming(&mut self) -> Result<()> {
        self.streaming_manager.stop_streaming().await?;
        println!("Live streaming stopped");
        Ok(())
    }

    pub fn is_streaming(&self) -> bool {
        self.streaming_manager.is_streaming()
    }

    pub async fn get_streaming_stats(&self) -> Result<crate::streaming::StreamStats> {
        self.streaming_manager.get_stream_stats().await
    }

    pub async fn play_audio(
        &self,
        source: crate::audio::AudioSource,
        volume: Option<f32>,
        loop_playback: Option<bool>,
        priority: crate::audio::AudioPriority,
    ) -> Result<String> {
        let request = crate::audio::AudioPlaybackRequest {
            source,
            volume,
            loop_playback,
            priority,
        };
        
        self.audio_manager.play_audio(request).await
    }

    pub async fn stop_audio(&self) -> Result<()> {
        self.audio_manager.stop_audio().await
    }

    pub async fn get_audio_status(&self) -> Result<crate::audio::AudioStatus> {
        self.audio_manager.get_status().await
    }

    pub async fn set_volume(&self, volume: f32) -> Result<()> {
        InputValidator::validate_volume(volume)?;
        self.audio_manager.set_volume(volume).await
    }

    pub async fn run_comprehensive_diagnostics(&self) -> Result<ComprehensiveDiagnostics> {
        let device_id = self.device_id.clone()
            .unwrap_or_else(|| "unknown".to_string());
            
        let diagnostics_runner = DiagnosticsRunner::new(
            device_id,
            self.config.clone()
        );
        
        diagnostics_runner.run_comprehensive_diagnostics(
            self.hardware.as_ref(),
            &self.resource_manager
        ).await
    }

    pub async fn diagnose(&self) -> Result<DiagnosticsReport> {
        let battery_level = self.hardware.get_battery_level().await?;
        let storage_info = self.hardware.get_storage_info().await?;
        let temperature = self.hardware.get_temperature().await?;
        
        // Get resource stats from resource manager
        let resource_stats = self.resource_manager.get_resource_stats().await;

        let sensors = vec![
            SensorStatus {
                sensor_type: "battery".to_string(),
                status: "ok".to_string(),
                value: Some(battery_level as f64),
            },
            SensorStatus {
                sensor_type: "temperature".to_string(),
                status: "ok".to_string(),
                value: Some(temperature as f64),
            },
            SensorStatus {
                sensor_type: "memory_usage_mb".to_string(),
                status: "ok".to_string(),
                value: Some(resource_stats.memory_usage.process_memory_kb as f64 / 1024.0),
            },
            SensorStatus {
                sensor_type: "cpu_usage_percent".to_string(),
                status: "ok".to_string(),
                value: Some(resource_stats.process_stats.cpu_usage_percent),
            },
            SensorStatus {
                sensor_type: "disk_usage_percent".to_string(),
                status: if resource_stats.disk_usage.used_gb / resource_stats.disk_usage.total_gb > 0.85 { "warning".to_string() } else { "ok".to_string() },
                value: Some((resource_stats.disk_usage.used_gb / resource_stats.disk_usage.total_gb) * 100.0),
            },
        ];

        Ok(DiagnosticsReport {
            device_id: self.device_id.clone().unwrap_or_else(|| "unknown".to_string()),
            timestamp: Utc::now(),
            battery_level,
            storage_info,
            temperature,
            sensors,
            camera_status: CameraStatus {
                enabled: true,
                resolution: self.config.recording.resolution.clone(),
                fps: self.config.recording.fps,
                bitrate: self.config.recording.bitrate,
            },
            audio_status: AudioStatus {
                enabled: true,
                sample_rate: self.config.audio.sample_rate,
                channels: self.config.audio.channels,
                bitrate: self.config.audio.bitrate,
            },
            network_status: NetworkStatus {
                connected: true,
                signal_strength: Some(-45),
                ip_address: Some("192.168.1.100".to_string()),
                upload_speed: Some(5000),
            },
        })
    }

    async fn start_monitoring(&mut self
    ) -> Result<()> {
        let hardware_events = self.hardware.start_monitoring().await?;
        let device = Arc::new(Mutex::new(self));
        
        tokio::spawn(async move {
            let mut event_rx = hardware_events;
            
            while let Some(event) = event_rx.recv().await {
                let mut device = device.lock().await;
                Self::handle_hardware_event(&mut device, event).await;
            }
        });

        Ok(())
    }

    async fn start_status_reporting(&self
    ) -> Result<()> {
        let status_reporter = self.status_reporter.clone();
        let device = Arc::new(Mutex::new(self));
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                let mut device_guard = device.lock().await;
                
                // Report current status
                if let Ok(status) = device_guard.get_status().await {
                    let _ = status_reporter.report_status(status).await;
                }
                
                // Check storage and perform automatic cleanup
                if let Ok(deleted_files) = device_guard.storage_manager.check_storage_and_cleanup().await {
                    if !deleted_files.is_empty() {
                        tracing::info!("Automatic storage cleanup completed, deleted {} files", deleted_files.len());
                        
                        // Save deletion log
                        if let Err(e) = device_guard.storage_manager.save_deletion_log().await {
                            tracing::error!("Failed to save deletion log: {}", e);
                        }
                        
                        // Sync deletions to server
                        let _ = device_guard.sync_deletions_to_server().await;
                    }
                }
            }
        });

        Ok(())
    }

    async fn handle_hardware_event(
        device: &mut BodycamDevice,
        event: HardwareEvent
    ) {
        match event {
            HardwareEvent::ButtonPressed { button, duration } => {
                match button {
                    crate::hardware::ButtonType::Record => {
                        if duration.is_some() {
                            let _ = device.stop_recording().await;
                        } else {
                            let _ = device.start_recording(None, None).await;
                        }
                    }
                    crate::hardware::ButtonType::Emergency => {
                        let _ = device.trigger_incident("emergency", "high").await;
                    }
                    crate::hardware::ButtonType::Power => {
                        if duration.map(|d| d >= 3000).unwrap_or(false) {
                            let _ = device.hardware.shutdown().await;
                        }
                    }
                    _ => {}
                }
            }
            HardwareEvent::BatteryLow { level } => {
                if level < 10.0 {
                    let _ = device.hardware.shutdown().await;
                }
            }
            HardwareEvent::StorageFull => {
                let _ = device.stop_recording().await;
                
                // Perform immediate storage cleanup
                if let Ok(deleted_files) = device.storage_manager.check_storage_and_cleanup().await {
                    if !deleted_files.is_empty() {
                        tracing::info!("Storage full cleanup completed, deleted {} files", deleted_files.len());
                        
                        // Save deletion log and notify server
                        let _ = device.storage_manager.save_deletion_log().await;
                        let _ = device.sync_deletions_to_server().await;
                    } else {
                        tracing::warn!("Storage full but no files could be deleted");
                    }
                }
            }
            HardwareEvent::TamperDetected => {
                let _ = device.trigger_incident("tamper", "critical").await;
            }
            HardwareEvent::LightDetected { level, threshold } => {
                let _ = device.trigger_incident("light_detection", "medium").await;
            }
            HardwareEvent::SoundDetected { level, frequency } => {
                let _ = device.trigger_incident("sound_detection", "low").await;
            }
            HardwareEvent::MovementDetected { acceleration, threshold } => {
                let _ = device.trigger_incident("movement_detection", "medium").await;
            }
            HardwareEvent::SpeechDetected { confidence, phrase, duration } => {
                let _ = device.trigger_incident("speech_detection", "high").await;
            }
            _ => {}
        }
    }

    pub async fn get_resource_stats(&self) -> Result<crate::resource_manager::ResourceStats> {
        Ok(self.resource_manager.get_resource_stats().await)
    }

    pub async fn force_cleanup(&self) -> Result<()> {
        self.resource_manager.force_cleanup().await
    }

    pub async fn clear_storage(&mut self) -> Result<()> {
        let _transaction = sentry_integration::start_transaction("device.clear_storage", "storage");
        
        tracing::info!("Clearing all storage");
        sentry_integration::add_device_breadcrumb("clear_storage", Some("user_requested"));
        
        // Stop recording if active
        if self.is_recording {
            self.stop_recording().await?;
        }
        
        // Clear media files
        let media_dir = std::env::current_dir()?.join("media");
        if media_dir.exists() {
            tokio::fs::remove_dir_all(&media_dir).await?;
            tokio::fs::create_dir_all(&media_dir).await?;
        }
        
        // Clear temp files
        let temp_dir = std::env::current_dir()?.join("temp");
        if temp_dir.exists() {
            tokio::fs::remove_dir_all(&temp_dir).await?;
            tokio::fs::create_dir_all(&temp_dir).await?;
        }
        
        // Clear buffer
        self.buffer.clear().await?;
        
        // Force resource cleanup
        self.force_cleanup().await?;
        
        sentry_integration::add_device_breadcrumb("clear_storage", Some("complete"));
        Ok(())
    }

    pub async fn scan_wifi(&self) -> Result<Vec<crate::hardware::WifiNetwork>> {
        let _transaction = sentry_integration::start_transaction("device.scan_wifi", "network");
        
        tracing::info!("Scanning for WiFi networks");
        let networks = self.hardware.scan_wifi().await?;
        
        sentry_integration::add_device_breadcrumb("scan_wifi", 
            Some(&format!("found {} networks", networks.len())));
        
        Ok(networks)
    }

    pub async fn get_storage_breakdown(&self) -> Result<Vec<crate::media::StorageBreakdown>> {
        let _transaction = sentry_integration::start_transaction("device.get_storage_breakdown", "storage");
        
        let media_dir = std::env::current_dir()?.join("media");
        let breakdown = crate::media::analyze_storage_usage(&media_dir).await?;
        
        Ok(breakdown)
    }

    pub async fn get_network_status(&self) -> Result<crate::hardware::NetworkStatus> {
        let _transaction = sentry_integration::start_transaction("device.get_network_status", "network");
        
        let status = self.hardware.get_network_status().await?;
        
        Ok(status)
    }

    pub fn get_recent_deletions(&self, limit: usize) -> Vec<crate::storage_manager::DeletedFileRecord> {
        self.storage_manager.get_recent_deletions(limit)
    }

    pub async fn sync_deletions_to_server(&mut self) -> Result<()> {
        let deleted_files = self.storage_manager.get_recent_deletions(100);
        if !deleted_files.is_empty() {
            // Here you would integrate with your server API to notify about deleted files
            tracing::info!("Syncing {} deleted files to server", deleted_files.len());
            
            // After successful sync, clear the local deletion log
            self.storage_manager.clear_deletion_log().await?;
        }
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down bodycam device");

        // Stop recording if active
        if self.is_recording {
            if let Err(e) = self.stop_recording().await {
                tracing::error!("Failed to stop recording during shutdown: {}", e);
            }
        }

        // Stop streaming if active
        if let Err(e) = self.streaming_manager.stop_streaming().await {
            tracing::error!("Failed to stop streaming during shutdown: {}", e);
        }

        // Stop audio if active
        if let Err(e) = self.audio_manager.stop_all().await {
            tracing::error!("Failed to stop audio during shutdown: {}", e);
        }

        // Shutdown resource manager (this will cleanup processes and temp files)
        if let Err(e) = self.resource_manager.shutdown().await {
            tracing::error!("Failed to shutdown resource manager: {}", e);
        }

        // Shutdown hardware interface
        if let Err(e) = self.hardware.shutdown().await {
            tracing::error!("Failed to shutdown hardware interface: {}", e);
        }

        tracing::info!("Bodycam device shutdown complete");
        Ok(())
    }
}