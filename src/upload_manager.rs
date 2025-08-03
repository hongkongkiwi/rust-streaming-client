use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, Semaphore};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::time::{interval, Duration};
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::config::Config;
use crate::convex_api::ConvexApiClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadFile {
    pub id: String,
    pub local_path: String,
    pub filename: String,
    pub file_size: u64,
    pub chunk_size: u64,
    pub total_chunks: u64,
    pub uploaded_chunks: Vec<u32>,
    pub status: UploadStatus,
    pub priority: UploadPriority,
    pub metadata: serde_json::Value,
    pub incident_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub retry_count: u32,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UploadStatus {
    Pending,
    Uploading,
    Paused,
    Failed,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum UploadPriority {
    Critical, // SOS incidents, system alerts
    High,     // Incidents, emergency recordings
    Medium,   // Scheduled uploads, background tasks
    Low,      // Historical data, maintenance uploads
}

#[derive(Debug, Clone)]
pub struct UploadChunk {
    pub file_id: String,
    pub chunk_index: u32,
    pub data: Vec<u8>,
    pub size: u64,
    pub md5_hash: String,
}

#[derive(Debug, Clone)]
pub struct UploadManager {
    api_client: Arc<RwLock<ConvexApiClient>>,
    upload_queue: Arc<RwLock<HashMap<String, UploadFile>>>,
    active_uploads: Arc<RwLock<HashMap<String, Semaphore>>>,
    upload_sender: mpsc::UnboundedSender<UploadCommand>,
    upload_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<UploadCommand>>>>,
    max_concurrent_uploads: usize,
    max_retries: u32,
    chunk_size: u64,
}

#[derive(Debug, Clone)]
pub enum UploadCommand {
    AddFile {
        file_path: String,
        priority: UploadPriority,
        metadata: serde_json::Value,
        incident_id: Option<String>,
    },
    StartUpload {
        file_id: String,
    },
    PauseUpload {
        file_id: String,
    },
    ResumeUpload {
        file_id: String,
    },
    CancelUpload {
        file_id: String,
    },
    RetryUpload {
        file_id: String,
    },
    UpdateStatus {
        file_id: String,
        status: UploadStatus,
    },
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct UploadProgress {
    pub file_id: String,
    pub filename: String,
    pub progress: f64,
    pub status: UploadStatus,
    pub bytes_uploaded: u64,
    pub bytes_total: u64,
    pub speed: f64, // bytes per second
    pub eta: Option<Duration>,
}

impl UploadManager {
    pub fn new(
        api_client: Arc<RwLock<ConvexApiClient>>,
        max_concurrent_uploads: usize,
        max_retries: u32,
        chunk_size: u64,
    ) -> Self {
        let (upload_sender, upload_receiver) = mpsc::unbounded_channel();
        
        Self {
            api_client,
            upload_queue: Arc::new(RwLock::new(HashMap::new())),
            active_uploads: Arc::new(RwLock::new(HashMap::new())),
            upload_sender,
            upload_receiver: Arc::new(RwLock::new(Some(upload_receiver))),
            max_concurrent_uploads,
            max_retries,
            chunk_size,
        }
    }

    pub async fn start(&self) -> Result<()> {
        let mut receiver = self.upload_receiver.write().await.take()
            .context("Upload receiver already taken")?;

        let api_client = self.api_client.clone();
        let upload_queue = self.upload_queue.clone();
        let active_uploads = self.active_uploads.clone();

        let max_concurrent_uploads = self.max_concurrent_uploads;
        let max_retries = self.max_retries;
        let chunk_size = self.chunk_size;

        tokio::spawn(async move {
            let semaphore = Arc::new(Semaphore::new(max_concurrent_uploads));
            let mut interval = interval(Duration::from_secs(5));

            loop {
                tokio::select! {
                    Some(command) = receiver.recv() => {
                        match command {
                            UploadCommand::AddFile { file_path, priority, metadata, incident_id } => {
                                if let Err(e) = self.add_file_to_queue(&file_path, priority, metadata, incident_id).await {
                                    error!("Failed to add file to queue: {}", e);
                                }
                            }
                            UploadCommand::StartUpload { file_id } => {
                                self.start_upload_worker(&file_id, semaphore.clone()).await;
                            }
                            UploadCommand::PauseUpload { file_id } => {
                                self.pause_upload(&file_id).await;
                            }
                            UploadCommand::ResumeUpload { file_id } => {
                                self.resume_upload(&file_id).await;
                            }
                            UploadCommand::CancelUpload { file_id } => {
                                self.cancel_upload(&file_id).await;
                            }
                            UploadCommand::RetryUpload { file_id } => {
                                self.retry_upload(&file_id).await;
                            }
                            UploadCommand::UpdateStatus { file_id, status } => {
                                self.update_upload_status(&file_id, status).await;
                            }
                            UploadCommand::Shutdown => {
                                info!("Upload manager shutting down...");
                                break;
                            }
                        }
                    }
                    _ = interval.tick() => {
                        self.process_pending_uploads().await;
                    }
                }
            }
        });

        info!("Upload manager started with {} concurrent uploads", self.max_concurrent_uploads);
        Ok(())
    }

    pub fn get_sender(&self) -> mpsc::UnboundedSender<UploadCommand> {
        self.upload_sender.clone()
    }

    async fn add_file_to_queue(
        &self,
        file_path: &str,
        priority: UploadPriority,
        metadata: serde_json::Value,
        incident_id: Option<String>,
    ) -> Result<String> {
        let path = Path::new(file_path);
        let file_size = fs::metadata(&path).await?.len();
        let total_chunks = ((file_size + self.chunk_size - 1) / self.chunk_size) as u64;

        let file_id = Uuid::new_v4().to_string();
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let upload_file = UploadFile {
            id: file_id.clone(),
            local_path: file_path.to_string(),
            filename,
            file_size,
            chunk_size: self.chunk_size,
            total_chunks,
            uploaded_chunks: Vec::new(),
            status: UploadStatus::Pending,
            priority,
            metadata,
            incident_id,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            retry_count: 0,
            max_retries: self.max_retries,
        };

        {
            let mut queue = self.upload_queue.write().await;
            queue.insert(file_id.clone(), upload_file);
        }

        info!("Added file {} to upload queue with ID: {}", file_path, file_id);
        
        // Automatically start upload for high priority files
        if priority >= UploadPriority::High {
            self.start_upload_worker(&file_id, Arc::new(Semaphore::new(1))).await;
        }

        Ok(file_id)
    }

    async fn start_upload_worker(
        &self,
        file_id: &str,
        semaphore: Arc<Semaphore>,
    ) {
        let file_id = file_id.to_string();
        let convex_integration = self.convex_integration.clone();
        let upload_queue = self.upload_queue.clone();
        let active_uploads = self.active_uploads.clone();

        tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            
            let upload_file = {
                let mut queue = upload_queue.write().await;
                if let Some(file) = queue.get_mut(&file_id) {
                    file.status = UploadStatus::Uploading;
                    file.updated_at = chrono::Utc::now();
                    file.clone()
                } else {
                    error!("Upload file {} not found in queue", file_id);
                    return;
                }
            };

            let semaphore = Arc::new(Semaphore::new(3)); // Allow 3 concurrent chunk uploads
            active_uploads.write().await.insert(file_id.clone(), semaphore.clone());

            match Self::upload_file_chunks(&upload_file, &api_client, semaphore).await {
                Ok(_) => {
                    info!("Successfully uploaded file: {}", upload_file.filename);
                    Self::update_upload_status(&upload_queue, &file_id, UploadStatus::Completed).await;
                    
                    // Clean up local file after successful upload
                    if let Err(e) = fs::remove_file(&upload_file.local_path).await {
                        warn!("Failed to remove local file {}: {}", upload_file.local_path, e);
                    }
                }
                Err(e) => {
                    error!("Failed to upload file {}: {}", upload_file.filename, e);
                    Self::handle_upload_failure(&upload_queue, &file_id, e).await;
                }
            }

            active_uploads.write().await.remove(&file_id);
        });
    }

    async fn upload_file_chunks(
        upload_file: &UploadFile,
        api_client: &Arc<RwLock<ConvexApiClient>>,
        semaphore: Arc<Semaphore>,
    ) -> Result<()> {
        let mut tasks = Vec::new();
        let client = api_client.read().await;

        // Start chunked upload session
        let upload_session_id = client.start_chunked_upload(
            &upload_file.filename,
            upload_file.file_size,
            upload_file.chunk_size,
            upload_file.metadata.clone(),
            upload_file.incident_id.clone(),
        ).await?;

        // Upload each chunk
        for chunk_index in 0..upload_file.total_chunks {
            let permit = semaphore.acquire().await.unwrap();
            
            let task = Self::upload_single_chunk(
                upload_file.clone(),
                chunk_index as u32,
                upload_session_id.clone(),
                client.clone(),
                permit,
            );
            
            tasks.push(task);
        }

        // Wait for all chunks to complete
        let results = futures::future::join_all(tasks).await;
        
        // Check for any failures
        for (index, result) in results.iter().enumerate() {
            if let Err(e) = result {
                error!("Failed to upload chunk {}: {}", index, e);
                return Err(anyhow::anyhow!("Chunk upload failed: {}", e));
            }
        }

        // Complete the upload
        client.complete_chunked_upload(&upload_session_id).await?;
        
        info!("Completed chunked upload for file: {}", upload_file.filename);
        Ok(())
    }

    async fn upload_single_chunk(
        upload_file: UploadFile,
        chunk_index: u32,
        upload_session_id: String,
        client: Arc<RwLock<ConvexApiClient>>,
        _permit: tokio::sync::SemaphorePermit<'_>,
    ) -> Result<()> {
        let start_offset = (chunk_index as u64) * upload_file.chunk_size;
        let end_offset = std::cmp::min(
            start_offset + upload_file.chunk_size,
            upload_file.file_size,
        );
        let chunk_size = end_offset - start_offset;

        // Read chunk data
        let mut file = fs::File::open(&upload_file.local_path).await?;
        file.seek(std::io::SeekFrom::Start(start_offset)).await?;
        
        let mut chunk_data = vec![0u8; chunk_size as usize];
        file.read_exact(&mut chunk_data).await?;

        // Calculate MD5 hash
        let md5_hash = format!("{:x}", md5::compute(&chunk_data));

        // Upload chunk
        let client_guard = client.read().await;
        client_guard.upload_chunk(
            &upload_session_id,
            chunk_index,
            &chunk_data,
            &md5_hash,
        ).await?;

        info!("Uploaded chunk {}/{} for file {}", 
              chunk_index + 1, upload_file.total_chunks, upload_file.filename);

        Ok(())
    }

    async fn process_pending_uploads(&self) {
        let queue = self.upload_queue.read().await;
        let mut pending_files: Vec<_> = queue.values()
            .filter(|f| f.status == UploadStatus::Pending || f.status == UploadStatus::Failed)
            .collect();

        // Sort by priority and creation time
        pending_files.sort_by(|a, b| {
            match a.priority.cmp(&b.priority) {
                std::cmp::Ordering::Equal => a.created_at.cmp(&b.created_at),
                other => other,
            }
        });

        drop(queue);

        for file in pending_files {
            if file.retry_count < file.max_retries {
                self.upload_sender.send(UploadCommand::StartUpload {
                    file_id: file.id.clone(),
                }).unwrap();
            }
        }
    }

    async fn pause_upload(&self, file_id: &str) {
        Self::update_upload_status(&self.upload_queue, file_id, UploadStatus::Paused).await;
    }

    async fn resume_upload(&self, file_id: &str) {
        Self::update_upload_status(&self.upload_queue, file_id, UploadStatus::Pending).await;
        self.upload_sender.send(UploadCommand::StartUpload {
            file_id: file_id.to_string(),
        }).unwrap();
    }

    async fn cancel_upload(&self, file_id: &str) {
        Self::update_upload_status(&self.upload_queue, file_id, UploadStatus::Cancelled).await;
    }

    async fn retry_upload(&self, file_id: &str) {
        Self::update_upload_status(&self.upload_queue, file_id, UploadStatus::Pending).await;
        self.upload_sender.send(UploadCommand::StartUpload {
            file_id: file_id.to_string(),
        }).unwrap();
    }

    async fn update_upload_status(&self, file_id: &str, status: UploadStatus) {
        Self::update_upload_status(&self.upload_queue, file_id, status).await;
    }

    async fn update_upload_status(
        queue: &Arc<RwLock<HashMap<String, UploadFile>>>,
        file_id: &str,
        status: UploadStatus,
    ) {
        let mut queue = queue.write().await;
        if let Some(file) = queue.get_mut(file_id) {
            file.status = status;
            file.updated_at = chrono::Utc::now();
        }
    }

    async fn handle_upload_failure(
        queue: &Arc<RwLock<HashMap<String, UploadFile>>>,
        file_id: &str,
        error: anyhow::Error,
    ) {
        let mut queue = queue.write().await;
        if let Some(file) = queue.get_mut(file_id) {
            file.retry_count += 1;
            file.status = if file.retry_count >= file.max_retries {
                UploadStatus::Failed
            } else {
                UploadStatus::Pending
            };
            file.updated_at = chrono::Utc::now();
        }
    }

    pub async fn get_upload_progress(&self, file_id: &str) -> Option<UploadProgress> {
        let queue = self.upload_queue.read().await;
        queue.get(file_id).map(|file| {
            let uploaded_chunks = file.uploaded_chunks.len() as u64;
            let total_chunks = file.total_chunks;
            let progress = if total_chunks > 0 {
                (uploaded_chunks as f64 / total_chunks as f64) * 100.0
            } else {
                0.0
            };

            UploadProgress {
                file_id: file.id.clone(),
                filename: file.filename.clone(),
                progress,
                status: file.status.clone(),
                bytes_uploaded: uploaded_chunks * file.chunk_size,
                bytes_total: file.file_size,
                speed: 0.0, // TODO: Implement speed calculation
                eta: None,  // TODO: Implement ETA calculation
            }
        })
    }

    pub async fn get_all_uploads(&self) -> Vec<UploadProgress> {
        let queue = self.upload_queue.read().await;
        queue.values()
            .filter_map(|file| self.get_upload_progress(&file.id).await)
            .collect()
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.upload_sender.send(UploadCommand::Shutdown)?;
        
        // Wait for active uploads to complete
        let mut active_uploads = self.active_uploads.write().await;
        while !active_uploads.is_empty() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        info!("Upload manager shut down successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_add_file_to_queue() {
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_str().unwrap().to_string();
        
        // Create a test upload manager
        // Note: In real tests, you'd use a mock ConvexIntegration
    }

    #[tokio::test]
    async fn test_upload_priority_sorting() {
        // Test priority-based sorting
    }

    #[tokio::test]
    async fn test_chunk_calculation() {
        let file_size = 10_000_000; // 10MB
        let chunk_size = 1_000_000; // 1MB
        let expected_chunks = 10;
        
        let total_chunks = ((file_size + chunk_size - 1) / chunk_size) as u64;
        assert_eq!(total_chunks, expected_chunks);
    }
}