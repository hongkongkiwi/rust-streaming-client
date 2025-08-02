use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::auth::Authenticator;
use crate::config::Config;
use crate::hardware::{HardwareInterface, HardwareEvent, LedState};
use crate::media::MediaRecorder;
use crate::status::StatusReporter;
use crate::incident::IncidentManager;
use crate::buffer::CircularBuffer;
use crate::audio::AudioManager;
use crate::gps::GpsManager;

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
    hardware: Box<dyn HardwareInterface>,
    recorder: Option<MediaRecorder>,
    buffer: CircularBuffer,
    status_reporter: StatusReporter,
    incident_manager: IncidentManager,
    audio_manager: AudioManager,
    gps_manager: GpsManager,
    device_id: Option<String>,
    device_key: Option<String>,
    is_recording: bool,
    current_incident_id: Option<String>,
}

impl BodycamDevice {
    pub async fn new(mut config: Config) -> Result<Self> {
        let simulation = config.simulation.enabled;
        let hardware = crate::hardware::create_hardware_interface(simulation);
        
        hardware.init(&config.hardware).await?;
        
        let auth = Authenticator::new(config.clone());
        let status_reporter = StatusReporter::new(config.clone());
        let incident_manager = IncidentManager::new(config.clone());
        let audio_manager = AudioManager::new(config.clone());
        let gps_manager = GpsManager::new(config.hardware.gps);
        
        // Check if device is provisioned
        let device_id = config.device_id.clone();
        let device_key = config.device_key.clone();
        
        let mut device = Self {
            config,
            auth,
            hardware,
            recorder: None,
            buffer: CircularBuffer::new(config.clone(), device_id.clone().unwrap_or_default()),
            status_reporter,
            incident_manager,
            audio_manager,
            gps_manager,
            device_id,
            device_key,
            is_recording: false,
            current_incident_id: None,
        };

        // Start hardware monitoring
        device.start_monitoring().await?;
        
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

    pub async fn register(&mut self, device_name: &str, site_id: &str
    ) -> Result<()> {
        println!("Registering device '{}' with site '{}'", device_name, site_id);
        
        let credentials = self.auth.provision_device(device_name, site_id).await?;
        
        self.device_id = Some(credentials.device_id.clone());
        self.device_key = Some(credentials.device_key.clone());
        
        self.config.device_id = Some(credentials.device_id);
        self.config.device_key = Some(credentials.device_key);
        self.config.site_id = Some(credentials.site_id);
        self.config.tenant_id = Some(credentials.tenant_id);
        
        self.config.save(std::path::Path::new("config.toml")).await?;
        
        println!("Device successfully registered!");
        Ok(())
    }

    pub async fn start_recording(
        &mut self,
        duration: Option<u64>,
        incident_id: Option<String>
    ) -> Result<()> {
        if self.is_recording {
            return Err(anyhow::anyhow!("Already recording"));
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

        let recorder = MediaRecorder::new(
            self.config.clone(),
            self.device_id.as_ref().unwrap().clone(),
            incident_id.clone(),
            duration,
        );

        recorder.start().await?;
        self.recorder = Some(recorder);
        self.is_recording = true;
        self.current_incident_id = incident_id;

        self.hardware.set_led("recording", LedState::On).await?;
        
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
            device_id: self.device_id.clone().unwrap_or_default(),
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
                self.device_id.as_ref().unwrap(),
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

        Ok(incident_id)
    }

    pub async fn start_streaming(&mut self
    ) -> Result<()> {
        if !self.config.is_provisioned() {
            return Err(anyhow::anyhow!("Device not provisioned"));
        }

        println!("Starting live streaming...");
        // TODO: Implement streaming
        Ok(())
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
        self.audio_manager.set_volume(volume).await
    }

    pub async fn diagnose(&self) -> Result<DiagnosticsReport> {
        let battery_level = self.hardware.get_battery_level().await?;
        let storage_info = self.hardware.get_storage_info().await?;
        let temperature = self.hardware.get_temperature().await?;

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
        ];

        Ok(DiagnosticsReport {
            device_id: self.device_id.clone().unwrap_or_default(),
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
                let device = device.lock().await;
                Self::handle_hardware_event(&device, event).await;
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
                
                if let Ok(status) = device.lock().await.get_status().await {
                    let _ = status_reporter.report_status(status).await;
                }
            }
        });

        Ok(())
    }

    async fn handle_hardware_event(
        device: &BodycamDevice,
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
}