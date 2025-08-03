use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server_url: String,
    pub convex_url: Option<String>, // New: Convex backend URL
    pub device_id: Option<String>,
    pub device_key: Option<String>,
    pub device_serial: Option<String>, // New: Hardware serial number
    pub factory_secret: Option<String>, // New: Factory provisioning secret
    pub site_id: Option<String>,
    pub tenant_id: Option<String>,
    pub auth_token: Option<String>,
    pub api_key: Option<String>,
    pub simulation: SimulationConfig,
    pub hardware: HardwareConfig,
    pub recording: RecordingConfig,
    pub audio: AudioConfig,
    pub network: NetworkConfig,
    pub camera: CameraConfig,
    pub encryption: EncryptionConfig,
    pub power_management: PowerManagementConfig,
    pub sentry: Option<SentryConfig>,
    pub monitoring: MonitoringConfig,
    pub remote_config: RemoteConfig,
    pub security: SecurityConfig,
    pub storage: StorageConfig,
    pub streaming: StreamingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    pub enabled: bool,
    pub auto_incidents: bool,
    pub incident_frequency: u64,
    pub simulate_battery: bool,
    pub battery_drain_rate: f64,
    pub simulate_storage: bool,
    pub storage_usage_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareConfig {
    pub camera_index: Option<i32>,
    pub microphone: bool,
    pub gps: bool,
    pub accelerometer: bool,
    pub battery_capacity: u32,
    pub storage_capacity: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfig {
    pub resolution: String,
    pub fps: u32,
    pub bitrate: u32,
    pub duration_limit: Option<u64>,
    pub segment_duration: u64,
    pub encryption: bool,
    pub pre_incident_buffer_seconds: u64,
    pub default_quality: VideoQuality,
    pub available_qualities: Vec<VideoQualityConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub upload_bandwidth: u32,
    pub retry_attempts: u32,
    pub timeout: u64,
    pub compression: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    pub device_index: u32,
    pub device_name: String,
    pub resolution: String,
    pub fps: u32,
    pub format: String,
    pub exposure: String,
    pub white_balance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoQualityConfig {
    pub quality: VideoQuality,
    pub resolution: String,
    pub fps: u32,
    pub bitrate: u32,
    pub codec: String,
    pub stream_index: u32,
    pub device_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum VideoQuality {
    Low,
    Medium,
    High,
    Ultra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub enabled: bool,
    pub device_name: String,
    pub device_index: u32,
    pub device_path: Option<String>, // Added missing field
    pub sample_rate: u32,
    pub channels: u8,
    pub bitrate: u32,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    pub enabled: bool,
    pub key: Option<String>,
    pub algorithm: String,
    pub key_derivation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerManagementConfig {
    pub low_power_mode: bool,
    pub cpu_scaling: bool,
    pub adaptive_monitoring: bool,
    pub sleep_when_idle: bool,
    pub idle_timeout_seconds: u64,
    pub max_cpu_usage_percent: f64,
    pub background_task_delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub checkin_interval_seconds: u64,
    pub enable_real_time_updates: bool,
    pub enable_server_polling: bool,
    pub update_on_demand: bool,
    pub max_retry_attempts: u32,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteConfig {
    pub auto_update: bool,
    pub update_interval_seconds: u64,
    pub config_endpoint: String,
    pub last_update: Option<chrono::DateTime<chrono::Utc>>,
    pub config_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub enable_tamper_detection: bool,
    pub enable_encryption: bool,
    pub require_pin: bool,
    pub pin_code: Option<String>,
    pub auto_lock_timeout: u64,
    pub emergency_button_enabled: bool,
    pub single_press_action: Option<String>,
    pub double_press_action: Option<String>,
    pub long_press_action: Option<String>,
    pub triple_press_action: Option<String>,
    pub sos_enabled: bool,
    pub emergency_contacts: Vec<String>,
    pub auto_call_timeout: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub max_local_storage_gb: u32,
    pub auto_cleanup_days: u32,
    pub upload_on_wifi_only: bool,
    pub upload_on_charging_only: bool,
    pub max_file_size_mb: u32,
    pub compression_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    pub enable_live_streaming: bool,
    pub default_quality: VideoQuality,
    pub stream_timeout_seconds: u64,
    pub reconnect_attempts: u32,
    pub buffer_size_seconds: u32,
    pub adaptive_bitrate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryConfig {
    pub dsn: Option<String>,
    pub environment: Option<String>,
    pub sample_rate: Option<f32>,
    pub traces_sample_rate: Option<f32>,
    pub enable_tracing: Option<bool>,
    pub debug: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:3000".to_string(),
            convex_url: None, // Set via environment or config file
            device_id: None,
            device_key: None,
            device_serial: None,
            factory_secret: None,
            site_id: None,
            tenant_id: None,
            auth_token: None,
            api_key: None,
            simulation: SimulationConfig {
                enabled: false,
                auto_incidents: false,
                incident_frequency: 60,
                simulate_battery: true,
                battery_drain_rate: 0.5,
                simulate_storage: true,
                storage_usage_rate: 0.1,
            },
            hardware: HardwareConfig {
                camera_index: Some(0),
                microphone: true,
                gps: true,
                accelerometer: true,
                battery_capacity: 4000,
                storage_capacity: 64_000_000_000, // 64GB
            },
            recording: RecordingConfig {
                resolution: "1920x1080".to_string(),
                fps: 30,
                bitrate: 5_000_000,
                duration_limit: None,
                segment_duration: 300,
                encryption: true,
                pre_incident_buffer_seconds: 30,
                default_quality: VideoQuality::Low,
                available_qualities: vec![
                    VideoQualityConfig {
                        quality: VideoQuality::Low,
                        resolution: "640x480".to_string(),
                        fps: 15,
                        bitrate: 500_000,
                        codec: "h264".to_string(),
                        stream_index: 0,
                        device_path: "/dev/video0".to_string(),
                    },
                    VideoQualityConfig {
                        quality: VideoQuality::High,
                        resolution: "1920x1080".to_string(),
                        fps: 30,
                        bitrate: 5_000_000,
                        codec: "h264".to_string(),
                        stream_index: 1,
                        device_path: "/dev/video1".to_string(),
                    },
                ],
            },
            network: NetworkConfig {
                upload_bandwidth: 1_000_000,
                retry_attempts: 3,
                timeout: 30,
                compression: true,
            },
            camera: CameraConfig {
                device_index: 0,
                device_name: "Default Camera".to_string(),
                resolution: "1920x1080".to_string(),
                fps: 30,
                format: "MJPEG".to_string(),
                exposure: "auto".to_string(),
                white_balance: "auto".to_string(),
            },
            audio: AudioConfig {
                enabled: true,
                device_name: "Default Audio".to_string(),
                device_index: 0,
                device_path: None,
                sample_rate: 44100,
                channels: 2,
                bitrate: 128000,
                format: "AAC".to_string(),
            },
            encryption: EncryptionConfig {
                enabled: false,
                key: None,
                algorithm: "AES-256-GCM".to_string(),
                key_derivation: "Argon2id".to_string(),
            },
            power_management: PowerManagementConfig {
                low_power_mode: true,  // Enable by default for bodycams
                cpu_scaling: true,
                adaptive_monitoring: true,
                sleep_when_idle: true,
                idle_timeout_seconds: 300,  // 5 minutes
                max_cpu_usage_percent: 15.0,  // Keep CPU usage low
                background_task_delay_ms: 100,
            },
            sentry: None, // Sentry configuration is optional
            monitoring: MonitoringConfig {
                checkin_interval_seconds: 30, // Default 30 seconds
                enable_real_time_updates: true, // Enable Convex real-time
                enable_server_polling: true, // Enable polling fallback
                update_on_demand: true, // Enable server-requested updates
                max_retry_attempts: 3,
                timeout_seconds: 10,
            },
            remote_config: RemoteConfig {
                auto_update: true,
                update_interval_seconds: 3600, // 1 hour
                config_endpoint: "/api/devices/config".to_string(),
                last_update: None,
                config_version: "1.0.0".to_string(),
            },
            security: SecurityConfig {
                enable_tamper_detection: true,
                enable_encryption: false,
                require_pin: false,
                pin_code: None,
                auto_lock_timeout: 300, // 5 minutes
                emergency_button_enabled: true,
                single_press_action: Some("toggle_recording".to_string()),
                double_press_action: Some("take_photo".to_string()),
                long_press_action: Some("start_sos".to_string()),
                triple_press_action: Some("start_streaming".to_string()),
                sos_enabled: true,
                emergency_contacts: vec![],
                auto_call_timeout: 30,
            },
            storage: StorageConfig {
                max_local_storage_gb: 32,
                auto_cleanup_days: 7,
                upload_on_wifi_only: false,
                upload_on_charging_only: false,
                max_file_size_mb: 1024, // 1GB
                compression_level: 6,
            },
            streaming: StreamingConfig {
                enable_live_streaming: true,
                default_quality: VideoQuality::Medium,
                stream_timeout_seconds: 30,
                reconnect_attempts: 3,
                buffer_size_seconds: 5,
                adaptive_bitrate: true,
            },
        }
    }
}

impl Config {
    pub async fn load(path: &str) -> Result<Self> {
        let path = Path::new(path);
        
        if !path.exists() {
            let config = Config::default();
            config.save(path).await?;
            return Ok(config);
        }
        
        let content = tokio::fs::read_to_string(path).await?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub async fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        tokio::fs::write(path, content).await?;
        Ok(())
    }

    pub fn is_provisioned(&self) -> bool {
        self.device_id.is_some() && self.device_key.is_some() 
        && self.site_id.is_some() && self.tenant_id.is_some()
    }
}