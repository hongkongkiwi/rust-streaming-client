use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server_url: String,
    pub device_id: Option<String>,
    pub device_key: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub sample_rate: u32,
    pub channels: u8,
    pub bitrate: u32,
    pub format: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:3000".to_string(),
            device_id: None,
            device_key: None,
            site_id: None,
            tenant_id: None,
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
                sample_rate: 44100,
                channels: 2,
                bitrate: 128000,
                format: "AAC".to_string(),
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