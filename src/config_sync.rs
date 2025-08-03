use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, watch};
use tokio::time::{interval, Duration};
use tracing::{info, warn, error};

use crate::config::Config;
use crate::convex_api::ConvexApiClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigUpdateEvent {
    pub device_id: String,
    pub config_version: String,
    pub settings_delta: serde_json::Value,
    pub timestamp: u64,
    pub source: String, // "server", "manual", "automated"
}

#[derive(Debug, Clone)]
pub struct RealTimeConfig {
    pub config: Arc<RwLock<Config>>,
    pub update_sender: mpsc::UnboundedSender<ConfigUpdateEvent>,
    pub update_receiver: watch::Receiver<Config>,
    pub subscription_active: Arc<RwLock<bool>>,
}

pub struct ConfigSyncManager {
    convex_client: Arc<RwLock<ConvexApiClient>>,
    config: Arc<RwLock<Config>>,
    real_time_config: Arc<RealTimeConfig>,
    device_id: String,
    tenant_id: String,
}

impl ConfigSyncManager {
    pub fn new(
        convex_client: Arc<RwLock<ConvexApiClient>>,
        config: Arc<RwLock<Config>>,
        device_id: String,
        tenant_id: String,
    ) -> Self {
        let (update_sender, mut update_receiver) = mpsc::unbounded_channel();
        let (config_watcher, config_receiver) = watch::channel(Config::default());

        let real_time_config = Arc::new(RealTimeConfig {
            config: config.clone(),
            update_sender,
            update_receiver: config_receiver,
            subscription_active: Arc::new(RwLock::new(false)),
        });

        // Spawn a task to handle config updates
        tokio::spawn(async move {
            while let Some(event) = update_receiver.recv().await {
                info!("Received config update event: {:?}", event);
                // Handle the update event
            }
        });

        Self {
            convex_client,
            config,
            real_time_config,
            device_id,
            tenant_id,
        }
    }

    pub async fn start_real_time_sync(&self) -> Result<()> {
        info!("Starting real-time configuration sync for device {}", self.device_id);
        
        // Get initial configuration from server
        self.sync_config_from_server().await?;
        
        // Start subscription to device configuration changes
        self.start_configuration_subscription().await?;
        
        // Start periodic sync as fallback
        self.start_periodic_sync().await?;
        
        Ok(())
    }

    pub async fn sync_config_from_server(&self) -> Result<()> {
        let client = self.convex_client.read().await;
        let server_settings = client.get_device_settings(&self.device_id).await
            .context("Failed to fetch device settings from server")?;

        let mut config = self.config.write().await;
        
        // Update configuration with server settings
        config.recording.video_quality = server_settings.video_quality;
        config.recording.bitrate = server_settings.video_bitrate;
        config.audio.enabled = server_settings.audio_enabled;
        
        // Update button actions
        for (button_type, action) in server_settings.button_actions {
            match button_type.as_str() {
                "single_press" => {
                    config.security.single_press_action = Some(action);
                },
                "double_press" => {
                    config.security.double_press_action = Some(action);
                },
                "long_press" => {
                    config.security.long_press_action = Some(action);
                },
                "triple_press" => {
                    config.security.triple_press_action = Some(action);
                },
                _ => warn!("Unknown button action type: {}", button_type),
            }
        }

        // Update SOS settings
        config.security.sos_enabled = server_settings.sos_settings.enabled;
        config.security.emergency_contacts = server_settings.sos_settings.emergency_contacts;
        config.security.auto_call_timeout = server_settings.sos_settings.auto_call_timeout;

        // Update WiFi networks
        config.network.wifi_networks = server_settings.wifi_networks;

        // Update power management settings
        config.power_management.low_power_mode = server_settings.power_management.low_power_mode;
        config.power_management.auto_shutdown_timeout = server_settings.power_management.auto_shutdown_timeout;
        config.power_management.brightness_level = server_settings.power_management.brightness_level;

        // Save updated configuration
        config.remote_config.last_update = Some(chrono::Utc::now());
        config.remote_config.config_version = chrono::Utc::now().timestamp().to_string();

        info!("Configuration successfully synced from server");

        // Update the watch channel
        let _ = self.real_time_config.update_receiver.send(config.clone());

        Ok(())
    }

    async fn start_configuration_subscription(&self) -> Result<()> {
        let device_id = self.device_id.clone();
        let tenant_id = self.tenant_id.clone();
        let convex_client = self.convex_client.clone();
        let config = self.config.clone();
        let real_time_config = self.real_time_config.clone();

        tokio::spawn(async move {
            loop {
                match Self::subscribe_to_config_changes(
                    &convex_client,
                    &device_id,
                    &tenant_id,
                    &config,
                    &real_time_config,
                ).await {
                    Ok(_) => {
                        info!("Configuration subscription completed successfully");
                        break;
                    }
                    Err(e) => {
                        error!("Configuration subscription error: {:?}. Retrying in 30 seconds...", e);
                        tokio::time::sleep(Duration::from_secs(30)).await;
                    }
                }
            }
        });

        Ok(())
    }

    async fn subscribe_to_config_changes(
        convex_client: &Arc<RwLock<ConvexApiClient>>,
        device_id: &str,
        tenant_id: &str,
        config: &Arc<RwLock<Config>>,
        real_time_config: &Arc<RealTimeConfig>,
    ) -> Result<()> {
        let client = convex_client.read().await;
        
        // Create a subscription query for device configuration changes
        let subscription_query = format!(
            r#"
            subscription deviceConfig($deviceId: String!, $tenantId: String!) {{
                deviceSettings(deviceId: $deviceId, tenantId: $tenantId) {{
                    id
                    deviceId
                    tenantId
                    videoQuality
                    videoBitrate
                    audioEnabled
                    buttonActions
                    sosSettings
                    wifiNetworks
                    powerManagement
                    updatedAt
                    version
                }}
            }}
            "#,
            deviceId = device_id,
            tenantId = tenant_id
        );

        let args = serde_json::json!({
            "deviceId": device_id,
            "tenantId": tenant_id
        });

        // Start the subscription
        // Note: This is a placeholder for Convex subscription implementation
        // In the actual implementation, you would use Convex's subscription API
        
        *real_time_config.subscription_active.write().await = true;

        // Simulate subscription handling
        loop {
            tokio::select! {
                // Handle subscription updates here
                _ = tokio::time::sleep(Duration::from_secs(60)) => {
                    // Check for updates periodically
                    let _ = Self::sync_config_from_server_internal(
                        convex_client,
                        device_id,
                        config,
                        real_time_config,
                    ).await;
                }
            }
        }
    }

    async fn sync_config_from_server_internal(
        convex_client: &Arc<RwLock<ConvexApiClient>>,
        device_id: &str,
        config: &Arc<RwLock<Config>>,
        real_time_config: &Arc<RealTimeConfig>,
    ) -> Result<()> {
        let client = convex_client.read().await;
        let server_settings = client.get_device_settings(device_id).await
            .context("Failed to fetch device settings from server")?;

        let mut config = config.write().await;
        let mut changed = false;

        // Check if configuration has changed
        if config.recording.video_quality != server_settings.video_quality {
            config.recording.video_quality = server_settings.video_quality.clone();
            changed = true;
        }

        if config.recording.bitrate != server_settings.video_bitrate {
            config.recording.bitrate = server_settings.video_bitrate;
            changed = true;
        }

        if config.audio.enabled != server_settings.audio_enabled {
            config.audio.enabled = server_settings.audio_enabled;
            changed = true;
        }

        if changed {
            config.remote_config.last_update = Some(chrono::Utc::now());
            config.remote_config.config_version = chrono::Utc::now().timestamp().to_string();

            info!("Configuration updated from server");

            // Update the watch channel
            let _ = real_time_config.update_receiver.send(config.clone());

            // Send update event
            if let Some(sender) = &real_time_config.update_sender {
                let event = ConfigUpdateEvent {
                    device_id: device_id.to_string(),
                    config_version: config.remote_config.config_version.clone(),
                    settings_delta: serde_json::json!({
                        "video_quality": server_settings.video_quality,
                        "video_bitrate": server_settings.video_bitrate,
                        "audio_enabled": server_settings.audio_enabled,
                    }),
                    timestamp: chrono::Utc::now().timestamp() as u64,
                    source: "server".to_string(),
                };

                let _ = sender.send(event);
            }
        }

        Ok(())
    }

    async fn start_periodic_sync(&self) -> Result<()> {
        let device_id = self.device_id.clone();
        let convex_client = self.convex_client.clone();
        let config = self.config.clone();
        let real_time_config = self.real_time_config.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(300)); // 5 minutes
            
            loop {
                interval.tick().await;
                
                if *real_time_config.subscription_active.read().await {
                    continue; // Skip if real-time sync is active
                }

                if let Err(e) = Self::sync_config_from_server_internal(
                    &convex_client,
                    &device_id,
                    &config,
                    &real_time_config,
                ).await {
                    warn!("Periodic config sync failed: {:?}", e);
                }
            }
        });

        Ok(())
    }

    pub async fn trigger_config_update(&self) -> Result<()> {
        self.sync_config_from_server().await
    }

    pub async fn get_config_watcher(&self) -> watch::Receiver<Config> {
        self.real_time_config.update_receiver.clone()
    }

    pub async fn is_subscription_active(&self) -> bool {
        *self.real_time_config.subscription_active.read().await
    }

    pub async fn stop_sync(&self) {
        *self.real_time_config.subscription_active.write().await = false;
    }
}

// Extension to Config struct for button actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonActionConfig {
    pub single_press: Option<String>,
    pub double_press: Option<String>,
    pub long_press: Option<String>,
    pub triple_press: Option<String>,
}

impl Config {
    pub fn get_button_actions(&self) -> ButtonActionConfig {
        ButtonActionConfig {
            single_press: self.security.single_press_action.clone(),
            double_press: self.security.double_press_action.clone(),
            long_press: self.security.long_press_action.clone(),
            triple_press: self.security.triple_press_action.clone(),
        }
    }

    pub fn set_button_action(&mut self, action_type: &str, action: String) {
        match action_type {
            "single_press" => self.security.single_press_action = Some(action),
            "double_press" => self.security.double_press_action = Some(action),
            "long_press" => self.security.long_press_action = Some(action),
            "triple_press" => self.security.triple_press_action = Some(action),
            _ => warn!("Unknown button action type: {}", action_type),
        }
    }
}

// Extension to SecurityConfig
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