use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, watch};
use tokio::time::{interval, Duration};
use tracing::{info, warn, error};

use crate::config::Config;
use crate::convex_api::ConvexApiClient;

#[derive(Debug, Clone)]
pub struct ConvexSubscriptionManager {
    convex_client: Arc<RwLock<ConvexApiClient>>,
    subscriptions: Arc<RwLock<HashMap<String, SubscriptionHandle>>,
    update_sender: mpsc::UnboundedSender<SubscriptionUpdate>,
    update_receiver: mpsc::UnboundedReceiver<SubscriptionUpdate>,
}

#[derive(Debug, Clone)]
pub struct SubscriptionHandle {
    pub id: String,
    pub query: String,
    pub variables: Value,
    pub active: bool,
    pub last_update: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct SubscriptionUpdate {
    pub subscription_id: String,
    pub data: Value,
    pub timestamp: u64,
    pub source: String,
}

#[derive(Debug, Clone)]
pub enum SubscriptionType {
    DeviceConfig,
    DeviceSettings,
    IncidentNotifications,
    UploadQueueStatus,
    SystemAlerts,
}

impl ConvexSubscriptionManager {
    pub fn new(convex_client: Arc<RwLock<ConvexApiClient>>) -> Self {
        let (update_sender, update_receiver) = mpsc::unbounded_channel();
        
        Self {
            convex_client,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            update_sender,
            update_receiver,
        }
    }

    pub async fn start_device_config_subscription(
        &self,
        device_id: String,
        tenant_id: String,
    ) -> Result<String> {
        let subscription_id = format!("device_config_{}", device_id);
        
        let query = r#"
            subscription deviceConfig($deviceId: String!, $tenantId: String!) {
                deviceSettings(deviceId: $deviceId, tenantId: $tenantId) {
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
                    storageSettings
                    streamingSettings
                    updatedAt
                    version
                }
            }
        "#;

        let variables = json!({
            "deviceId": device_id,
            "tenantId": tenant_id
        });

        self.create_subscription(
            subscription_id.clone(),
            query.to_string(),
            variables,
            SubscriptionType::DeviceConfig,
        ).await
    }

    pub async fn start_incident_notifications_subscription(
        &self,
        device_id: String,
        tenant_id: String,
    ) -> Result<String> {
        let subscription_id = format!("incidents_{}", device_id);
        
        let query = r#"
            subscription incidentNotifications($deviceId: String!, $tenantId: String!) {
                incidents(deviceId: $deviceId, tenantId: $tenantId) {
                    id
                    deviceId
                    incidentType
                    status
                    priority
                    gpsLatitude
                    gpsLongitude
                    createdAt
                    updatedAt
                    metadata
                }
            }
        "#;

        let variables = json!({
            "deviceId": device_id,
            "tenantId": tenant_id
        });

        self.create_subscription(
            subscription_id.clone(),
            query.to_string(),
            variables,
            SubscriptionType::IncidentNotifications,
        ).await
    }

    pub async fn start_upload_queue_subscription(
        &self,
        device_id: String,
        tenant_id: String,
    ) -> Result<String> {
        let subscription_id = format!("upload_queue_{}", device_id);
        
        let query = r#"
            subscription uploadQueueStatus($deviceId: String!, $tenantId: String!) {
                uploadQueue(deviceId: $deviceId, tenantId: $tenantId) {
                    id
                    deviceId
                    fileName
                    status
                    priority
                    progress
                    error
                    createdAt
                    updatedAt
                    retryCount
                }
            }
        "#;

        let variables = json!({
            "deviceId": device_id,
            "tenantId": tenant_id
        });

        self.create_subscription(
            subscription_id.clone(),
            query.to_string(),
            variables,
            SubscriptionType::UploadQueueStatus,
        ).await
    }

    pub async fn start_system_alerts_subscription(
        &self,
        device_id: String,
        tenant_id: String,
    ) -> Result<String> {
        let subscription_id = format!("alerts_{}", device_id);
        
        let query = r#"
            subscription systemAlerts($deviceId: String!, $tenantId: String!) {
                systemAlerts(deviceId: $deviceId, tenantId: $tenantId) {
                    id
                    alertType
                    severity
                    message
                    details
                    createdAt
                    acknowledged
                }
            }
        "#;

        let variables = json!({
            "deviceId": device_id,
            "tenantId": tenant_id
        });

        self.create_subscription(
            subscription_id.clone(),
            query.to_string(),
            variables,
            SubscriptionType::SystemAlerts,
        ).await
    }

    async fn create_subscription(
        &self,
        subscription_id: String,
        query: String,
        variables: Value,
        subscription_type: SubscriptionType,
    ) -> Result<String> {
        let handle = SubscriptionHandle {
            id: subscription_id.clone(),
            query: query.clone(),
            variables: variables.clone(),
            active: true,
            last_update: chrono::Utc::now(),
        };

        {
            let mut subscriptions = self.subscriptions.write().await;
            subscriptions.insert(subscription_id.clone(), handle);
        }

        // Start the subscription handling
        self.start_subscription_worker(subscription_id.clone(), query, variables, subscription_type)
            .await?;

        info!("Created subscription: {}", subscription_id);
        Ok(subscription_id)
    }

    async fn start_subscription_worker(
        &self,
        subscription_id: String,
        query: String,
        variables: Value,
        subscription_type: SubscriptionType,
    ) -> Result<()> {
        let convex_client = self.convex_client.clone();
        let subscriptions = self.subscriptions.clone();
        let update_sender = self.update_sender.clone();

        tokio::spawn(async move {
            info!("Starting subscription worker for: {}", subscription_id);
            
            loop {
                match Self::handle_subscription_updates(
                    &convex_client,
                    &subscription_id,
                    &query,
                    &variables,
                    &subscription_type,
                    &update_sender,
                ).await {
                    Ok(_) => {
                        info!("Subscription worker completed for: {}", subscription_id);
                        break;
                    }
                    Err(e) => {
                        error!("Subscription worker error for {}: {:?}. Retrying in 10 seconds...", subscription_id, e);
                        tokio::time::sleep(Duration::from_secs(10)).await;
                    }
                }
            }
        });

        Ok(())
    }

    async fn handle_subscription_updates(
        convex_client: &Arc<RwLock<ConvexApiClient>>,
        subscription_id: &str,
        query: &str,
        variables: &Value,
        subscription_type: &SubscriptionType,
        update_sender: &mpsc::UnboundedSender<SubscriptionUpdate>,
    ) -> Result<()> {
        let client = convex_client.read().await;
        
        // Note: This is a placeholder for actual Convex subscription implementation
        // In a real implementation, you would use the Convex client's subscription API
        
        info!("Handling subscription updates for: {}", subscription_id);
        
        // Simulate subscription updates (remove in real implementation)
        let mut interval = interval(Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            
            // Simulate receiving an update
            let update_data = match subscription_type {
                SubscriptionType::DeviceConfig => {
                    json!({
                        "type": "device_config_update",
                        "timestamp": chrono::Utc::now().timestamp(),
                        "changes": {
                            "videoQuality": "high",
                            "audioEnabled": true
                        }
                    })
                }
                SubscriptionType::IncidentNotifications => {
                    json!({
                        "type": "new_incident",
                        "incidentId": "inc_12345",
                        "incidentType": "sos",
                        "priority": "high"
                    })
                }
                SubscriptionType::UploadQueueStatus => {
                    json!({
                        "type": "upload_progress",
                        "fileId": "file_12345",
                        "progress": 75,
                        "status": "uploading"
                    })
                }
                SubscriptionType::SystemAlerts => {
                    json!({
                        "type": "system_alert",
                        "alertType": "battery_low",
                        "severity": "warning",
                        "message": "Battery level below 20%"
                    })
                }
            };

            let update = SubscriptionUpdate {
                subscription_id: subscription_id.to_string(),
                data: update_data,
                timestamp: chrono::Utc::now().timestamp() as u64,
                source: "subscription".to_string(),
            };

            let _ = update_sender.send(update);
        }
    }

    pub async fn stop_subscription(&self,
        subscription_id: &str,
    ) -> Result<()> {
        let mut subscriptions = self.subscriptions.write().await;
        
        if let Some(mut handle) = subscriptions.get_mut(subscription_id) {
            handle.active = false;
            subscriptions.remove(subscription_id);
            info!("Stopped subscription: {}", subscription_id);
        }

        Ok(())
    }

    pub async fn get_subscription_status(
        &self,
        subscription_id: &str,
    ) -> Option<SubscriptionHandle> {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.get(subscription_id).cloned()
    }

    pub async fn get_all_subscriptions(&self) -> Vec<SubscriptionHandle> {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.values().cloned().collect()
    }

    pub fn get_update_receiver(&self,
    ) -> mpsc::UnboundedReceiver<SubscriptionUpdate> {
        self.update_receiver.clone()
    }

    pub async fn stop_all_subscriptions(&self) -> Result<()> {
        let mut subscriptions = self.subscriptions.write().await;
        
        for (id, mut handle) in subscriptions.iter_mut() {
            handle.active = false;
            info!("Stopping subscription: {}", id);
        }
        
        subscriptions.clear();
        info!("All subscriptions stopped");
        
        Ok(())
    }

    pub async fn restart_subscription(
        &self,
        subscription_id: &str,
    ) -> Result<()> {
        let subscriptions = self.subscriptions.read().await;
        
        if let Some(handle) = subscriptions.get(subscription_id) {
            let subscription_type = Self::determine_subscription_type(subscription_id);
            
            self.start_subscription_worker(
                handle.id.clone(),
                handle.query.clone(),
                handle.variables.clone(),
                subscription_type,
            ).await?;
            
            info!("Restarted subscription: {}", subscription_id);
        }

        Ok(())
    }

    fn determine_subscription_type(subscription_id: &str) -> SubscriptionType {
        if subscription_id.contains("device_config") {
            SubscriptionType::DeviceConfig
        } else if subscription_id.contains("incidents") {
            SubscriptionType::IncidentNotifications
        } else if subscription_id.contains("upload_queue") {
            SubscriptionType::UploadQueueStatus
        } else if subscription_id.contains("alerts") {
            SubscriptionType::SystemAlerts
        } else {
            SubscriptionType::DeviceConfig // Default
        }
    }
}

// Helper functions for subscription management
#[derive(Debug, Clone)]
pub struct SubscriptionConfig {
    pub device_id: String,
    pub tenant_id: String,
    pub auto_reconnect: bool,
    pub reconnect_interval: Duration,
    pub max_reconnect_attempts: u32,
}

impl Default for SubscriptionConfig {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            tenant_id: String::new(),
            auto_reconnect: true,
            reconnect_interval: Duration::from_secs(30),
            max_reconnect_attempts: 5,
        }
    }
}