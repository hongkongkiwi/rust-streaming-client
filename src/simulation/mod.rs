use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::{Editor, Helper, Context};
use rustyline::completion::{Completer, Pair};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::{Validator, ValidationContext, ValidationResult};
use std::collections::{HashSet, HashMap};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

use crate::device::BodycamDevice;
use crate::hardware::HardwareEvent;
use crate::config::Config;

pub struct SimulationRepl {
    device: Arc<Mutex<BodycamDevice>>,
    event_tx: mpsc::UnboundedSender<HardwareEvent>,
    event_rx: Option<mpsc::UnboundedReceiver<HardwareEvent>>,
}

struct ReplHelper {
    commands: HashSet<String>,
    hinter: HistoryHinter,
}

impl ReplHelper {
    fn new() -> Self {
        let mut commands = HashSet::new();
        commands.insert("help".to_string());
        commands.insert("status".to_string());
        commands.insert("battery".to_string());
        commands.insert("temperature".to_string());
        commands.insert("storage".to_string());
        commands.insert("press".to_string());
        commands.insert("longpress".to_string());
        commands.insert("motion".to_string());
        commands.insert("lowbattery".to_string());
        commands.insert("charging".to_string());
        commands.insert("tamper".to_string());
        commands.insert("record".to_string());
        commands.insert("stop".to_string());
        commands.insert("incident".to_string());
        commands.insert("exit".to_string());
        commands.insert("quit".to_string());
        
        Self {
            commands,
            hinter: HistoryHinter {},
        }
    }
}

impl Completer for ReplHelper {
    type Candidate = Pair;
    
    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>
    ) -> rustyline::Result<(Vec<Pair>, usize)> {
        let matches: Vec<Pair> = self.commands
            .iter()
            .filter(|cmd| cmd.starts_with(line))
            .map(|cmd| Pair {
                display: cmd.clone(),
                replacement: cmd.clone(),
            })
            .collect();
        
        Ok((matches, 0))
    }
}

impl Helper for ReplHelper {}

impl Hinter for ReplHelper {
    fn hint(&self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>
    ) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl Validator for ReplHelper {
    fn validate(
        &self,
        _ctx: &mut ValidationContext
    ) -> rustyline::Result<ValidationResult> {
        Ok(ValidationResult::Valid)
    }
}

impl SimulationRepl {
    pub fn new(device: Arc<Mutex<BodycamDevice>>) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        
        Self {
            device,
            event_tx,
            event_rx: Some(event_rx),
        }
    }

    pub async fn run(&mut self
    ) -> Result<()> {
        println!("=== Bodycam Simulation REPL ===");
        println!("Enter commands to simulate hardware events");
        println!("Type 'help' for available commands");
        println!();

        let mut rl = Editor::<ReplHelper>::new()?;
        rl.set_helper(Some(ReplHelper::new()));

        // Spawn event listener
        let device_clone = Arc::clone(&self.device);
        let mut event_rx = self.event_rx.take().unwrap();
        
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                Self::handle_hardware_event(&device_clone, event).await;
            }
        });

        loop {
            let readline = rl.readline("bodycam> ");
            
            match readline {
                Ok(line) => {
                    rl.add_history_entry(line.as_str());
                    
                    if let Err(e) = self.handle_command(&line).await {
                        println!("Error: {}", e);
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("CTRL-C");
                    break;
                }
                Err(ReadlineError::Eof) => {
                    println!("CTRL-D");
                    break;
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                    break;
                }
            }
        }

        println!("Exiting simulation REPL...");
        Ok(())
    }

    async fn handle_command(&self, command: &str) -> Result<()> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        
        match parts.get(0).map(|s| *s) {
            Some("help") => {
                self.print_help();
            }
            Some("status") => {
                let device = self.device.lock().await;
                let status = device.get_status().await?;
                println!("{}", serde_json::to_string_pretty(&status)?);
            }
            Some("battery") => {
                if let Some(level) = parts.get(1) {
                    if let Ok(level) = level.parse::<f32>() {
                        let event = HardwareEvent::BatteryLow { level };
                        let _ = self.event_tx.send(event);
                        println!("Battery level set to {}%", level);
                    }
                } else {
                    println!("Usage: battery <level>");
                }
            }
            Some("temperature") => {
                if let Some(temp) = parts.get(1) {
                    if let Ok(temp) = temp.parse::<f32>() {
                        let event = HardwareEvent::TemperatureHigh { temp };
                        let _ = self.event_tx.send(event);
                        println!("Temperature set to {}°C", temp);
                    }
                } else {
                    println!("Usage: temperature <temp>");
                }
            }
            Some("storage") => {
                println!("Storage usage simulated");
                let event = HardwareEvent::StorageFull;
                let _ = self.event_tx.send(event);
            }
            Some("press") => {
                if let Some(button) = parts.get(1) {
                    let button_type = match *button {
                        "record" => crate::hardware::ButtonType::Record,
                        "emergency" => crate::hardware::ButtonType::Emergency,
                        "power" => crate::hardware::ButtonType::Power,
                        "menu" => crate::hardware::ButtonType::Menu,
                        _ => {
                            println!("Unknown button: {}", button);
                            return Ok(());
                        }
                    };
                    
                    let event = HardwareEvent::ButtonPressed {
                        button: button_type,
                        duration: None,
                    };
                    let _ = self.event_tx.send(event);
                    println!("Button pressed: {}", button);
                } else {
                    println!("Usage: press <button> (record|emergency|power|menu)");
                }
            }
            Some("longpress") => {
                if let Some(button) = parts.get(1) {
                    let button_type = match *button {
                        "record" => crate::hardware::ButtonType::Record,
                        "emergency" => crate::hardware::ButtonType::Emergency,
                        "power" => crate::hardware::ButtonType::Power,
                        _ => {
                            println!("Unknown button: {}", button);
                            return Ok(());
                        }
                    };
                    
                    let duration = parts.get(2).and_then(|d| d.parse::<u64>().ok()).unwrap_or(2000);
                    
                    let event = HardwareEvent::ButtonPressed {
                        button: button_type,
                        duration: Some(duration),
                    };
                    let _ = self.event_tx.send(event);
                    println!("Button long-pressed: {} ({}ms)", button, duration);
                } else {
                    println!("Usage: longpress <button> [duration_ms]");
                }
            }
            Some("motion") => {
                let intensity = parts.get(1).and_then(|i| i.parse::<f64>().ok()).unwrap_or(5.0);
                let event = HardwareEvent::MotionDetected { intensity };
                let _ = self.event_tx.send(event);
                println!("Motion detected with intensity: {}", intensity);
            }
            Some("lowbattery") => {
                let event = HardwareEvent::BatteryLow { level: 15.0 };
                let _ = self.event_tx.send(event);
                println!("Low battery event triggered");
            }
            Some("charging") => {
                let event = HardwareEvent::ChargingConnected;
                let _ = self.event_tx.send(event);
                println!("Charging connected event triggered");
            }
            Some("tamper") => {
                let event = HardwareEvent::TamperDetected;
                let _ = self.event_tx.send(event);
                println!("Tamper detected event triggered");
            }
            Some("record") => {
                let device = self.device.lock().await;
                device.start_recording(None, None).await?;
                println!("Recording started");
            }
            Some("stop") => {
                let device = self.device.lock().await;
                device.stop_recording().await?;
                println!("Recording stopped");
            }
            Some("incident") => {
                let incident_type = parts.get(1).unwrap_or(&"manual").to_string();
                let severity = parts.get(2).unwrap_or(&"medium").to_string();
                
                let device = self.device.lock().await;
                let incident_id = device.trigger_incident(&incident_type, &severity).await?;
                println!("Incident triggered: {}", incident_id);
            }
            Some("exit") | Some("quit") => {
                return Err(anyhow::anyhow!("exit"));
            }
            _ => {
                println!("Unknown command. Type 'help' for available commands.");
            }
        }
        
        Ok(())
    }

    async fn handle_hardware_event(
        device: &Arc<Mutex<BodycamDevice>>,
        event: HardwareEvent
    ) {
        let device = device.lock().await;
        
        match event {
            HardwareEvent::ButtonPressed { button, duration } => {
                match button {
                    crate::hardware::ButtonType::Record => {
                        if duration.is_some() {
                            // Long press - stop recording
                            let _ = device.stop_recording().await;
                            println!("Long press - recording stopped");
                        } else {
                            // Short press - toggle recording
                            let _ = device.start_recording(None, None).await;
                            println!("Short press - recording started");
                        }
                    }
                    crate::hardware::ButtonType::Emergency => {
                        let _ = device.trigger_incident("emergency", "high").await;
                        println!("Emergency button pressed");
                    }
                    crate::hardware::ButtonType::Power => {
                        println!("Power button pressed");
                    }
                    _ => {}
                }
            }
            HardwareEvent::BatteryLow { level } => {
                println!("⚠️  Battery low: {}%", level);
                let _ = device.hardware.set_led("battery", crate::hardware::LedState::Blink {
                    on_duration: 200,
                    off_duration: 200,
                    repeat: Some(10),
                }).await;
            }
            HardwareEvent::BatteryCritical { level } => {
                println!("🚨 Battery critical: {}% - shutting down", level);
                let _ = device.hardware.shutdown().await;
            }
            HardwareEvent::StorageFull => {
                println!("💾 Storage full - stopping recording");
                let _ = device.stop_recording().await;
            }
            HardwareEvent::TemperatureHigh { temp } => {
                println!("🌡️  Temperature high: {}°C", temp);
            }
            HardwareEvent::MotionDetected { intensity } => {
                println!("🏃 Motion detected: intensity {}", intensity);
                if intensity > 7.0 {
                    let _ = device.trigger_incident("motion", "medium").await;
                }
            }
            HardwareEvent::TamperDetected => {
                println!("🚨 Tamper detected");
                let _ = device.trigger_incident("tamper", "high").await;
            }
            _ => {}
        }
    }

    fn print_help(&self
    ) {
        println!("Available commands:");
        println!("  help                - Show this help");
        println!("  status              - Show device status");
        println!("  battery <level>      - Simulate battery level (0-100)");
        println!("  temperature <temp>  - Simulate temperature (°C)");
        println!("  storage             - Simulate storage full");
        println!("  press <button>      - Simulate button press (record|emergency|power|menu)");
        println!("  longpress <button>  - Simulate long button press");
        println!("  motion [intensity]  - Simulate motion detection");
        println!("  lowbattery          - Simulate low battery");
        println!("  charging            - Simulate charging connected");
        println!("  tamper              - Simulate tamper detection");
        println!("  record              - Start recording");
        println!("  stop                - Stop recording");
        println!("  incident [type] [sev] - Trigger incident");
        println!("  exit/quit           - Exit simulation");
    }
}