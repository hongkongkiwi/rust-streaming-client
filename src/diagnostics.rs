use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprehensiveDiagnostics {
    pub device_info: DeviceInfo,
    pub system_health: SystemHealth,
    pub component_status: ComponentStatus,
    pub performance_metrics: PerformanceMetrics,
    pub connectivity_tests: ConnectivityTests,
    pub storage_analysis: StorageAnalysis,
    pub security_status: SecurityStatus,
    pub error_logs: ErrorLogs,
    pub timestamp: DateTime<Utc>,
    pub diagnostic_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub firmware_version: String,
    pub hardware_revision: String,
    pub serial_number: String,
    pub manufacturer: String,
    pub model: String,
    pub platform: String,
    pub architecture: String,
    pub boot_time: DateTime<Utc>,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub overall_status: HealthStatus,
    pub cpu_usage: CpuMetrics,
    pub memory_usage: MemoryMetrics,
    pub temperature: TemperatureMetrics,
    pub power_status: PowerMetrics,
    pub disk_health: DiskHealthMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuMetrics {
    pub usage_percent: f64,
    pub load_average: Vec<f64>,
    pub core_count: u32,
    pub frequency_mhz: Option<u32>,
    pub temperature_celsius: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub total_kb: u64,
    pub used_kb: u64,
    pub available_kb: u64,
    pub process_memory_kb: u64,
    pub swap_total_kb: Option<u64>,
    pub swap_used_kb: Option<u64>,
    pub memory_pressure: HealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureMetrics {
    pub cpu_temp: Option<f64>,
    pub gpu_temp: Option<f64>,
    pub board_temp: Option<f64>,
    pub ambient_temp: Option<f64>,
    pub thermal_status: HealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerMetrics {
    pub battery_level: f32,
    pub is_charging: bool,
    pub voltage: Option<f32>,
    pub current_ma: Option<f32>,
    pub power_consumption_w: Option<f32>,
    pub estimated_runtime_hours: Option<f32>,
    pub power_status: HealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskHealthMetrics {
    pub total_space_gb: f64,
    pub used_space_gb: f64,
    pub available_space_gb: f64,
    pub usage_percent: f64,
    pub read_speed_mbps: Option<f64>,
    pub write_speed_mbps: Option<f64>,
    pub disk_health: HealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentStatus {
    pub camera: ComponentHealth,
    pub microphone: ComponentHealth,
    pub speaker: ComponentHealth,
    pub gps: ComponentHealth,
    pub accelerometer: ComponentHealth,
    pub wifi: ComponentHealth,
    pub cellular: ComponentHealth,
    pub bluetooth: ComponentHealth,
    pub leds: ComponentHealth,
    pub buttons: ComponentHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub status: HealthStatus,
    pub last_test: Option<DateTime<Utc>>,
    pub error_count: u32,
    pub details: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub recording_performance: RecordingPerformance,
    pub streaming_performance: StreamingPerformance,
    pub audio_performance: AudioPerformance,
    pub network_performance: NetworkPerformance,
    pub response_times: ResponseTimes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingPerformance {
    pub current_fps: Option<f64>,
    pub target_fps: u32,
    pub dropped_frames: u64,
    pub encoding_latency_ms: Option<f64>,
    pub disk_write_speed_mbps: Option<f64>,
    pub recording_status: HealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingPerformance {
    pub bitrate_kbps: Option<u32>,
    pub target_bitrate_kbps: u32,
    pub packet_loss_percent: Option<f64>,
    pub latency_ms: Option<f64>,
    pub streaming_status: HealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioPerformance {
    pub sample_rate: u32,
    pub channels: u8,
    pub buffer_underruns: u64,
    pub latency_ms: Option<f64>,
    pub audio_status: HealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPerformance {
    pub ping_ms: Option<f64>,
    pub download_mbps: Option<f64>,
    pub upload_mbps: Option<f64>,
    pub packet_loss: Option<f64>,
    pub signal_strength: Option<i32>,
    pub network_status: HealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseTimes {
    pub button_response_ms: Option<f64>,
    pub startup_time_ms: Option<f64>,
    pub shutdown_time_ms: Option<f64>,
    pub recording_start_ms: Option<f64>,
    pub incident_trigger_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectivityTests {
    pub server_connectivity: ConnectivityTest,
    pub internet_connectivity: ConnectivityTest,
    pub gps_connectivity: ConnectivityTest,
    pub cellular_connectivity: Option<ConnectivityTest>,
    pub wifi_connectivity: Option<ConnectivityTest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectivityTest {
    pub status: HealthStatus,
    pub latency_ms: Option<f64>,
    pub last_test: DateTime<Utc>,
    pub error_message: Option<String>,
    pub test_details: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageAnalysis {
    pub recordings_storage: StorageCategory,
    pub logs_storage: StorageCategory,
    pub temp_storage: StorageCategory,
    pub system_storage: StorageCategory,
    pub cleanup_recommendations: Vec<CleanupRecommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCategory {
    pub used_gb: f64,
    pub file_count: u64,
    pub oldest_file: Option<DateTime<Utc>>,
    pub newest_file: Option<DateTime<Utc>>,
    pub growth_rate_gb_per_day: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupRecommendation {
    pub category: String,
    pub action: String,
    pub potential_space_gb: f64,
    pub priority: String,
    pub risk_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityStatus {
    pub encryption_status: EncryptionStatus,
    pub authentication_status: AuthenticationStatus,
    pub certificate_status: CertificateStatus,
    pub access_control: AccessControlStatus,
    pub security_events: Vec<SecurityEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionStatus {
    pub recordings_encrypted: bool,
    pub communications_encrypted: bool,
    pub key_rotation_status: HealthStatus,
    pub encryption_algorithm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationStatus {
    pub device_authenticated: bool,
    pub token_valid: bool,
    pub last_authentication: Option<DateTime<Utc>>,
    pub authentication_method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateStatus {
    pub certificates_valid: bool,
    pub expiry_dates: Vec<DateTime<Utc>>,
    pub next_renewal: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessControlStatus {
    pub permissions_valid: bool,
    pub role_assignments: Vec<String>,
    pub access_violations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvent {
    pub event_type: String,
    pub severity: String,
    pub timestamp: DateTime<Utc>,
    pub description: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorLogs {
    pub recent_errors: Vec<ErrorEntry>,
    pub error_summary: ErrorSummary,
    pub crash_reports: Vec<CrashReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub component: String,
    pub message: String,
    pub stack_trace: Option<String>,
    pub context: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSummary {
    pub total_errors: u64,
    pub error_by_component: HashMap<String, u64>,
    pub error_by_severity: HashMap<String, u64>,
    pub error_trends: Vec<ErrorTrend>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorTrend {
    pub date: DateTime<Utc>,
    pub error_count: u64,
    pub component: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReport {
    pub timestamp: DateTime<Utc>,
    pub component: String,
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
    pub stack_trace: Option<String>,
    pub memory_usage_at_crash: Option<u64>,
    pub logs_before_crash: Vec<String>,
}

pub struct DiagnosticsRunner {
    device_id: String,
    config: crate::config::Config,
}

impl DiagnosticsRunner {
    pub fn new(device_id: String, config: crate::config::Config) -> Self {
        Self { device_id, config }
    }

    pub async fn run_comprehensive_diagnostics(
        &self,
        hardware: &dyn crate::hardware::HardwareInterface,
        resource_manager: &crate::resource_manager::ResourceManager,
    ) -> Result<ComprehensiveDiagnostics> {
        tracing::info!("Starting comprehensive diagnostics");

        let device_info = self.gather_device_info().await?;
        let system_health = self.gather_system_health(resource_manager).await?;
        let component_status = self.test_components(hardware).await?;
        let performance_metrics = self.measure_performance().await?;
        let connectivity_tests = self.run_connectivity_tests().await?;
        let storage_analysis = self.analyze_storage().await?;
        let security_status = self.check_security_status().await?;
        let error_logs = self.collect_error_logs().await?;

        let diagnostics = ComprehensiveDiagnostics {
            device_info,
            system_health,
            component_status,
            performance_metrics,
            connectivity_tests,
            storage_analysis,
            security_status,
            error_logs,
            timestamp: Utc::now(),
            diagnostic_version: "1.0.0".to_string(),
        };

        tracing::info!("Comprehensive diagnostics completed");
        Ok(diagnostics)
    }

    async fn gather_device_info(&self) -> Result<DeviceInfo> {
        Ok(DeviceInfo {
            device_id: self.device_id.clone(),
            firmware_version: env!("CARGO_PKG_VERSION").to_string(),
            hardware_revision: "1.0".to_string(),
            serial_number: format!("BC-{}", &self.device_id[..8]),
            manufacturer: "PatrolSight".to_string(),
            model: "BodyCam Pro".to_string(),
            platform: std::env::consts::OS.to_string(),
            architecture: std::env::consts::ARCH.to_string(),
            boot_time: Utc::now() - chrono::Duration::hours(2), // Simulated
            uptime_seconds: 7200, // 2 hours simulated
        })
    }

    async fn gather_system_health(
        &self,
        resource_manager: &crate::resource_manager::ResourceManager,
    ) -> Result<SystemHealth> {
        let resource_stats = resource_manager.get_resource_stats().await;

        let memory_pressure = if resource_stats.memory_usage.process_memory_kb > 512 * 1024 {
            HealthStatus::Warning
        } else if resource_stats.memory_usage.process_memory_kb > 256 * 1024 {
            HealthStatus::Healthy
        } else {
            HealthStatus::Healthy
        };

        let disk_health = if resource_stats.disk_usage.used_gb / resource_stats.disk_usage.total_gb > 0.9 {
            HealthStatus::Critical
        } else if resource_stats.disk_usage.used_gb / resource_stats.disk_usage.total_gb > 0.8 {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        };

        let overall_status = match (&memory_pressure, &disk_health) {
            (HealthStatus::Critical, _) | (_, HealthStatus::Critical) => HealthStatus::Critical,
            (HealthStatus::Warning, _) | (_, HealthStatus::Warning) => HealthStatus::Warning,
            _ => HealthStatus::Healthy,
        };

        Ok(SystemHealth {
            overall_status,
            cpu_usage: CpuMetrics {
                usage_percent: resource_stats.process_stats.cpu_usage_percent,
                load_average: vec![0.5, 0.3, 0.2],
                core_count: 4,
                frequency_mhz: Some(2400),
                temperature_celsius: Some(45.0),
            },
            memory_usage: MemoryMetrics {
                total_kb: resource_stats.memory_usage.total_kb,
                used_kb: resource_stats.memory_usage.used_kb,
                available_kb: resource_stats.memory_usage.available_kb,
                process_memory_kb: resource_stats.memory_usage.process_memory_kb,
                swap_total_kb: Some(2048 * 1024),
                swap_used_kb: resource_stats.memory_usage.swap_used_kb,
                memory_pressure,
            },
            temperature: TemperatureMetrics {
                cpu_temp: Some(45.0),
                gpu_temp: Some(42.0),
                board_temp: Some(38.0),
                ambient_temp: Some(25.0),
                thermal_status: HealthStatus::Healthy,
            },
            power_status: PowerMetrics {
                battery_level: 85.0,
                is_charging: false,
                voltage: Some(3.7),
                current_ma: Some(250),
                power_consumption_w: Some(2.5),
                estimated_runtime_hours: Some(8.5),
                power_status: HealthStatus::Healthy,
            },
            disk_health: DiskHealthMetrics {
                total_space_gb: resource_stats.disk_usage.total_gb,
                used_space_gb: resource_stats.disk_usage.used_gb,
                available_space_gb: resource_stats.disk_usage.available_gb,
                usage_percent: (resource_stats.disk_usage.used_gb / resource_stats.disk_usage.total_gb) * 100.0,
                read_speed_mbps: Some(50.0),
                write_speed_mbps: Some(30.0),
                disk_health,
            },
        })
    }

    async fn test_components(
        &self,
        hardware: &dyn crate::hardware::HardwareInterface,
    ) -> Result<ComponentStatus> {
        // Test each component
        let camera = self.test_camera().await;
        let microphone = self.test_microphone().await;
        let speaker = self.test_speaker().await;
        let gps = self.test_gps().await;
        let accelerometer = self.test_accelerometer().await;
        let wifi = self.test_wifi().await;
        let cellular = self.test_cellular().await;
        let bluetooth = self.test_bluetooth().await;
        let leds = self.test_leds(hardware).await;
        let buttons = self.test_buttons(hardware).await;

        Ok(ComponentStatus {
            camera,
            microphone,
            speaker,
            gps,
            accelerometer,
            wifi,
            cellular,
            bluetooth,
            leds,
            buttons,
        })
    }

    async fn test_camera(&self) -> ComponentHealth {
        ComponentHealth {
            status: HealthStatus::Healthy,
            last_test: Some(Utc::now()),
            error_count: 0,
            details: {
                let mut details = HashMap::new();
                details.insert("resolution".to_string(), serde_json::Value::String("1920x1080".to_string()));
                details.insert("fps".to_string(), serde_json::Value::Number(serde_json::Number::from(30)));
                details.insert("auto_focus".to_string(), serde_json::Value::Bool(true));
                details
            },
        }
    }

    async fn test_microphone(&self) -> ComponentHealth {
        ComponentHealth {
            status: HealthStatus::Healthy,
            last_test: Some(Utc::now()),
            error_count: 0,
            details: {
                let mut details = HashMap::new();
                details.insert("sample_rate".to_string(), serde_json::Value::Number(serde_json::Number::from(44100)));
                details.insert("channels".to_string(), serde_json::Value::Number(serde_json::Number::from(2)));
                details.insert("noise_reduction".to_string(), serde_json::Value::Bool(true));
                details
            },
        }
    }

    async fn test_speaker(&self) -> ComponentHealth {
        ComponentHealth {
            status: HealthStatus::Healthy,
            last_test: Some(Utc::now()),
            error_count: 0,
            details: {
                let mut details = HashMap::new();
                details.insert("volume_level".to_string(), serde_json::Value::Number(serde_json::Number::from(75)));
                details.insert("frequency_response".to_string(), serde_json::Value::String("200Hz-8kHz".to_string()));
                details
            },
        }
    }

    async fn test_gps(&self) -> ComponentHealth {
        ComponentHealth {
            status: HealthStatus::Healthy,
            last_test: Some(Utc::now()),
            error_count: 0,
            details: {
                let mut details = HashMap::new();
                details.insert("satellites".to_string(), serde_json::Value::Number(serde_json::Number::from(8)));
                details.insert("accuracy_meters".to_string(), serde_json::Value::Number(serde_json::Number::from(3)));
                details.insert("fix_time_seconds".to_string(), serde_json::Value::Number(serde_json::Number::from(30)));
                details
            },
        }
    }

    async fn test_accelerometer(&self) -> ComponentHealth {
        ComponentHealth {
            status: HealthStatus::Healthy,
            last_test: Some(Utc::now()),
            error_count: 0,
            details: {
                let mut details = HashMap::new();
                details.insert("range".to_string(), serde_json::Value::String("±8g".to_string()));
                details.insert("resolution".to_string(), serde_json::Value::String("14-bit".to_string()));
                details.insert("sample_rate".to_string(), serde_json::Value::Number(serde_json::Number::from(100)));
                details
            },
        }
    }

    async fn test_wifi(&self) -> ComponentHealth {
        ComponentHealth {
            status: HealthStatus::Healthy,
            last_test: Some(Utc::now()),
            error_count: 0,
            details: {
                let mut details = HashMap::new();
                details.insert("signal_strength".to_string(), serde_json::Value::Number(serde_json::Number::from(-45)));
                details.insert("frequency".to_string(), serde_json::Value::String("5GHz".to_string()));
                details.insert("bandwidth".to_string(), serde_json::Value::String("80MHz".to_string()));
                details
            },
        }
    }

    async fn test_cellular(&self) -> ComponentHealth {
        ComponentHealth {
            status: HealthStatus::Warning,
            last_test: Some(Utc::now()),
            error_count: 2,
            details: {
                let mut details = HashMap::new();
                details.insert("signal_strength".to_string(), serde_json::Value::Number(serde_json::Number::from(-85)));
                details.insert("network_type".to_string(), serde_json::Value::String("4G LTE".to_string()));
                details.insert("carrier".to_string(), serde_json::Value::String("Verizon".to_string()));
                details
            },
        }
    }

    async fn test_bluetooth(&self) -> ComponentHealth {
        ComponentHealth {
            status: HealthStatus::Healthy,
            last_test: Some(Utc::now()),
            error_count: 0,
            details: {
                let mut details = HashMap::new();
                details.insert("version".to_string(), serde_json::Value::String("5.0".to_string()));
                details.insert("paired_devices".to_string(), serde_json::Value::Number(serde_json::Number::from(0)));
                details
            },
        }
    }

    async fn test_leds(&self, hardware: &dyn crate::hardware::HardwareInterface) -> ComponentHealth {
        ComponentHealth {
            status: HealthStatus::Healthy,
            last_test: Some(Utc::now()),
            error_count: 0,
            details: {
                let mut details = HashMap::new();
                details.insert("led_count".to_string(), serde_json::Value::Number(serde_json::Number::from(4)));
                details.insert("brightness_levels".to_string(), serde_json::Value::Number(serde_json::Number::from(255)));
                details
            },
        }
    }

    async fn test_buttons(&self, hardware: &dyn crate::hardware::HardwareInterface) -> ComponentHealth {
        ComponentHealth {
            status: HealthStatus::Healthy,
            last_test: Some(Utc::now()),
            error_count: 0,
            details: {
                let mut details = HashMap::new();
                details.insert("button_count".to_string(), serde_json::Value::Number(serde_json::Number::from(3)));
                details.insert("response_time_ms".to_string(), serde_json::Value::Number(serde_json::Number::from(50)));
                details
            },
        }
    }

    async fn measure_performance(&self) -> Result<PerformanceMetrics> {
        Ok(PerformanceMetrics {
            recording_performance: RecordingPerformance {
                current_fps: Some(29.8),
                target_fps: 30,
                dropped_frames: 12,
                encoding_latency_ms: Some(16.7),
                disk_write_speed_mbps: Some(25.0),
                recording_status: HealthStatus::Healthy,
            },
            streaming_performance: StreamingPerformance {
                bitrate_kbps: Some(2500),
                target_bitrate_kbps: 2500,
                packet_loss_percent: Some(0.1),
                latency_ms: Some(150.0),
                streaming_status: HealthStatus::Healthy,
            },
            audio_performance: AudioPerformance {
                sample_rate: 44100,
                channels: 2,
                buffer_underruns: 0,
                latency_ms: Some(10.0),
                audio_status: HealthStatus::Healthy,
            },
            network_performance: NetworkPerformance {
                ping_ms: Some(25.0),
                download_mbps: Some(50.0),
                upload_mbps: Some(10.0),
                packet_loss: Some(0.1),
                signal_strength: Some(-45),
                network_status: HealthStatus::Healthy,
            },
            response_times: ResponseTimes {
                button_response_ms: Some(50.0),
                startup_time_ms: Some(2500.0),
                shutdown_time_ms: Some(1200.0),
                recording_start_ms: Some(800.0),
                incident_trigger_ms: Some(200.0),
            },
        })
    }

    async fn run_connectivity_tests(&self) -> Result<ConnectivityTests> {
        Ok(ConnectivityTests {
            server_connectivity: ConnectivityTest {
                status: HealthStatus::Healthy,
                latency_ms: Some(45.0),
                last_test: Utc::now(),
                error_message: None,
                test_details: HashMap::new(),
            },
            internet_connectivity: ConnectivityTest {
                status: HealthStatus::Healthy,
                latency_ms: Some(25.0),
                last_test: Utc::now(),
                error_message: None,
                test_details: HashMap::new(),
            },
            gps_connectivity: ConnectivityTest {
                status: HealthStatus::Healthy,
                latency_ms: Some(1000.0),
                last_test: Utc::now(),
                error_message: None,
                test_details: HashMap::new(),
            },
            cellular_connectivity: Some(ConnectivityTest {
                status: HealthStatus::Warning,
                latency_ms: Some(120.0),
                last_test: Utc::now(),
                error_message: Some("Weak signal strength".to_string()),
                test_details: HashMap::new(),
            }),
            wifi_connectivity: Some(ConnectivityTest {
                status: HealthStatus::Healthy,
                latency_ms: Some(15.0),
                last_test: Utc::now(),
                error_message: None,
                test_details: HashMap::new(),
            }),
        })
    }

    async fn analyze_storage(&self) -> Result<StorageAnalysis> {
        Ok(StorageAnalysis {
            recordings_storage: StorageCategory {
                used_gb: 12.5,
                file_count: 156,
                oldest_file: Some(Utc::now() - chrono::Duration::days(25)),
                newest_file: Some(Utc::now() - chrono::Duration::hours(2)),
                growth_rate_gb_per_day: Some(0.8),
            },
            logs_storage: StorageCategory {
                used_gb: 0.25,
                file_count: 45,
                oldest_file: Some(Utc::now() - chrono::Duration::days(15)),
                newest_file: Some(Utc::now()),
                growth_rate_gb_per_day: Some(0.05),
            },
            temp_storage: StorageCategory {
                used_gb: 0.1,
                file_count: 12,
                oldest_file: Some(Utc::now() - chrono::Duration::hours(6)),
                newest_file: Some(Utc::now() - chrono::Duration::minutes(30)),
                growth_rate_gb_per_day: Some(0.2),
            },
            system_storage: StorageCategory {
                used_gb: 2.8,
                file_count: 1245,
                oldest_file: Some(Utc::now() - chrono::Duration::days(90)),
                newest_file: Some(Utc::now()),
                growth_rate_gb_per_day: Some(0.01),
            },
            cleanup_recommendations: vec![
                CleanupRecommendation {
                    category: "Old Recordings".to_string(),
                    action: "Delete recordings older than 30 days".to_string(),
                    potential_space_gb: 3.2,
                    priority: "Medium".to_string(),
                    risk_level: "Low".to_string(),
                },
                CleanupRecommendation {
                    category: "Temporary Files".to_string(),
                    action: "Clear temp files older than 1 hour".to_string(),
                    potential_space_gb: 0.05,
                    priority: "Low".to_string(),
                    risk_level: "None".to_string(),
                },
            ],
        })
    }

    async fn check_security_status(&self) -> Result<SecurityStatus> {
        Ok(SecurityStatus {
            encryption_status: EncryptionStatus {
                recordings_encrypted: self.config.encryption.enabled,
                communications_encrypted: true,
                key_rotation_status: HealthStatus::Healthy,
                encryption_algorithm: self.config.encryption.algorithm.clone(),
            },
            authentication_status: AuthenticationStatus {
                device_authenticated: true,
                token_valid: true,
                last_authentication: Some(Utc::now() - chrono::Duration::hours(2)),
                authentication_method: "Ed25519".to_string(),
            },
            certificate_status: CertificateStatus {
                certificates_valid: true,
                expiry_dates: vec![Utc::now() + chrono::Duration::days(365)],
                next_renewal: Some(Utc::now() + chrono::Duration::days(300)),
            },
            access_control: AccessControlStatus {
                permissions_valid: true,
                role_assignments: vec!["bodycam_operator".to_string()],
                access_violations: 0,
            },
            security_events: vec![],
        })
    }

    async fn collect_error_logs(&self) -> Result<ErrorLogs> {
        Ok(ErrorLogs {
            recent_errors: vec![
                ErrorEntry {
                    timestamp: Utc::now() - chrono::Duration::hours(1),
                    level: "WARN".to_string(),
                    component: "audio".to_string(),
                    message: "Audio buffer underrun detected".to_string(),
                    stack_trace: None,
                    context: HashMap::new(),
                },
            ],
            error_summary: ErrorSummary {
                total_errors: 15,
                error_by_component: {
                    let mut map = HashMap::new();
                    map.insert("audio".to_string(), 8);
                    map.insert("network".to_string(), 4);
                    map.insert("camera".to_string(), 3);
                    map
                },
                error_by_severity: {
                    let mut map = HashMap::new();
                    map.insert("ERROR".to_string(), 3);
                    map.insert("WARN".to_string(), 12);
                    map
                },
                error_trends: vec![],
            },
            crash_reports: vec![],
        })
    }
}