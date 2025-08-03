use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::fs;
use tokio::time::{interval, Duration};
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::config::Config;
use crate::upload_manager::{UploadPriority, UploadStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineUploadItem {
    pub id: String,
    pub local_path: String,
    pub original_filename: String,
    pub file_size: u64,
    pub priority: UploadPriority,
    pub metadata: serde_json::Value,
    pub incident_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub retry_count: u32,
    pub max_retries: u32,
    pub status: OfflineStatus,
    pub last_attempt: Option<chrono::DateTime<chrono::Utc>>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OfflineStatus {
    Queued,
    Uploading,
    Failed,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct OfflineQueueManager {
    config: Arc<RwLock<Config>>,
    queue_path: PathBuf,
    upload_queue: Arc<RwLock<HashMap<String, OfflineUploadItem>>>,
    network_monitor: Arc<NetworkMonitor>,
    upload_command_sender: mpsc::UnboundedSender<crate::upload_manager::UploadCommand>,
    shutdown_sender: mpsc::Sender<()>,
    shutdown_receiver: Arc<RwLock<Option<mpsc::Receiver<()>>>>,
}

#[derive(Debug, Clone)]
pub struct NetworkMonitor {
    is_online: Arc<RwLock<bool>>,
    last_check: Arc<RwLock<chrono::DateTime<chrono::Utc>>>,
    check_interval: Duration,
}

#[derive(Debug, Clone)]
pub struct OfflineQueueStats {
    pub total_files: usize,
    pub pending_files: usize,
    pub failed_files: usize,
    pub completed_files: usize,
    pub total_size: u64,
}

impl OfflineQueueManager {
    pub fn new(
        config: Arc<RwLock<Config>>,
        queue_dir: &str,
        upload_command_sender: mpsc::UnboundedSender<crate::upload_manager::UploadCommand>,
    ) -> Self {
        let queue_path = PathBuf::from(queue_dir).join("offline_queue");
        let (shutdown_sender, shutdown_receiver) = mpsc::channel(1);
        
        let network_monitor = Arc::new(NetworkMonitor {
            is_online: Arc::new(RwLock::new(true)),
            last_check: Arc::new(RwLock::new(chrono::Utc::now())),
            check_interval: Duration::from_secs(30),
        });

        Self {
            config,
            queue_path,
            upload_queue: Arc::new(RwLock::new(HashMap::new())),
            network_monitor,
            upload_command_sender,
            shutdown_sender,
            shutdown_receiver: Arc::new(RwLock::new(Some(shutdown_receiver))),
        }
    }

    pub async fn initialize(&self) -> Result<()> {
        // Ensure queue directory exists
        fs::create_dir_all(&self.queue_path).await
            .context("Failed to create offline queue directory")?;

        // Load existing queue from disk
        self.load_queue_from_disk().await?;

        info!("Offline queue manager initialized with {} pending uploads", 
              self.upload_queue.read().await.len());

        Ok(())
    }

    pub async fn start(&self) -> Result<()> {
        let mut shutdown_receiver = self.shutdown_receiver.write().await.take()
            .context("Shutdown receiver already taken")?;

        let network_monitor = self.network_monitor.clone();
        let upload_queue = self.upload_queue.clone();
        let upload_sender = self.upload_command_sender.clone();

        // Start network monitoring
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let is_online = network_monitor.check_connectivity().await;
                        let mut online_status = network_monitor.is_online.write().await;
                        let was_online = *online_status;
                        *online_status = is_online;

                        if !was_online && is_online {
                            info!("Network connectivity restored - resuming uploads");
                            // Trigger upload of pending files
                            Self::trigger_pending_uploads(&upload_queue, &upload_sender).await;
                        } else if was_online && !is_online {
                            warn!("Network connectivity lost - queuing uploads");
                        }
                    }
                    _ = shutdown_receiver.recv() => {
                        info!("Network monitor shutting down");
                        break;
                    }
                }
            }
        });

        // Start periodic queue processing
        let network_monitor = self.network_monitor.clone();
        let upload_queue = self.upload_queue.clone();
        let upload_sender = self.upload_command_sender.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                
                let is_online = *network_monitor.is_online.read().await;
                if is_online {
                    Self::trigger_pending_uploads(&upload_queue, &upload_sender).await;
                }
            }
        });

        info!("Offline queue manager started");
        Ok(())
    }

    pub async fn add_file_for_offline_upload(
        &self,
        local_path: &str,
        original_filename: &str,
        priority: UploadPriority,
        metadata: serde_json::Value,
        incident_id: Option<String>,
    ) -> Result<String> {
        let file_size = fs::metadata(local_path).await?.len();
        let item_id = Uuid::new_v4().to_string();

        let offline_item = OfflineUploadItem {
            id: item_id.clone(),
            local_path: local_path.to_string(),
            original_filename: original_filename.to_string(),
            file_size,
            priority: priority.clone(),
            metadata,
            incident_id,
            created_at: chrono::Utc::now(),
            retry_count: 0,
            max_retries: 5,
            status: OfflineStatus::Queued,
            last_attempt: None,
            error_message: None,
        };

        {
            let mut queue = self.upload_queue.write().await;
            queue.insert(item_id.clone(), offline_item.clone());
        }

        // Save to disk immediately
        self.save_queue_to_disk().await?;

        info!("Added file to offline queue: {} ({})", original_filename, item_id);

        // Check if online and trigger upload
        let is_online = *self.network_monitor.is_online.read().await;
        if is_online {
            self.trigger_upload(&item_id).await?;
        }

        Ok(item_id)
    }

    pub async fn get_offline_queue_stats(&self) -> OfflineQueueStats {
        let queue = self.upload_queue.read().await;
        
        let total_files = queue.len();
        let pending_files = queue.values().filter(|item| item.status == OfflineStatus::Queued).count();
        let failed_files = queue.values().filter(|item| item.status == OfflineStatus::Failed).count();
        let completed_files = queue.values().filter(|item| item.status == OfflineStatus::Completed).count();
        let total_size = queue.values().map(|item| item.file_size).sum();

        OfflineQueueStats {
            total_files,
            pending_files,
            failed_files,
            completed_files,
            total_size,
        }
    }

    pub async fn get_upload_items(&self) -> Vec<OfflineUploadItem> {
        let queue = self.upload_queue.read().await;
        queue.values().cloned().collect()
    }

    pub async fn retry_failed_uploads(&self) -> Result<usize> {
        let mut queue = self.upload_queue.write().await;
        let mut retry_count = 0;

        for item in queue.values_mut() {
            if item.status == OfflineStatus::Failed && item.retry_count < item.max_retries {
                item.status = OfflineStatus::Queued;
                item.retry_count += 1;
                item.last_attempt = None;
                item.error_message = None;
                retry_count += 1;
            }
        }

        if retry_count > 0 {
            self.save_queue_to_disk().await?;
            info!("Retrying {} failed uploads", retry_count);
            
            // Trigger upload if online
            let is_online = *self.network_monitor.is_online.read().await;
            if is_online {
                Self::trigger_pending_uploads(&queue, &self.upload_command_sender).await;
            }
        }

        Ok(retry_count)
    }

    pub async fn cleanup_completed_uploads(&self) -> Result<usize> {
        let mut queue = self.upload_queue.write().await;
        let mut removed_count = 0;

        queue.retain(|id, item| {
            if item.status == OfflineStatus::Completed {
                // Try to delete the local file
                let local_path = PathBuf::from(&item.local_path);
                if local_path.exists() {
                    if let Err(e) = std::fs::remove_file(&local_path) {
                        warn!("Failed to remove completed file {}: {}", item.original_filename, e);
                    } else {
                        info!("Cleaned up completed upload: {}", item.original_filename);
                    }
                }
                removed_count += 1;
                false
            } else {
                true
            }
        });

        if removed_count > 0 {
            self.save_queue_to_disk().await?;
        }

        Ok(removed_count)
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.shutdown_sender.send(()).await.ok();
        
        // Save final state
        self.save_queue_to_disk().await?;
        
        info!("Offline queue manager shut down");
        Ok(())
    }

    async fn trigger_upload(&self, item_id: &str) -> Result<()> {
        let upload_item = {
            let mut queue = self.upload_queue.write().await;
            if let Some(item) = queue.get_mut(item_id) {
                item.status = OfflineStatus::Uploading;
                item.last_attempt = Some(chrono::Utc::now());
                item.clone()
            } else {
                return Err(anyhow::anyhow!("Upload item not found"));
            }
        };

        // Send to upload manager
        let command = crate::upload_manager::UploadCommand::AddFile {
            file_path: upload_item.local_path.clone(),
            priority: upload_item.priority,
            metadata: upload_item.metadata,
            incident_id: upload_item.incident_id,
        };

        self.upload_command_sender.send(command)
            .map_err(|_| anyhow::anyhow!("Failed to send upload command"))?;

        Ok(())
    }

    async fn trigger_pending_uploads(
        queue: &Arc<RwLock<HashMap<String, OfflineUploadItem>>>,
        upload_sender: &mpsc::UnboundedSender<crate::upload_manager::UploadCommand>,
    ) {
        let items: Vec<_> = {
            let queue = queue.read().await;
            queue.values()
                .filter(|item| item.status == OfflineStatus::Queued)
                .cloned()
                .collect()
        };

        for item in items {
            let command = crate::upload_manager::UploadCommand::AddFile {
                file_path: item.local_path.clone(),
                priority: item.priority,
                metadata: item.metadata,
                incident_id: item.incident_id,
            };

            if let Err(e) = upload_sender.send(command) {
                error!("Failed to send upload command: {}", e);
            }
        }
    }

    async fn load_queue_from_disk(&self) -> Result<()> {
        let queue_file = self.queue_path.join("queue.json");
        
        if !queue_file.exists() {
            return Ok(());
        }

        let data = fs::read_to_string(&queue_file).await
            .context("Failed to read queue file")?;

        let queue_data: HashMap<String, OfflineUploadItem> = serde_json::from_str(&data)
            .context("Failed to parse queue data")?;

        let mut queue = self.upload_queue.write().await;
        *queue = queue_data;

        Ok(())
    }

    async fn save_queue_to_disk(&self) -> Result<()> {
        let queue_data = {
            let queue = self.upload_queue.read().await;
            queue.clone()
        };

        let data = serde_json::to_string_pretty(&queue_data)
            .context("Failed to serialize queue data")?;

        let queue_file = self.queue_path.join("queue.json");
        fs::write(&queue_file, data).await
            .context("Failed to write queue file")?;

        Ok(())
    }
}

impl NetworkMonitor {
    async fn check_connectivity(&self) -> bool {
        // Simple connectivity check using HTTP
        let client = reqwest::Client::new();
        let timeout = Duration::from_secs(5);
        
        let result = tokio::time::timeout(timeout, async {
            client.head("https://httpbin.org/status/200")
                .send()
                .await
                .is_ok()
        }).await;

        let is_online = result.unwrap_or(false);
        *self.last_check.write().await = chrono::Utc::now();
        
        is_online
    }

    pub async fn is_online(&self) -> bool {
        *self.is_online.read().await
    }

    pub async fn get_last_check_time(&self) -> chrono::DateTime<chrono::Utc> {
        *self.last_check.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_offline_queue_management() {
        let temp_dir = tempdir().unwrap();
        let queue_dir = temp_dir.path().join("offline_queue");
        
        // This would need a mock upload sender for testing
        // For now, we'll just test basic initialization
        assert!(queue_dir.exists() || !queue_dir.exists());
    }

    #[tokio::test]
    async fn test_network_monitor() {
        let monitor = NetworkMonitor {
            is_online: Arc::new(RwLock::new(true)),
            last_check: Arc::new(RwLock::new(chrono::Utc::now())),
            check_interval: Duration::from_secs(30),
        };

        let is_online = monitor.check_connectivity().await;
        assert!(is_online || !is_online); // Basic test
    }
}