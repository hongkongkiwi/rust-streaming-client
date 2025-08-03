use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn, debug};

/// Comprehensive device capabilities detected from hardware
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    /// Device identification
    pub device_info: DeviceInfo,
    /// Network connectivity capabilities
    pub network: NetworkCapabilities,
    /// Audio input/output capabilities
    pub audio: AudioCapabilities,
    /// Camera capabilities
    pub camera: CameraCapabilities,
    /// Sensor capabilities
    pub sensors: SensorCapabilities,
    /// Storage capabilities
    pub storage: StorageCapabilities,
    /// Power management capabilities
    pub power: PowerCapabilities,
    /// User interface capabilities
    pub ui: UICapabilities,
    /// Connectivity capabilities
    pub connectivity: ConnectivityCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub platform: String,
    pub architecture: String,
    pub os_version: String,
    pub kernel_version: Option<String>,
    pub hostname: String,
    pub cpu_info: CpuInfo,
    pub memory_info: MemoryInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub cores: u32,
    pub threads: u32,
    pub model: String,
    pub frequency_mhz: u32,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total_mb: u64,
    pub available_mb: u64,
    pub swap_total_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkCapabilities {
    pub wifi: Option<WiFiCapability>,
    pub ethernet: Option<EthernetCapability>,
    pub cellular: Option<CellularCapability>,
    pub bluetooth: Option<BluetoothCapability>,
    pub interfaces: Vec<NetworkInterface>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WiFiCapability {
    pub enabled: bool,
    pub interface: String,
    pub standards: Vec<String>, // 802.11a/b/g/n/ac/ax
    pub bands: Vec<String>,     // 2.4GHz, 5GHz, 6GHz
    pub max_speed_mbps: u32,
    pub current_connection: Option<WiFiConnection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WiFiConnection {
    pub ssid: String,
    pub signal_strength: i32,
    pub frequency_mhz: u32,
    pub security: String,
    pub ip_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthernetCapability {
    pub enabled: bool,
    pub interface: String,
    pub speeds: Vec<u32>, // Supported speeds in Mbps
    pub current_speed: Option<u32>,
    pub link_detected: bool,
    pub ip_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellularCapability {
    pub enabled: bool,
    pub interface: String,
    pub technologies: Vec<String>, // 2G, 3G, 4G, 5G
    pub carrier: Option<String>,
    pub signal_strength: Option<i32>,
    pub ip_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothCapability {
    pub enabled: bool,
    pub version: String,
    pub profiles: Vec<String>,
    pub low_energy_support: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub interface_type: String,
    pub mac_address: String,
    pub ip_addresses: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioCapabilities {
    pub input: Option<AudioInputCapability>,
    pub output: Option<AudioOutputCapability>,
    pub devices: Vec<AudioDevice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioInputCapability {
    pub enabled: bool,
    pub default_device: String,
    pub sample_rates: Vec<u32>,
    pub bit_depths: Vec<u8>,
    pub channels: Vec<u8>,
    pub formats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioOutputCapability {
    pub enabled: bool,
    pub default_device: String,
    pub sample_rates: Vec<u32>,
    pub bit_depths: Vec<u8>,
    pub channels: Vec<u8>,
    pub formats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub name: String,
    pub device_type: String, // input, output
    pub driver: String,
    pub sample_rate: u32,
    pub channels: u8,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraCapabilities {
    pub devices: Vec<CameraDevice>,
    pub default_device: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraDevice {
    pub name: String,
    pub device_path: String,
    pub driver: String,
    pub resolutions: Vec<Resolution>,
    pub frame_rates: Vec<u32>,
    pub formats: Vec<String>,
    pub controls: Vec<CameraControl>,
    pub is_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
    pub aspect_ratio: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraControl {
    pub name: String,
    pub control_type: String,
    pub min_value: i32,
    pub max_value: i32,
    pub default_value: i32,
    pub step: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorCapabilities {
    pub gps: Option<GpsSensor>,
    pub accelerometer: Option<AccelerometerSensor>,
    pub gyroscope: Option<GyroscopeSensor>,
    pub magnetometer: Option<MagnetometerSensor>,
    pub barometer: Option<BarometerSensor>,
    pub temperature: Option<TemperatureSensor>,
    pub humidity: Option<HumiditySensor>,
    pub light: Option<LightSensor>,
    pub proximity: Option<ProximitySensor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsSensor {
    pub enabled: bool,
    pub device_path: String,
    pub protocols: Vec<String>, // NMEA, UBX, etc.
    pub accuracy_meters: f64,
    pub update_rate_hz: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccelerometerSensor {
    pub enabled: bool,
    pub device_path: String,
    pub range_g: f64,
    pub resolution_bits: u8,
    pub sample_rate_hz: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GyroscopeSensor {
    pub enabled: bool,
    pub device_path: String,
    pub range_dps: f64,
    pub resolution_bits: u8,
    pub sample_rate_hz: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagnetometerSensor {
    pub enabled: bool,
    pub device_path: String,
    pub range_gauss: f64,
    pub resolution_bits: u8,
    pub sample_rate_hz: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarometerSensor {
    pub enabled: bool,
    pub device_path: String,
    pub range_hpa: (f64, f64),
    pub resolution_pa: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureSensor {
    pub enabled: bool,
    pub device_path: String,
    pub range_celsius: (f64, f64),
    pub accuracy_celsius: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumiditySensor {
    pub enabled: bool,
    pub device_path: String,
    pub range_percent: (f64, f64),
    pub accuracy_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightSensor {
    pub enabled: bool,
    pub device_path: String,
    pub range_lux: (f64, f64),
    pub resolution_lux: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProximitySensor {
    pub enabled: bool,
    pub device_path: String,
    pub range_cm: f64,
    pub resolution_cm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCapabilities {
    pub internal: StorageDevice,
    pub external: Vec<StorageDevice>,
    pub total_space_gb: u64,
    pub available_space_gb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageDevice {
    pub name: String,
    pub device_path: String,
    pub filesystem: String,
    pub size_gb: u64,
    pub available_gb: u64,
    pub mount_point: String,
    pub is_removable: bool,
    pub write_speed_mbps: Option<u32>,
    pub read_speed_mbps: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerCapabilities {
    pub battery: Option<BatteryInfo>,
    pub charging: Option<ChargingInfo>,
    pub power_management: PowerManagementInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryInfo {
    pub present: bool,
    pub capacity_mah: u32,
    pub voltage_v: f32,
    pub technology: String,
    pub health_percent: u8,
    pub cycles: u32,
    pub temperature_celsius: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChargingInfo {
    pub supports_fast_charging: bool,
    pub max_charging_power_w: u32,
    pub charging_ports: Vec<String>,
    pub wireless_charging: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerManagementInfo {
    pub cpu_scaling_available: bool,
    pub sleep_modes: Vec<String>,
    pub wake_sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UICapabilities {
    pub display: Option<DisplayInfo>,
    pub buttons: Vec<ButtonInfo>,
    pub leds: Vec<LedInfo>,
    pub audio_indicators: Vec<AudioIndicator>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayInfo {
    pub enabled: bool,
    pub width: u32,
    pub height: u32,
    pub color_depth: u8,
    pub refresh_rate_hz: u32,
    pub touchscreen: bool,
    pub backlight_control: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonInfo {
    pub name: String,
    pub button_type: String,
    pub location: String,
    pub supports_long_press: bool,
    pub debounce_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedInfo {
    pub name: String,
    pub color: String,
    pub location: String,
    pub supports_dimming: bool,
    pub supports_blinking: bool,
    pub max_brightness: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioIndicator {
    pub name: String,
    pub indicator_type: String, // buzzer, speaker, beeper
    pub frequency_range_hz: (u32, u32),
    pub max_volume_db: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectivityCapabilities {
    pub usb: Option<UsbCapability>,
    pub serial: Vec<SerialPort>,
    pub i2c: Vec<I2cBus>,
    pub spi: Vec<SpiBus>,
    pub gpio: Option<GpioCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbCapability {
    pub host_mode: bool,
    pub device_mode: bool,
    pub otg_support: bool,
    pub usb_version: String,
    pub ports: Vec<UsbPort>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbPort {
    pub port_number: u8,
    pub usb_version: String,
    pub max_power_ma: u32,
    pub connected_device: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialPort {
    pub device_path: String,
    pub baud_rates: Vec<u32>,
    pub data_bits: Vec<u8>,
    pub stop_bits: Vec<f32>,
    pub parity_support: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I2cBus {
    pub bus_number: u8,
    pub device_path: String,
    pub clock_speed_khz: u32,
    pub connected_devices: Vec<I2cDevice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I2cDevice {
    pub address: u8,
    pub name: Option<String>,
    pub driver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiBus {
    pub bus_number: u8,
    pub device_path: String,
    pub max_speed_hz: u32,
    pub modes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpioCapability {
    pub total_pins: u32,
    pub available_pins: Vec<u32>,
    pub pull_up_support: bool,
    pub pull_down_support: bool,
    pub interrupt_support: bool,
}

/// Capability detector for different platforms
pub struct CapabilityDetector {
    simulation: bool,
}

impl CapabilityDetector {
    pub fn new(simulation: bool) -> Self {
        Self { simulation }
    }

    /// Detect all device capabilities
    pub async fn detect_capabilities(&self) -> Result<DeviceCapabilities> {
        info!("Starting comprehensive capability detection...");

        let device_info = self.detect_device_info().await?;
        let network = self.detect_network_capabilities().await?;
        let audio = self.detect_audio_capabilities().await?;
        let camera = self.detect_camera_capabilities().await?;
        let sensors = self.detect_sensor_capabilities().await?;
        let storage = self.detect_storage_capabilities().await?;
        let power = self.detect_power_capabilities().await?;
        let ui = self.detect_ui_capabilities().await?;
        let connectivity = self.detect_connectivity_capabilities().await?;

        let capabilities = DeviceCapabilities {
            device_info,
            network,
            audio,
            camera,
            sensors,
            storage,
            power,
            ui,
            connectivity,
        };

        info!("Capability detection completed successfully");
        debug!("Detected capabilities: {:#?}", capabilities);

        Ok(capabilities)
    }

    async fn detect_device_info(&self) -> Result<DeviceInfo> {
        let platform = std::env::consts::OS.to_string();
        let architecture = std::env::consts::ARCH.to_string();

        #[cfg(target_os = "linux")]
        let os_version = self.get_linux_version().await.unwrap_or_else(|_| "Unknown".to_string());
        #[cfg(target_os = "macos")]
        let os_version = self.get_macos_version().await.unwrap_or_else(|_| "Unknown".to_string());
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let os_version = "Unknown".to_string();

        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let cpu_info = self.detect_cpu_info().await.unwrap_or_else(|_| CpuInfo {
            cores: 1,
            threads: 1,
            model: "Unknown".to_string(),
            frequency_mhz: 1000,
            features: vec![],
        });

        let memory_info = self.detect_memory_info().await.unwrap_or_else(|_| MemoryInfo {
            total_mb: 1024,
            available_mb: 512,
            swap_total_mb: 0,
        });

        Ok(DeviceInfo {
            platform,
            architecture,
            os_version,
            kernel_version: self.get_kernel_version().await.ok(),
            hostname,
            cpu_info,
            memory_info,
        })
    }

    #[cfg(target_os = "linux")]
    async fn get_linux_version(&self) -> Result<String> {
        if self.simulation {
            return Ok("Linux Simulation 1.0".to_string());
        }
        
        match tokio::fs::read_to_string("/etc/os-release").await {
            Ok(content) => {
                for line in content.lines() {
                    if line.starts_with("PRETTY_NAME=") {
                        return Ok(line.trim_start_matches("PRETTY_NAME=\"")
                                    .trim_end_matches('"')
                                    .to_string());
                    }
                }
                Ok("Linux Unknown".to_string())
            }
            Err(_) => Ok("Linux Unknown".to_string()),
        }
    }

    #[cfg(target_os = "macos")]
    async fn get_macos_version(&self) -> Result<String> {
        if self.simulation {
            return Ok("macOS Simulation 1.0".to_string());
        }

        use std::process::Command;
        match Command::new("sw_vers").arg("-productVersion").output() {
            Ok(output) => {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                Ok(format!("macOS {}", version))
            }
            Err(_) => Ok("macOS Unknown".to_string()),
        }
    }

    async fn get_kernel_version(&self) -> Result<String> {
        if self.simulation {
            return Ok("Simulation Kernel 1.0.0".to_string());
        }

        #[cfg(target_os = "linux")]
        {
            match tokio::fs::read_to_string("/proc/version").await {
                Ok(content) => Ok(content.lines().next().unwrap_or("Unknown").to_string()),
                Err(_) => Ok("Unknown".to_string()),
            }
        }

        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            match Command::new("uname").arg("-r").output() {
                Ok(output) => Ok(String::from_utf8_lossy(&output.stdout).trim().to_string()),
                Err(_) => Ok("Unknown".to_string()),
            }
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        Ok("Unknown".to_string())
    }

    async fn detect_cpu_info(&self) -> Result<CpuInfo> {
        if self.simulation {
            return Ok(CpuInfo {
                cores: 4,
                threads: 8,
                model: "Simulation CPU".to_string(),
                frequency_mhz: 2400,
                features: vec!["sse".to_string(), "avx".to_string()],
            });
        }

        #[cfg(target_os = "linux")]
        {
            self.detect_linux_cpu_info().await
        }

        #[cfg(target_os = "macos")]
        {
            self.detect_macos_cpu_info().await
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Ok(CpuInfo {
                cores: 1,
                threads: 1,
                model: "Unknown".to_string(),
                frequency_mhz: 1000,
                features: vec![],
            })
        }
    }

    #[cfg(target_os = "linux")]
    async fn detect_linux_cpu_info(&self) -> Result<CpuInfo> {
        let content = tokio::fs::read_to_string("/proc/cpuinfo").await
            .context("Failed to read /proc/cpuinfo")?;

        let mut cores = 0;
        let mut model = "Unknown".to_string();
        let mut frequency_mhz = 1000;
        let mut features = Vec::new();

        for line in content.lines() {
            if line.starts_with("processor") {
                cores += 1;
            } else if line.starts_with("model name") {
                if let Some(value) = line.split(':').nth(1) {
                    model = value.trim().to_string();
                }
            } else if line.starts_with("cpu MHz") {
                if let Some(value) = line.split(':').nth(1) {
                    if let Ok(freq) = value.trim().parse::<f32>() {
                        frequency_mhz = freq as u32;
                    }
                }
            } else if line.starts_with("flags") {
                if let Some(value) = line.split(':').nth(1) {
                    features = value.trim().split_whitespace().map(String::from).collect();
                }
            }
        }

        Ok(CpuInfo {
            cores,
            threads: cores, // Assuming 1 thread per core for simplicity
            model,
            frequency_mhz,
            features,
        })
    }

    #[cfg(target_os = "macos")]
    async fn detect_macos_cpu_info(&self) -> Result<CpuInfo> {
        use std::process::Command;

        let cores = Command::new("sysctl")
            .args(["-n", "hw.ncpu"])
            .output()
            .ok()
            .and_then(|output| String::from_utf8_lossy(&output.stdout).trim().parse().ok())
            .unwrap_or(1);

        let model = Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .ok()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let frequency_mhz = Command::new("sysctl")
            .args(["-n", "hw.cpufrequency_max"])
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .parse::<u64>()
                    .ok()
                    .map(|hz| (hz / 1_000_000) as u32)
            })
            .unwrap_or(2400);

        Ok(CpuInfo {
            cores,
            threads: cores,
            model,
            frequency_mhz,
            features: vec![], // macOS doesn't expose CPU features easily
        })
    }

    async fn detect_memory_info(&self) -> Result<MemoryInfo> {
        if self.simulation {
            return Ok(MemoryInfo {
                total_mb: 8192,
                available_mb: 4096,
                swap_total_mb: 2048,
            });
        }

        #[cfg(target_os = "linux")]
        {
            self.detect_linux_memory_info().await
        }

        #[cfg(target_os = "macos")]
        {
            self.detect_macos_memory_info().await
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Ok(MemoryInfo {
                total_mb: 1024,
                available_mb: 512,
                swap_total_mb: 0,
            })
        }
    }

    #[cfg(target_os = "linux")]
    async fn detect_linux_memory_info(&self) -> Result<MemoryInfo> {
        let content = tokio::fs::read_to_string("/proc/meminfo").await
            .context("Failed to read /proc/meminfo")?;

        let mut total_mb = 0;
        let mut available_mb = 0;
        let mut swap_total_mb = 0;

        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(value) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = value.parse::<u64>() {
                        total_mb = kb / 1024;
                    }
                }
            } else if line.starts_with("MemAvailable:") {
                if let Some(value) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = value.parse::<u64>() {
                        available_mb = kb / 1024;
                    }
                }
            } else if line.starts_with("SwapTotal:") {
                if let Some(value) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = value.parse::<u64>() {
                        swap_total_mb = kb / 1024;
                    }
                }
            }
        }

        Ok(MemoryInfo {
            total_mb,
            available_mb,
            swap_total_mb,
        })
    }

    #[cfg(target_os = "macos")]
    async fn detect_macos_memory_info(&self) -> Result<MemoryInfo> {
        use std::process::Command;

        let total_mb = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .parse::<u64>()
                    .ok()
                    .map(|bytes| bytes / 1024 / 1024)
            })
            .unwrap_or(1024);

        // macOS doesn't have easily accessible available memory info via sysctl
        let available_mb = total_mb / 2; // Rough estimate

        Ok(MemoryInfo {
            total_mb,
            available_mb,
            swap_total_mb: 0, // macOS swap is dynamic
        })
    }
}