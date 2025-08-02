use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareConfig {
    pub gpio: GpioConfig,
    pub camera: CameraConfig,
    pub audio: AudioConfig,
    pub sensors: SensorConfig,
    pub leds: LedConfig,
    pub buttons: ButtonConfig,
    pub display: DisplayConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpioConfig {
    pub enabled: bool,
    pub pins: Vec<GpioPin>,
    pub export_path: String,
    pub value_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpioPin {
    pub number: u32,
    pub direction: GpioDirection,
    pub active_low: bool,
    pub description: String,
    pub function: PinFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpioDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PinFunction {
    Button(ButtonType),
    Led(LedType),
    Sensor(SensorType),
    Buzzer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ButtonType {
    Record,
    Emergency,
    Power,
    Menu,
    ZoomIn,
    ZoomOut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LedType {
    Recording,
    Power,
    Charging,
    WiFi,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SensorType {
    Motion,
    Tamper,
    MicrophoneTrigger,
    Light,
    Sound,
    Movement,
    Speech,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    pub enabled: bool,
    pub device_path: String,
    pub resolution: String,
    pub fps: u32,
    pub bitrate: u32,
    pub rotation: u32,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub enabled: bool,
    pub device_path: String,
    pub sample_rate: u32,
    pub channels: u8,
    pub bitrate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorConfig {
    pub accelerometer: Option<AccelerometerConfig>,
    pub gps: Option<GpsConfig>,
    pub battery: Option<BatteryConfig>,
    pub temperature: Option<TemperatureConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccelerometerConfig {
    pub enabled: bool,
    pub device_path: String,
    pub sensitivity: f64,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsConfig {
    pub enabled: bool,
    pub device_path: String,
    pub update_frequency: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryConfig {
    pub enabled: bool,
    pub capacity_path: String,
    pub voltage_path: String,
    pub current_path: String,
    pub temperature_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureConfig {
    pub enabled: bool,
    pub device_path: String,
    pub critical_temp: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedConfig {
    pub enabled: bool,
    pub leds: Vec<Led>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Led {
    pub name: String,
    pub gpio_pin: u32,
    pub color: String,
    pub blink_patterns: Vec<BlinkPattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlinkPattern {
    pub name: String,
    pub on_duration: u64,
    pub off_duration: u64,
    pub repeat_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonConfig {
    pub enabled: bool,
    pub buttons: Vec<Button>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Button {
    pub name: String,
    pub gpio_pin: u32,
    pub button_type: ButtonType,
    pub debounce_ms: u64,
    pub long_press_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub enabled: bool,
    pub device_path: String,
    pub width: u32,
    pub height: u32,
    pub color_depth: u8,
}

#[async_trait::async_trait]
pub trait HardwareInterface: Send + Sync {
    async fn init(&mut self, config: &HardwareConfig) -> Result<()>;
    async fn start_monitoring(&self) -> Result<mpsc::UnboundedReceiver<HardwareEvent>>;
    async fn set_led(&self, led: &str, state: LedState) -> Result<()>;
    async fn get_battery_level(&self) -> Result<f32>;
    async fn get_storage_info(&self) -> Result<StorageInfo>;
    async fn get_temperature(&self) -> Result<f32>;
    async fn is_charging(&self) -> Result<bool>;
    async fn vibrate(&self, duration_ms: u64) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;
}

#[derive(Debug, Clone)]
pub enum HardwareEvent {
    ButtonPressed {
        button: ButtonType,
        duration: Option<u64>,
    },
    MotionDetected {
        intensity: f64,
    },
    LightDetected {
        level: f64,
        threshold: f64,
    },
    SoundDetected {
        level: f64,
        frequency: Option<f64>,
    },
    SpeechDetected {
        confidence: f64,
        phrase: Option<String>,
        duration: u64,
    },
    MovementDetected {
        acceleration: (f64, f64, f64),
        threshold: f64,
    },
    BatteryLow {
        level: f32,
    },
    BatteryCritical {
        level: f32,
    },
    ChargingConnected,
    ChargingDisconnected,
    TemperatureHigh {
        temp: f32,
    },
    StorageFull,
    TamperDetected,
    SensorError {
        sensor: String,
        error: String,
    },
}

#[derive(Debug, Clone)]
pub enum LedState {
    On,
    Off,
    Blink {
        on_duration: u64,
        off_duration: u64,
        repeat: Option<usize>,
    },
    Pulse {
        duration: u64,
    },
}

#[derive(Debug, Clone)]
pub struct StorageInfo {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub recording_space: u64,
}

pub fn create_hardware_interface(simulation: bool) -> Box<dyn HardwareInterface> {
    if simulation {
        #[cfg(target_os = "linux")]
        return Box::new(linux::LinuxHardware::new(true));
        #[cfg(target_os = "macos")]
        return Box::new(macos::MacHardware::new(true));
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        return Box::new(macos::MacHardware::new(true));
    } else {
        #[cfg(target_os = "linux")]
        return Box::new(linux::LinuxHardware::new(false));
        #[cfg(target_os = "macos")]
        return Box::new(macos::MacHardware::new(false));
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        return Box::new(macos::MacHardware::new(false));
    }
}