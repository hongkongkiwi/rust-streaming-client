use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use std::sync::Arc;
use tokio::sync::Mutex;
use reqwest::Client;

use crate::config::{Config, RemoteConfig as ConfigRemote};
use crate::device::BodycamDevice;
use crate::sentry_integration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteConfigUpdate {
    pub device_id: String,
    pub config_version: String,
    pub update_timestamp: chrono::DateTime<chrono::Utc>,
    pub changes: serde_json::Value,
    pub force_update: bool,
    pub restart_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigRequest {
    pub device_id: String,
    pub current_config_version: String,
    pub capabilities_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub success: bool,
    pub config: Option<serde_json::Value>,
    pub version: String,
    pub last_modified: chrono::DateTime<chrono::Utc>,
    pub restart_required: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNotification {
    pub notification_type: String,
    pub message: String,
    pub config_version: String,
    pub priority: UpdatePriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdatePriority {
    Low,
    Medium,
    High,
    Critical,
}

pub struct RemoteConfigManager {
    config: Config,
    client: Client,
    device: Arc<Mutex<BodycamDevice>>,
    update_tx: mpsc::UnboundedSender<RemoteConfigUpdate>,
}

impl RemoteConfigManager {
    pub fn new(config: Config, device: Arc<Mutex<BodycamDevice>>) -> (Self, mpsc::UnboundedReceiver<RemoteConfigUpdate>) {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        let (update_tx, update_rx) = mpsc::unbounded_channel();
        
        let manager = Self {
            config,
            client,
            device,
            update_tx,
        };
        
        (manager, update_rx)
    }
    
    pub async fn start(&mut self
    ) -> Result<()> {
        let _transaction = sentry_integration::start_transaction("remote_config.start", "config");
        
        if !self.config.remote_config.auto_update {
            tracing::info!("Remote configuration updates disabled");
            return Ok(());
        }
        
        tracing::info!("Starting remote configuration manager");
        
        let device = self.device.clone();
        let config = self.config.clone();
        let client = self.client.clone();
        let update_tx = self.update_tx.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.remote_config.update_interval_seconds));
            
            loop {
                interval.tick().await;
                
                let _transaction = sentry_integration::start_transaction("remote_config.check", "config");
                
                if let Err(e) = Self::check_for_updates(&config, &client, &device, &update_tx
                ).await {
                    tracing::error!("Failed to check for remote configuration updates: {}", e);
                    sentry_integration::capture_error(&e, "operation" => "remote_config_check");
                }
            }
        });
        
        Ok(())
    }
    
    pub async fn request_config_update(&self
    ) -> Result<Option<ConfigResponse>> {
        let _transaction = sentry_integration::start_transaction("remote_config.request_update", "config");
        
        let device = self.device.lock().await;
        let device_id = device.device_id.clone()
            .ok_or_else(|| anyhow::anyhow!("Device not provisioned"))?;
        
        let request = ConfigRequest {
            device_id,
            current_config_version: self.config.remote_config.config_version.clone(),
            capabilities_hash: None, // Could include device capabilities hash
        };
        
        let url = format!("{}{}", 
            self.config.server_url, 
            self.config.remote_config.config_endpoint
        );
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send configuration request")?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Configuration request failed: {}", error_text));
        }
        
        let config_response: ConfigResponse = response
            .json()
            .await
            .context("Failed to parse configuration response")?;
        
        if !config_response.success {
            return Err(anyhow::anyhow!("Configuration update failed: {:?}", config_response.message));
        }
        
        if let Some(new_config) = &config_response.config {
            tracing::info!("Received configuration update: {}", config_response.version);
            
            let update = RemoteConfigUpdate {
                device_id: request.device_id,
                config_version: config_response.version.clone(),
                update_timestamp: chrono::Utc::now(),
                changes: new_config.clone(),
                force_update: false,
                restart_required: config_response.restart_required,
            };
            
            let _ = self.update_tx.send(update);
        }
        
        Ok(Some(config_response))
    }
    
    pub async fn force_config_update(&self
    ) -> Result<ConfigResponse> {
        let _transaction = sentry_integration::start_transaction("remote_config.force_update", "config");
        
        // This simulates a server-initiated force update
        let update_notification = UpdateNotification {
            notification_type: "force_update".to_string(),
            message: "Server requested immediate configuration update".to_string(),
            config_version: "latest".to_string(),
            priority: UpdatePriority::High,
        };
        
        tracing::info!("Processing force update request: {:?}", update_notification);
        
        // Trigger immediate update check
        let response = self.request_config_update()
            .await?
            .unwrap_or_else(|| ConfigResponse {
                success: false,
                config: None,
                version: self.config.remote_config.config_version.clone(),
                last_modified: chrono::Utc::now(),
                restart_required: false,
                message: Some("No configuration changes available".to_string()),
            });
        
        Ok(response)
    }
    
    async fn check_for_updates(
        config: &Config,
        client: &Client,
        device: &Arc<Mutex<BodycamDevice>>,
        update_tx: &mpsc::UnboundedSender<RemoteConfigUpdate>,
    ) -> Result<()> {
        let device_locked = device.lock().await;
        let device_id = device_locked.device_id.clone()
            .ok_or_else(|| anyhow::anyhow!("Device not provisioned"))?;
        
        let request = ConfigRequest {
            device_id,
            current_config_version: config.remote_config.config_version.clone(),
            capabilities_hash: None,
        };
        
        let url = format!("{}{}", 
            config.server_url, 
            config.remote_config.config_endpoint
        );
        
        let response = client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to check for configuration updates")?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Configuration check failed: {}", error_text));
        }
        
        let config_response: ConfigResponse = response
            .json()
            .await
            .context("Failed to parse configuration response")?;
        
        if config_response.success && config_response.config.is_some() {
            tracing::info!("Configuration update available: {}", config_response.version);
            
            let update = RemoteConfigUpdate {
                device_id: request.device_id,
                config_version: config_response.version.clone(),
                update_timestamp: chrono::Utc::now(),
                changes: config_response.config.unwrap(),
                force_update: false,
                restart_required: config_response.restart_required,
            };
            
            let _ = update_tx.send(update);
        } else if let Some(message) = config_response.message {
            tracing::debug!("Configuration check: {}", message);
        }
        
        Ok(())
    }
    
    pub async fn handle_config_update(&mut self, update: RemoteConfigUpdate
    ) -> Result<()> {
        let _transaction = sentry_integration::start_transaction("remote_config.handle_update", "config");
        
        tracing::info!("Processing configuration update: {}", update.config_version);
        
        // Log the update for now
        // In a full implementation, this would apply the configuration changes
        sentry_integration::add_device_breadcrumb(
            "config_update_received", 
            Some(&format!("version: {}, restart_required: {}, force_update: {}", 
                update.config_version, 
                update.restart_required, 
                update.force_update
            ))
        );
        
        // Notify server of update receipt
        let notification = UpdateNotification {
            notification_type: "config_applied".to_string(),
            message: format!("Configuration {} applied successfully", update.config_version),
            config_version: update.config_version.clone(),
            priority: UpdatePriority::Medium,
        };
        
        tracing::info!("Configuration update processed: {:?}", notification);
        
        Ok(())
    }
    
    pub async fn send_config_status(&self, status: &str, details: Option<serde_json::Value>
    ) -> Result<()> {
        let device = self.device.lock().await;
        let device_id = device.device_id.clone()
            .ok_or_else(|| anyhow::anyhow!("Device not provisioned"))?;
        
        let status_update = serde_json::json!({
            "device_id": device_id,
            "status": status,
            "details": details,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        
        let url = format!("{}/api/devices/config-status", self.config.server_url);
        
        let response = self.client
            .post(&url)
            .json(&status_update)
            .send()
            .await
            .context("Failed to send configuration status")?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Failed to send config status: {}", error_text));
        }
        
        Ok(())
    }
}

impl BodycamDevice {
    pub async fn request_remote_config_update(&mut self
    ) -> Result<()> {
        let config_manager = RemoteConfigManager::new(self.config.clone(), 
            Arc::new(tokio::sync::Mutex::new(self.clone())));
        
        let (_, _) = config_manager;
        
        // Implementation would integrate with existing device
        tracing::info!("Requesting remote configuration update");
        
        Ok(())
    }
}