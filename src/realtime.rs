use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::device::{BodycamDevice, DeviceStatus};
use crate::sentry_integration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeConfig {
    pub checkin_interval_seconds: u64,
    pub enable_real_time_updates: bool,
    pub enable_server_polling: bool,
    pub update_on_demand: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusUpdate {
    pub device_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub status: DeviceStatus,
    pub capabilities: Option<crate::capabilities::DeviceCapabilities>,
    pub checkin_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCommand {
    pub command: String,
    pub parameters: serde_json::Value,
    pub request_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse {
    pub request_id: String,
    pub device_id: String,
    pub status: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct RealtimeManager {
    config: Config,
    device: Arc<Mutex<BodycamDevice>>,
    checkin_interval: u64,
    update_tx: mpsc::UnboundedSender<StatusUpdate>,
    command_rx: mpsc::UnboundedReceiver<ServerCommand>,
}

impl RealtimeManager {
    pub fn new(config: Config, device: Arc<Mutex<BodycamDevice>>) -> (Self, mpsc::UnboundedReceiver<StatusUpdate>, mpsc::UnboundedSender<ServerCommand>) {
        let checkin_interval = config.monitoring.checkin_interval_seconds;
        
        let (update_tx, update_rx) = mpsc::unbounded_channel();
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        
        let manager = Self {
            config,
            device,
            checkin_interval,
            update_tx,
            command_rx,
        };
        
        (manager, update_rx, command_tx)
    }
    
    pub async fn start(&mut self) -> Result<()> {
        let _transaction = sentry_integration::start_transaction("realtime.start", "realtime");
        
        if !self.config.monitoring.enable_real_time_updates {
            tracing::info!("Real-time updates disabled, using polling mode");
            return self.start_polling_mode().await;
        }
        
        tracing::info!("Starting real-time monitoring with {} second checkin interval", self.checkin_interval);
        
        // Start the status reporting loop
        let device = self.device.clone();
        let update_tx = self.update_tx.clone();
        let checkin_interval = self.checkin_interval;
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(checkin_interval));
            
            loop {
                interval.tick().await;
                
                if let Ok(status) = device.lock().await.get_status().await {
                    let capabilities = device.lock().await.get_capabilities().await.ok();
                    
                    let update = StatusUpdate {
                        device_id: status.device_id.clone(),
                        timestamp: chrono::Utc::now(),
                        status,
                        capabilities,
                        checkin_interval,
                    };
                    
                    let _ = update_tx.send(update);
                }
            }
        });
        
        // Start command handling loop
        let device = self.device.clone();
        let mut command_rx = self.command_rx;
        
        tokio::spawn(async move {
            while let Some(command) = command_rx.recv().await {
                let _transaction = sentry_integration::start_transaction("realtime.handle_command", "command");
                
                let response = match Self::handle_server_command(&device, command.clone()).await {
                    Ok(result) => CommandResponse {
                        request_id: command.request_id,
                        device_id: device.lock().await.device_id.clone().unwrap_or_default(),
                        status: "success".to_string(),
                        result: Some(result),
                        error: None,
                        timestamp: chrono::Utc::now(),
                    },
                    Err(e) => CommandResponse {
                        request_id: command.request_id,
                        device_id: device.lock().await.device_id.clone().unwrap_or_default(),
                        status: "error".to_string(),
                        result: None,
                        error: Some(e.to_string()),
                        timestamp: chrono::Utc::now(),
                    },
                };
                
                // Send response back to server
                let _ = Self::send_command_response(&response).await;
            }
        });
        
        Ok(())
    }
    
    async fn start_polling_mode(&mut self) -> Result<()> {
        let device = self.device.clone();
        let update_tx = self.update_tx.clone();
        let checkin_interval = self.checkin_interval;
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(checkin_interval));
            
            loop {
                interval.tick().await;
                
                if let Ok(status) = device.lock().await.get_status().await {
                    let capabilities = device.lock().await.get_capabilities().await.ok();
                    
                    let update = StatusUpdate {
                        device_id: status.device_id.clone(),
                        timestamp: chrono::Utc::now(),
                        status,
                        capabilities,
                        checkin_interval,
                    };
                    
                    let _ = update_tx.send(update);
                }
            }
        });
        
        Ok(())
    }
    
    pub async fn update_checkin_interval(&mut self, new_interval: u64) -> Result<()> {
        self.checkin_interval = new_interval;
        
        // Send immediate status update with new interval
        let device = self.device.lock().await;
        let status = device.get_status().await?;
        let capabilities = device.get_capabilities().await.ok();
        
        let update = StatusUpdate {
            device_id: status.device_id.clone(),
            timestamp: chrono::Utc::now(),
            status,
            capabilities,
            checkin_interval: new_interval,
        };
        
        let _ = self.update_tx.send(update);
        
        Ok(())
    }
    
    async fn handle_server_command(device: &Arc<Mutex<BodycamDevice>>, command: ServerCommand) -> Result<serde_json::Value> {
        match command.command.as_str() {
            "get_status" => {
                let status = device.lock().await.get_status().await?;
                Ok(serde_json::to_value(status)?)
            },
            "get_capabilities" => {
                let capabilities = device.lock().await.get_capabilities().await?;
                Ok(serde_json::to_value(capabilities)?)
            },
            "start_recording" => {
                let duration = command.parameters.get("duration").and_then(|v| v.as_u64());
                let incident_id = command.parameters.get("incident_id").and_then(|v| v.as_str()).map(|s| s.to_string());
                
                device.lock().await.start_recording(duration, incident_id).await?;
                Ok(serde_json::json!({"status": "recording_started"}))
            },
            "stop_recording" => {
                device.lock().await.stop_recording().await?;
                Ok(serde_json::json!({"status": "recording_stopped"}))
            },
            "trigger_incident" => {
                let incident_type = command.parameters.get("type").and_then(|v| v.as_str()).unwrap_or("manual");
                let severity = command.parameters.get("severity").and_then(|v| v.as_str()).unwrap_or("medium");
                
                let incident_id = device.lock().await.trigger_incident(incident_type, severity).await?;
                Ok(serde_json::json!({"incident_id": incident_id}))
            },
            "diagnose" => {
                let report = device.lock().await.diagnose().await?;
                Ok(serde_json::to_value(report)?)
            },
            "set_checkin_interval" => {
                let interval = command.parameters.get("interval_seconds").and_then(|v| v.as_u64()).unwrap_or(30);
                // This would need to be handled by the RealtimeManager
                Ok(serde_json::json!({"new_interval": interval}))
            },
            _ => Err(anyhow::anyhow!("Unknown command: {}", command.command)),
        }
    }
    
    async fn send_command_response(response: &CommandResponse) -> Result<()> {
        // In a real implementation, this would send the response back to the server
        // For now, we'll log it and use the existing status reporting mechanism
        tracing::info!(
            "Command response: request_id={}, status={}", 
            response.request_id, 
            response.status
        );
        
        if let Some(error) = &response.error {
            tracing::error!("Command error: {}", error);
        }
        
        Ok(())
    }
}

impl BodycamDevice {
    async fn get_capabilities(&self) -> Result<crate::capabilities::DeviceCapabilities> {
        let detector = crate::capabilities::CapabilityDetector::new(self.config.simulation.enabled);
        detector.detect_capabilities().await
    }
}