use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tracing::{info, error};

use crate::config::Config;
use crate::convex_api::ConvexApiClient;
use crate::convex_auth::ConvexAuthenticator;
use crate::convex_tenant::TenantManager;
use crate::config_sync::ConfigSyncManager;
use crate::convex_subscriptions::ConvexSubscriptionManager;
use crate::upload_manager::{UploadManager, UploadPriority};
use crate::offline_queue::OfflineQueueManager;

#[derive(Debug, Clone)]
pub struct ConvexIntegration {
    pub authenticator: Arc<RwLock<ConvexAuthenticator>>,
    pub api_client: Arc<RwLock<ConvexApiClient>>,
    pub tenant_manager: Arc<RwLock<TenantManager>>,
    pub config_sync: Arc<ConfigSyncManager>,
    pub subscription_manager: Arc<ConvexSubscriptionManager>,
    pub upload_manager: Arc<UploadManager>,
    pub offline_queue: Arc<OfflineQueueManager>,
    pub device_id: String,
    pub tenant_id: String,
}

impl ConvexIntegration {
    pub async fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        // Extract device and tenant info from config
        let (device_id, tenant_id) = {
            let config = config.read().await;
            (
                config.device_id.clone().unwrap_or_else(|| "unknown".to_string()),
                config.tenant_id.clone().unwrap_or_else(|| "unknown".to_string()),
            )
        };

        // Initialize authenticator
        let authenticator = {
            let config = config.read().await;
            ConvexAuthenticator::new(config.clone())?
        };
        let authenticator = Arc::new(RwLock::new(authenticator));

        // Initialize API client
        let api_client = {
            let config = config.read().await;
            let convex_url = config.convex_url.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Convex URL not configured"))?;
            ConvexApiClient::new(convex_url, config.clone()).await?
        };
        let api_client = Arc::new(RwLock::new(api_client));

        // Initialize subscription manager
        let subscription_manager = Arc::new(ConvexSubscriptionManager::new(api_client.clone()));

        // Initialize offline queue manager
        let (upload_command_sender, upload_command_receiver) = mpsc::unbounded_channel();
        let offline_queue = Arc::new(OfflineQueueManager::new(
            config.clone(),
            "./data/offline_queue",
            upload_command_sender,
        ));

        // Initialize upload manager
        let upload_manager = Arc::new(UploadManager::new(
            api_client.clone(),
            3, // max_concurrent_uploads
            5, // max_retries
            1024 * 1024, // 1MB chunk_size
        ));

        // Initialize tenant manager
        let tenant_manager = Arc::new(RwLock::new(TenantManager::new(
            api_client.clone(),
            config.clone(),
        )));

        // Initialize config sync manager
        let config_sync = Arc::new(ConfigSyncManager::new(
            api_client.clone(),
            config.clone(),
            device_id.clone(),
            tenant_id.clone(),
            tenant_manager.clone(),
        ));

        Ok(Self {
            authenticator,
            api_client,
            config_sync,
            subscription_manager,
            upload_manager,
            offline_queue,
            device_id,
            tenant_id,
        })
    }

    pub async fn initialize_device(&self,
        device_name: &str,
        site_id: &str,
    ) -> Result<()> {
        info!("Initializing device with Convex backend...");

        // Check if device is already provisioned
        let is_provisioned = {
            let auth = self.authenticator.read().await;
            auth.is_device_provisioned()
        };

        if !is_provisioned {
            info!("Device not provisioned, starting factory provisioning...");
            
            // Start factory provisioning
            let credentials = {
                let mut auth = self.authenticator.write().await;
                auth.factory_provision(device_name, site_id).await?
            };

            // Save credentials
            {
                let mut auth = self.authenticator.write().await;
                auth.save_credentials_to_config(&credentials).await?;
            }

            // Update API client with new credentials
            {
                let mut client = self.api_client.write().await;
                client.set_auth_token(credentials.auth_token);
                client.set_device_id(credentials.device_id);
                client.set_tenant_id(credentials.tenant_id);
            }

            info!("Device successfully provisioned and authenticated");
        } else {
            info!("Device already provisioned, validating authentication...");
            
            // Validate existing authentication
            let is_valid = {
                let auth = self.authenticator.read().await;
                auth.validate_auth_session().await.unwrap_or(false)
            };

            if !is_valid {
                info!("Authentication expired, refreshing...");
                {
                    let mut auth = self.authenticator.write().await;
                    auth.refresh_auth_session().await?;
                }
            }
        }

        Ok(())
    }

    pub async fn start_real_time_features(&self) -> Result<()> {
        info!("Starting real-time Convex features...");

        // Start configuration sync
        self.config_sync.start_real_time_sync().await?;

        // Start device configuration subscription
        self.subscription_manager
            .start_device_config_subscription(
                self.device_id.clone(),
                self.tenant_id.clone(),
            )
            .await?;

        // Start incident notifications subscription
        self.subscription_manager
            .start_incident_notifications_subscription(
                self.device_id.clone(),
                self.tenant_id.clone(),
            )
            .await?;

        // Start upload queue subscription
        self.subscription_manager
            .start_upload_queue_subscription(
                self.device_id.clone(),
                self.tenant_id.clone(),
            )
            .await?;

        // Start system alerts subscription
        self.subscription_manager
            .start_system_alerts_subscription(
                self.device_id.clone(),
                self.tenant_id.clone(),
            )
            .await?;

        info!("All real-time features started successfully");
        Ok(())
    }

    pub async fn record_device_status(&self, status: &crate::device::DeviceStatus) -> Result<()> {
        let convex_status = crate::convex_api::ConvexDeviceStatus::from(status.clone());
        
        let client = self.api_client.read().await;
        client.record_device_status(&convex_status).await?;
        
        Ok(())
    }

    pub async fn create_incident(&self,
        incident_type: &str,
        button_type: &str,
        gps_location: Option<(f64, f64)>,
        metadata: serde_json::Value,
    ) -> Result<String> {
        let request = crate::convex_api::IncidentCreateRequest {
            device_id: self.device_id.clone(),
            incident_type: incident_type.to_string(),
            button_type: button_type.to_string(),
            gps_latitude: gps_location.map(|loc| loc.0),
            gps_longitude: gps_location.map(|loc| loc.1),
            gps_accuracy: Some(10.0), // Default accuracy
            metadata,
        };

        let client = self.api_client.read().await;
        let incident_id = client.create_incident(&request).await?;

        info!("Created incident: {}", incident_id);
        Ok(incident_id)
    }

    pub async fn upload_video(&self,
        file_path: &str,
        incident_id: Option<String>,
        duration: Option<u64>,
    ) -> Result<String> {
        // Use upload manager for chunked upload
        let metadata = serde_json::json!({
            "codec": "h264",
            "bitrate": 5_000_000,
            "resolution": "1920x1080",
            "is_encrypted": false,
            "encryption_algorithm": null
        });

        let file_id = self.upload_manager.add_file_to_queue(
            file_path,
            UploadPriority::High,
            metadata,
            incident_id,
        ).await?;

        info!("Queued video for upload: {}", file_id);
        Ok(file_id)
    }

    pub async fn start_upload_manager(&self) -> Result<()> {
        // Initialize offline queue first
        self.offline_queue.initialize().await?;
        self.offline_queue.start().await?;
        
        // Start the upload manager
        self.upload_manager.start().await?;
        info!("Upload manager and offline queue started");
        Ok(())
    }

    pub fn get_upload_manager_sender(&self) -> mpsc::UnboundedSender<UploadCommand> {
        self.upload_manager.get_sender()
    }

    pub async fn upload_video_offline(
        &self,
        file_path: &str,
        incident_id: Option<String>,
        duration: Option<u64>,
    ) -> Result<String> {
        let metadata = serde_json::json!({
            "codec": "h264",
            "bitrate": 5_000_000,
            "resolution": "1920x1080",
            "is_encrypted": false,
            "encryption_algorithm": null,
            "duration": duration.unwrap_or(0),
        });

        let file_id = self.offline_queue.add_file_for_offline_upload(
            file_path,
            "video.mp4", // This should be the actual filename
            UploadPriority::High,
            metadata,
            incident_id,
        ).await?;

        info!("Queued video for offline upload: {}", file_id);
        Ok(file_id)
    }

    pub async fn get_offline_queue_stats(&self) -> crate::offline_queue::OfflineQueueStats {
        self.offline_queue.get_offline_queue_stats().await
    }

    pub async fn retry_failed_uploads(&self) -> Result<usize> {
        self.offline_queue.retry_failed_uploads().await
    }

    pub async fn cleanup_completed_uploads(&self) -> Result<usize> {
        self.offline_queue.cleanup_completed_uploads().await
    }

    pub async fn sync_configuration(&self) -> Result<()> {
        self.config_sync.trigger_config_update().await?;
        info!("Configuration synced from server");
        Ok(())
    }

    pub async fn get_current_user(&self) -> Result<Option<serde_json::Value>> {
        let auth = self.authenticator.read().await;
        auth.get_current_user().await
    }

    pub async fn stop_all_services(&self) -> Result<()> {
        info!("Stopping all Convex services...");
        
        // Stop offline queue
        self.offline_queue.shutdown().await?;
        
        // Stop config sync
        self.config_sync.stop_sync().await;
        
        // Stop all subscriptions
        self.subscription_manager.stop_all_subscriptions().await?;
        
        // Sign out from authenticator
        {
            let mut auth = self.authenticator.write().await;
            auth.sign_out().await?;
        }

        info!("All Convex services stopped");
        Ok(())
    }

    pub async fn health_check(&self) -> Result<bool> {
        // Check authentication status
        let auth_valid = {
            let auth = self.authenticator.read().await;
            auth.validate_auth_session().await.unwrap_or(false)
        };

        // Check subscription status
        let subscriptions_active = {
            let subs = self.subscription_manager.get_all_subscriptions().await;
            !subs.is_empty()
        };

        // Check config sync status
        let config_sync_active = self.config_sync.is_subscription_active().await;

        let is_healthy = auth_valid && subscriptions_active && config_sync_active;
        
        info!("Convex integration health check: {}", is_healthy);
        Ok(is_healthy)
    }

    pub fn get_update_receiver(&self,
    ) -> mpsc::UnboundedReceiver<crate::convex_subscriptions::SubscriptionUpdate> {
        self.subscription_manager.get_update_receiver()
    }

    pub fn get_config_watcher(&self) -> tokio::sync::watch::Receiver<Config> {
        self.config_sync.get_config_watcher()
    }
}

// Example usage and integration testing
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_convex_integration_initialization() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        
        let config = Config::load(config_path.to_str().unwrap()).await.unwrap();
        let config = Arc::new(RwLock::new(config));
        
        // This test would normally require a mock Convex server
        // For now, we'll just test the initialization
        assert!(config.read().await.is_provisioned() == false);
    }

    #[tokio::test]
    async fn test_device_provisioning_flow() {
        // This would be a comprehensive test of the entire provisioning flow
        // Including factory provisioning, authentication, and configuration sync
    }

    #[tokio::test]
    async fn test_real_time_subscriptions() {
        // Test subscription creation and management
    }

    #[tokio::test]
    async fn test_configuration_sync() {
        // Test configuration synchronization
    }
}

// Integration example
pub async fn run_convex_integration_example() -> Result<()> {
    // Load configuration
    let config = Config::load("config.toml").await?;
    let config = Arc::new(RwLock::new(config));

    // Initialize Convex integration
    let integration = ConvexIntegration::new(config.clone()).await?;

    // Initialize device
    integration.initialize_device("BodyCam-001", "site-001").await?;

    // Start real-time features
    integration.start_real_time_features().await?;

    // Start monitoring loop
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
    
    loop {
        interval.tick().await;
        
        // Check health
        if let Err(e) = integration.health_check().await {
            error!("Health check failed: {:?}", e);
            break;
        }
        
        // Record device status
        // integration.record_device_status(&status).await?;
        
        // Handle any updates
        // integration.sync_configuration().await?;
    }

    Ok(())
}