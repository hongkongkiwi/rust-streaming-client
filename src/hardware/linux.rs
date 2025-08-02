use super::*;
use anyhow::{Result, Context};
use tokio::fs;
use tokio::sync::mpsc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct LinuxHardware {
    simulation: bool,
    gpio_pins: HashMap<u32, GpioPinInfo>,
    leds: HashMap<String, LedInfo>,
    buttons: HashMap<String, ButtonInfo>,
    sensors: HashMap<String, SensorInfo>,
    battery_level: Arc<Mutex<f32>>,
    storage_used: Arc<Mutex<u64>>,
    temperature: Arc<Mutex<f32>>,
    is_charging: Arc<Mutex<bool>>,
}

#[derive(Debug)]
struct GpioPinInfo {
    direction: GpioDirection,
    active_low: bool,
    value_path: String,
    function: PinFunction,
}

#[derive(Debug)]
struct LedInfo {
    gpio_pin: u32,
    color: String,
    current_state: LedState,
}

#[derive(Debug)]
struct ButtonInfo {
    gpio_pin: u32,
    button_type: ButtonType,
    debounce_ms: u64,
    long_press_ms: u64,
}

#[derive(Debug)]
struct SensorInfo {
    device_path: String,
    sensor_type: SensorType,
}

impl LinuxHardware {
    pub fn new(simulation: bool) -> Self {
        Self {
            simulation,
            gpio_pins: HashMap::new(),
            leds: HashMap::new(),
            buttons: HashMap::new(),
            sensors: HashMap::new(),
            battery_level: Arc::new(Mutex::new(100.0)),
            storage_used: Arc::new(Mutex::new(0)),
            temperature: Arc::new(Mutex::new(25.0)),
            is_charging: Arc::new(Mutex::new(false)),
        }
    }

    async fn init_gpio_pins(&mut self, config: &HardwareConfig) -> Result<()> {
        if !config.gpio.enabled {
            return Ok(());
        }

        for pin_config in &config.gpio.pins {
            if !self.simulation {
                self.export_gpio_pin(pin_config.number).await?;
                self.set_gpio_direction(pin_config.number, &pin_config.direction).await?;
                
                if pin_config.active_low {
                    self.set_active_low(pin_config.number, true).await?;
                }
            }

            let pin_info = GpioPinInfo {
                direction: pin_config.direction.clone(),
                active_low: pin_config.active_low,
                value_path: format!("/sys/class/gpio/gpio{}/value", pin_config.number),
                function: pin_config.function.clone(),
            };

            self.gpio_pins.insert(pin_config.number, pin_info);

            match &pin_config.function {
                PinFunction::Led(led_type) => {
                    let led_info = LedInfo {
                        gpio_pin: pin_config.number,
                        color: "red".to_string(), // Default color
                        current_state: LedState::Off,
                    };
                    self.leds.insert(format!("{:?}", led_type), led_info);
                }
                PinFunction::Button(button_type) => {
                    let button_info = ButtonInfo {
                        gpio_pin: pin_config.number,
                        button_type: button_type.clone(),
                        debounce_ms: 50,
                        long_press_ms: 1000,
                    };
                    self.buttons.insert(format!("{:?}", button_type), button_info);
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn export_gpio_pin(&self, pin: u32) -> Result<()> {
        let export_path = "/sys/class/gpio/export";
        fs::write(export_path, pin.to_string()).await
            .context(format!("Failed to export GPIO pin {}", pin))?;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        Ok(())
    }

    async fn set_gpio_direction(&self, pin: u32, direction: &GpioDirection) -> Result<()> {
        let direction_path = format!("/sys/class/gpio/gpio{}/direction", pin);
        let dir_str = match direction {
            GpioDirection::Input => "in",
            GpioDirection::Output => "out",
        };
        fs::write(direction_path, dir_str).await
            .context(format!("Failed to set direction for GPIO pin {}", pin))?;
        Ok(())
    }

    async fn set_active_low(&self, pin: u32, active_low: bool) -> Result<()> {
        let active_low_path = format!("/sys/class/gpio/gpio{}/active_low", pin);
        fs::write(active_low_path, if active_low { "1" } else { "0" })
            .await
            .context(format!("Failed to set active_low for GPIO pin {}", pin))?;
        Ok(())
    }

    async fn set_gpio_value(&self, pin: u32, value: bool) -> Result<()> {
        if let Some(pin_info) = self.gpio_pins.get(&pin) {
            if matches!(pin_info.direction, GpioDirection::Output) {
                let value_path = &pin_info.value_path;
                let value_str = if value { "1" } else { "0" };
                
                if !self.simulation {
                    fs::write(value_path, value_str).await
                        .context(format!("Failed to set GPIO pin {}", pin))?;
                }
                
                tracing::debug!("GPIO pin {} set to {}", pin, value);
            }
        }
        Ok(())
    }

    async fn read_gpio_value(&self, pin: u32) -> Result<bool> {
        if let Some(pin_info) = self.gpio_pins.get(&pin) {
            if matches!(pin_info.direction, GpioDirection::Input) {
                if self.simulation {
                    // Simulate button press for testing
                    return Ok(rand::random());
                }

                let value_path = &pin_info.value_path;
                let value_str = fs::read_to_string(value_path).await
                    .context(format!("Failed to read GPIO pin {}", pin))?;
                
                let value = value_str.trim() == "1";
                return Ok(value ^ pin_info.active_low);
            }
        }
        Ok(false)
    }

    async fn simulate_hardware_events(&self, tx: mpsc::UnboundedSender<HardwareEvent>) -> Result<()> {
        let battery_level = Arc::clone(&self.battery_level);
        let storage_used = Arc::clone(&self.storage_used);
        let temperature = Arc::clone(&self.temperature);
        let is_charging = Arc::clone(&self.is_charging);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                // Simulate battery drain
                {
                    let mut battery = battery_level.lock().await;
                    if !*is_charging.lock().await {
                        *battery -= 0.1;
                        if *battery < 20.0 {
                            let _ = tx.send(HardwareEvent::BatteryLow { level: *battery });
                        }
                        if *battery < 5.0 {
                            let _ = tx.send(HardwareEvent::BatteryCritical { level: *battery });
                        }
                    }
                }

                // Simulate storage usage
                {
                    let mut storage = storage_used.lock().await;
                    *storage += 10_000_000; // 10MB per interval
                    if *storage > 50_000_000_000 { // 50GB
                        let _ = tx.send(HardwareEvent::StorageFull);
                    }
                }

                // Simulate temperature changes
                {
                    let mut temp = temperature.lock().await;
                    *temp += (rand::random::<f32>() - 0.5) * 2.0;
                    if *temp > 60.0 {
                        let _ = tx.send(HardwareEvent::TemperatureHigh { temp: *temp });
                    }
                }

                // Random motion detection
                if rand::random::<f32>() < 0.1 {
                    let _ = tx.send(HardwareEvent::MotionDetected { 
                        intensity: rand::random::<f64>() * 10.0 
                    });
                }
            }
        });

        // Simulate button presses
        for (_, button_info) in &self.buttons {
            let tx_clone = tx.clone();
            let button_type = button_info.button_type.clone();
            
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
                
                loop {
                    interval.tick().await;
                    
                    if rand::random::<f32>() < 0.05 { // 5% chance every 30 seconds
                        let is_long_press = rand::random::<f32>() < 0.2; // 20% chance of long press
                        let duration = if is_long_press { Some(2000) } else { None };
                        
                        let _ = tx_clone.send(HardwareEvent::ButtonPressed {
                            button: button_type.clone(),
                            duration,
                        });
                    }
                }
            });
        }

        Ok(())
    }

    async fn monitor_buttons(&self, tx: mpsc::UnboundedSender<HardwareEvent>) -> Result<()> {
        if self.simulation {
            return self.simulate_hardware_events(tx).await;
        }

        for (_, button_info) in &self.buttons {
            let tx_clone = tx.clone();
            let pin = button_info.gpio_pin;
            let button_type = button_info.button_type.clone();
            let debounce_ms = button_info.debounce_ms;
            let long_press_ms = button_info.long_press_ms;

            tokio::spawn(async move {
                let mut last_state = false;
                let mut press_start = None;

                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(debounce_ms)).await;
                    
                    match self.read_gpio_value(pin).await {
                        Ok(current_state) => {
                            if current_state != last_state {
                                if current_state {
                                    // Button pressed
                                    press_start = Some(std::time::Instant::now());
                                } else {
                                    // Button released
                                    if let Some(start) = press_start {
                                        let duration = start.elapsed().as_millis() as u64;
                                        let is_long_press = duration >= long_press_ms;
                                        
                                        let _ = tx_clone.send(HardwareEvent::ButtonPressed {
                                            button: button_type.clone(),
                                            duration: if is_long_press { Some(duration) } else { None },
                                        });
                                    }
                                    press_start = None;
                                }
                                last_state = current_state;
                            }
                        }
                        Err(e) => {
                            let _ = tx_clone.send(HardwareEvent::SensorError {
                                sensor: format!("button_{}", pin),
                                error: e.to_string(),
                            });
                        }
                    }
                }
            });
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl HardwareInterface for LinuxHardware {
    async fn init(&mut self, config: &super::HardwareConfig
    ) -> Result<()> {
        self.init_gpio_pins(config).await?;
        
        if !self.simulation {
            tracing::info!("Initializing Linux hardware interface");
        } else {
            tracing::info!("Running in simulation mode");
        }
        
        Ok(())
    }

    async fn start_monitoring(
        &self
    ) -> Result<mpsc::UnboundedReceiver<HardwareEvent>> {
        let (tx, rx) = mpsc::unbounded_channel();
        
        self.monitor_buttons(tx.clone()).await?;
        
        Ok(rx)
    }

    async fn set_led(&self, led_name: &str, state: LedState) -> Result<()> {
        if let Some(led_info) = self.leds.get(led_name) {
            let value = match state {
                LedState::On => true,
                LedState::Off => false,
                _ => {
                    // For blink patterns, we'll just simulate
                    true
                }
            };
            
            self.set_gpio_value(led_info.gpio_pin, value).await?;
            tracing::debug!("LED {} set to {}", led_name, value);
        }
        Ok(())
    }

    async fn get_battery_level(&self
    ) -> Result<f32> {
        if self.simulation {
            let level = *self.battery_level.lock().await;
            return Ok(level);
        }

        // Real battery reading would go here
        Ok(75.0)
    }

    async fn get_storage_info(&self
    ) -> Result<StorageInfo> {
        if self.simulation {
            let used = *self.storage_used.lock().await;
            let total = 64_000_000_000; // 64GB
            let available = total.saturating_sub(used);
            
            return Ok(StorageInfo {
                total,
                used,
                available,
                recording_space: available,
            });
        }

        // Real storage reading would use statvfs
        Ok(StorageInfo {
            total: 64_000_000_000,
            used: 10_000_000_000,
            available: 54_000_000_000,
            recording_space: 54_000_000_000,
        })
    }

    async fn get_temperature(&self
    ) -> Result<f32> {
        if self.simulation {
            let temp = *self.temperature.lock().await;
            return Ok(temp);
        }

        // Real temperature reading would go here
        Ok(28.5)
    }

    async fn is_charging(&self
    ) -> Result<bool> {
        if self.simulation {
            let charging = *self.is_charging.lock().await;
            return Ok(charging);
        }

        // Real charging detection would go here
        Ok(false)
    }

    async fn vibrate(&self, duration_ms: u64) -> Result<()> {
        tracing::info!("Vibrating for {}ms", duration_ms);
        
        if !self.simulation {
            // Real vibration would trigger GPIO or I2C
        }
        
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down device");
        
        if !self.simulation {
            // Real shutdown would use system commands
            // std::process::Command::new("sudo").arg("halt").spawn()?;
        }
        
        Ok(())
    }
}