use super::*;
use anyhow::{Result, Context};
use tokio::sync::mpsc;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Instant;

pub struct MacHardware {
    simulation: bool,
    leds: HashMap<String, LedInfo>,
    buttons: HashMap<String, ButtonInfo>,
    battery_level: Arc<Mutex<f32>>,
    storage_used: Arc<Mutex<u64>>,
    temperature: Arc<Mutex<f32>>,
    is_charging: Arc<Mutex<bool>>,
    last_button_press: Arc<Mutex<HashMap<String, Instant>>>,
}

#[derive(Debug)]
struct LedInfo {
    name: String,
    color: String,
    current_state: LedState,
}

#[derive(Debug)]
struct ButtonInfo {
    name: String,
    button_type: ButtonType,
    debounce_ms: u64,
    long_press_ms: u64,
}

impl MacHardware {
    pub fn new(simulation: bool) -> Self {
        let mut leds = HashMap::new();
        let mut buttons = HashMap::new();

        // Default button configurations for macOS testing
        buttons.insert("record".to_string(), ButtonInfo {
            name: "record".to_string(),
            button_type: ButtonType::Record,
            debounce_ms: 50,
            long_press_ms: 1000,
        });

        buttons.insert("emergency".to_string(), ButtonInfo {
            name: "emergency".to_string(),
            button_type: ButtonType::Emergency,
            debounce_ms: 50,
            long_press_ms: 2000,
        });

        buttons.insert("power".to_string(), ButtonInfo {
            name: "power".to_string(),
            button_type: ButtonType::Power,
            debounce_ms: 100,
            long_press_ms: 3000,
        });

        // Default LED configurations
        leds.insert("recording".to_string(), LedInfo {
            name: "recording".to_string(),
            color: "red".to_string(),
            current_state: LedState::Off,
        });

        leds.insert("power".to_string(), LedInfo {
            name: "power".to_string(),
            color: "green".to_string(),
            current_state: LedState::On,
        });

        leds.insert("wifi".to_string(), LedInfo {
            name: "wifi".to_string(),
            color: "blue".to_string(),
            current_state: LedState::Blink {
                on_duration: 500,
                off_duration: 500,
                repeat: None,
            },
        });

        Self {
            simulation,
            leds,
            buttons,
            battery_level: Arc::new(Mutex::new(100.0)),
            storage_used: Arc::new(Mutex::new(0)),
            temperature: Arc::new(Mutex::new(25.0)),
            is_charging: Arc::new(Mutex::new(false)),
            last_button_press: Arc::new(Mutex::new(HashMap::new())),
        }
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
                    } else {
                        *battery = (*battery + 0.2).min(100.0);
                    }
                }

                // Simulate storage usage
                {
                    let mut storage = storage_used.lock().await;
                    *storage += 5_000_000; // 5MB per interval
                    if *storage > 60_000_000_000 { // 60GB
                        let _ = tx.send(HardwareEvent::StorageFull);
                    }
                }

                // Simulate temperature changes
                {
                    let mut temp = temperature.lock().await;
                    *temp += (rand::random::<f32>() - 0.5) * 0.5;
                    *temp = temp.clamp(20.0, 65.0);
                    if *temp > 55.0 {
                        let _ = tx.send(HardwareEvent::TemperatureHigh { temp: *temp });
                    }
                }

                // Random charging state changes
                if rand::random::<f32>() < 0.02 {
                    let mut charging = is_charging.lock().await;
                    *charging = !*charging;
                    if *charging {
                        let _ = tx.send(HardwareEvent::ChargingConnected);
                    } else {
                        let _ = tx.send(HardwareEvent::ChargingDisconnected);
                    }
                }

                // Random motion detection
                if rand::random::<f32>() < 0.15 {
                    let _ = tx.send(HardwareEvent::MotionDetected { 
                        intensity: rand::random::<f64>() * 8.0 
                    });
                }
            }
        });

        // Simulate button presses based on keyboard input
        for (_, button_info) in &self.buttons {
            let tx_clone = tx.clone();
            let button_type = button_info.button_type.clone();
            let debounce_ms = button_info.debounce_ms;
            let long_press_ms = button_info.long_press_ms;

            tokio::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
                
                loop {
                    interval.tick().await;
                    
                    // Simulate random button presses for demo
                    if rand::random::<f32>() < 0.1 {
                        let is_long_press = rand::random::<f32>() < 0.3;
                        let duration = if is_long_press { 
                            Some(long_press_ms + rand::random::<u64>() % 2000)
                        } else { None };
                        
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
}

#[async_trait::async_trait]
impl HardwareInterface for MacHardware {
    async fn init(&mut self, config: &super::HardwareConfig) -> Result<()> {
        if !self.simulation {
            tracing::info!("Initializing macOS hardware interface");
            // Real hardware initialization would go here
        } else {
            tracing::info!("Running in macOS simulation mode");
        }
        
        Ok(())
    }

    async fn start_monitoring(&self) -> Result<mpsc::UnboundedReceiver<HardwareEvent>> {
        let (tx, rx) = mpsc::unbounded_channel();
        
        if self.simulation {
            self.simulate_hardware_events(tx.clone()).await?;
        } else {
            // Real hardware monitoring would go here
            // For now, simulate in macOS since we don't have GPIO
            self.simulate_hardware_events(tx.clone()).await?;
        }
        
        Ok(rx)
    }

    async fn set_led(&self, led_name: &str, state: LedState) -> Result<()> {
        if let Some(led_info) = self.leds.get(led_name) {
            tracing::info!("Setting LED {} ({}): {:?}", led_name, led_info.color, state);
            
            // In real hardware, this would control actual LEDs
            // On macOS, we just log the action
        } else {
            tracing::warn!("Unknown LED: {}", led_name);
        }
        Ok(())
    }

    async fn get_battery_level(&self) -> Result<f32> {
        if self.simulation {
            let level = *self.battery_level.lock().await;
            return Ok(level);
        }

        // On macOS, we could use system_profiler or pmset
        // For now, return simulated value
        Ok(85.0)
    }

    async fn get_storage_info(&self) -> Result<StorageInfo> {
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

        // Real storage reading would use statfs
        Ok(StorageInfo {
            total: 64_000_000_000,
            used: 15_000_000_000,
            available: 49_000_000_000,
            recording_space: 49_000_000_000,
        })
    }

    async fn get_temperature(&self) -> Result<f32> {
        if self.simulation {
            let temp = *self.temperature.lock().await;
            return Ok(temp);
        }

        // On macOS, we could use SMC or system sensors
        Ok(30.5)
    }

    async fn is_charging(&self) -> Result<bool> {
        if self.simulation {
            let charging = *self.is_charging.lock().await;
            return Ok(charging);
        }

        // On macOS, check if power adapter is connected
        Ok(false)
    }

    async fn vibrate(&self, duration_ms: u64) -> Result<()> {
        tracing::info!("Simulating vibration for {}ms", duration_ms);
        
        // On macOS, we could use the haptic feedback API
        // For now, just log the action
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("Simulating shutdown");
        
        // On macOS, this would trigger a proper shutdown
        // For now, just log the action
        Ok(())
    }
}