use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use tokio::fs;
use std::path::{Path, PathBuf};
use std::collections::VecDeque;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::config::Config;
use crate::device::BodycamDevice;
use crate::media::{MediaFileInfo, StorageBreakdown};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletedFileRecord {
    pub file_path: String,
    pub incident_id: Option<String>,
    pub quality: String,
    pub size_bytes: u64,
    pub deleted_at: DateTime<Utc>,
    pub deletion_reason: String,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageManager {
    device_id: String,
    config: Config,
    deleted_files: Vec<DeletedFileRecord>,
    max_storage_gb: u64,
    cleanup_threshold_gb: u64,
}

impl StorageManager {
    pub fn new(device_id: String, config: Config) -> Self {
        let max_storage_gb = config.storage.max_local_storage_gb as u64;
        let cleanup_threshold_gb = (max_storage_gb as f64 * 0.9) as u64; // 90% threshold
        
        Self {
            device_id,
            config,
            deleted_files: Vec::new(),
            max_storage_gb,
            cleanup_threshold_gb,
        }
    }

    pub async fn check_storage_and_cleanup(&mut self) -> Result<Vec<DeletedFileRecord>> {
        let media_dir = std::env::current_dir()?.join("media");
        if !media_dir.exists() {
            return Ok(Vec::new());
        }

        let total_storage = self.get_total_storage_usage(&media_dir).await?;
        let max_bytes = self.max_storage_gb * 1024 * 1024 * 1024;
        let cleanup_bytes = self.cleanup_threshold_gb * 1024 * 1024 * 1024;

        if total_storage > cleanup_bytes {
            let bytes_to_free = total_storage - cleanup_bytes + (100 * 1024 * 1024); // Free extra 100MB
            self.cleanup_oldest_files(bytes_to_free).await
        } else {
            Ok(Vec::new())
        }
    }

    async fn get_total_storage_usage(&self, media_dir: &Path) -> Result<u64> {
        let mut total = 0;
        let mut reader = fs::read_dir(media_dir).await?;
        
        while let Some(entry) = reader.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                let metadata = entry.metadata().await?;
                total += metadata.len();
            }
        }
        
        Ok(total)
    }

    async fn cleanup_oldest_files(&mut self, bytes_to_free: u64) -> Result<Vec<DeletedFileRecord>> {
        let media_dir = std::env::current_dir()?.join("media");
        let mut files = self.get_sorted_media_files(&media_dir).await?;
        
        let mut deleted_records = Vec::new();
        let mut freed_bytes = 0;

        for file_info in files {
            if freed_bytes >= bytes_to_free {
                break;
            }

            let file_path = PathBuf::from(&file_info.path);
            if file_path.exists() {
                let record = DeletedFileRecord {
                    file_path: file_info.path.clone(),
                    incident_id: file_info.incident_id.clone(),
                    quality: file_info.quality.clone(),
                    size_bytes: file_info.size_bytes,
                    deleted_at: Utc::now(),
                    deletion_reason: "automatic_storage_cleanup".to_string(),
                    device_id: self.device_id.clone(),
                };

                match fs::remove_file(&file_path).await {
                    Ok(_) => {
                        deleted_records.push(record.clone());
                        self.deleted_files.push(record);
                        freed_bytes += file_info.size_bytes;
                        tracing::info!("Deleted file due to storage cleanup: {}", file_info.path);
                    }
                    Err(e) => {
                        tracing::error!("Failed to delete file {}: {}", file_info.path, e);
                    }
                }
            }
        }

        Ok(deleted_records)
    }

    async fn get_sorted_media_files(&self, media_dir: &Path) -> Result<Vec<MediaFileInfo>> {
        let mut files = crate::media::get_media_files(media_dir).await?;
        
        // Sort by creation time (oldest first)
        files.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        
        Ok(files)
    }

    pub async fn delete_uploaded_file(&mut self, file_path: &str) -> Result<DeletedFileRecord> {
        let path = PathBuf::from(file_path);
        
        if !path.exists() {
            return Err(anyhow::anyhow!("File not found: {}", file_path));
        }

        let metadata = fs::metadata(&path).await?;
        
        // Parse incident_id and quality from filename
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        
        let incident_id = if file_name.contains("_incident_") {
            file_name.split("_incident_")
                .nth(1)
                .and_then(|s| s.split('_').next())
                .map(|s| s.to_string())
        } else {
            None
        };

        let quality = if file_name.contains("_ultra_") {
            "Ultra"
        } else if file_name.contains("_high_") {
            "High"
        } else if file_name.contains("_medium_") {
            "Medium"
        } else if file_name.contains("_low_") {
            "Low"
        } else {
            "Unknown"
        };

        let record = DeletedFileRecord {
            file_path: file_path.to_string(),
            incident_id,
            quality: quality.to_string(),
            size_bytes: metadata.len(),
            deleted_at: Utc::now(),
            deletion_reason: "upload_complete_cleanup".to_string(),
            device_id: self.device_id.clone(),
        };

        fs::remove_file(&path).await?;
        self.deleted_files.push(record.clone());
        
        tracing::info!("Deleted uploaded file: {}", file_path);
        Ok(record)
    }

    pub fn get_deleted_files(&self) -> &[DeletedFileRecord] {
        &self.deleted_files
    }

    pub fn get_recent_deletions(&self, limit: usize) -> Vec<DeletedFileRecord> {
        self.deleted_files
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub async fn save_deletion_log(&self) -> Result<()> {
        let log_path = std::env::current_dir()?.join("logs");
        fs::create_dir_all(&log_path).await?;
        
        let file_path = log_path.join(format!("deletions_{}.json", Utc::now().format("%Y-%m-%d")));
        let log_content = serde_json::to_string_pretty(&self.deleted_files)?;
        
        fs::write(file_path, log_content).await?;
        Ok(())
    }

    pub async fn clear_deletion_log(&mut self) -> Result<()> {
        self.deleted_files.clear();
        
        let log_path = std::env::current_dir()?.join("logs");
        if log_path.exists() {
            let mut entries = fs::read_dir(&log_path).await?;
            while let Some(entry) = entries.next_entry().await? {
                let file_name = entry.file_name();
                if file_name.to_string_lossy().starts_with("deletions_") {
                    let _ = fs::remove_file(entry.path()).await;
                }
            }
        }
        
        Ok(())
    }
}