use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::process::Command;
use std::path::PathBuf;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;

use crate::config::{Config, VideoQuality};
use crate::buffer::{BufferSegment, CircularBuffer};
use crate::integrity::{IntegrityManager, VideoIntegrity, IntegrityVerification};
use crate::encryption::{MediaEncryptor, EncryptionMetadata};

#[derive(Debug, Serialize, Deserialize)]
pub struct RecordingSegment {
    pub id: String,
    pub incident_id: String,
    pub device_id: String,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub duration: Option<u64>,
    pub file_path: String,
    pub file_size: Option<u64>,
    pub metadata: RecordingMetadata,
    pub uploaded: bool,
    pub quality: VideoQuality,
    pub pre_incident_segments: Vec<BufferSegment>,
    pub integrity: Option<VideoIntegrity>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecordingMetadata {
    pub resolution: String,
    pub fps: u32,
    pub bitrate: u32,
    pub codec: String,
    pub audio_enabled: bool,
    pub audio_codec: String,
    pub encryption_key: Option<String>,
    pub location: Option<LocationData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocationData {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct MediaRecorder {
    config: Config,
    device_id: String,
    incident_id: String,
    duration: Option<u64>,
    current_segments: HashMap<VideoQuality, RecordingSegment>,
    recording_processes: HashMap<VideoQuality, tokio::process::Child>,
    buffer: CircularBuffer,
    encryptor: Option<MediaEncryptor>,
}

impl MediaRecorder {
    pub fn new(
        config: Config,
        device_id: String,
        incident_id: String,
        duration: Option<u64>,
    ) -> Self {
        let buffer = CircularBuffer::new(config.clone(), device_id.clone());
        Self {
            config,
            device_id,
            incident_id,
            duration,
            current_segments: HashMap::new(),
            recording_processes: HashMap::new(),
            buffer,
            encryptor: None,
        }
    }

    pub async fn initialize_encryption(&mut self, encryption_key: Option<String>) -> Result<()> {
        if let Some(key) = encryption_key {
            let mut encryptor = MediaEncryptor::new(self.device_id.clone());
            if key.starts_with("password:") {
                let password = &key[9..]; // Remove "password:" prefix
                encryptor.initialize_with_password(password).await?;
            } else {
                encryptor.initialize_with_device_key(&key).await?;
            }
            self.encryptor = Some(encryptor);
        }
        Ok(())
    }

    pub async fn start(&mut self) -> Result<()> {
        // Get pre-incident buffer segments
        let pre_incident_segments = self.buffer.get_buffer_segments(
            self.config.recording.pre_incident_buffer_seconds
        ).await?;
        
        // Start recording for each configured quality
        for quality_config in &self.config.recording.available_qualities {
            let segment_id = Uuid::new_v4().to_string();
            let start_time = Utc::now();
            
            let storage_path = self.get_storage_path().await?;
            let file_name = format!("{}_{}_{}_{}.mp4", 
                self.device_id, 
                self.incident_id, 
                segment_id,
                match quality_config.quality {
                    VideoQuality::Low => "low",
                    VideoQuality::Medium => "med",
                    VideoQuality::High => "high",
                    VideoQuality::Ultra => "ultra",
                }
            );
            let file_path = storage_path.join(file_name);
            
            let metadata = RecordingMetadata {
                resolution: quality_config.resolution.clone(),
                fps: quality_config.fps,
                bitrate: quality_config.bitrate,
                codec: quality_config.codec.clone(),
                audio_enabled: self.config.audio.enabled,
                audio_codec: "aac".to_string(),
                encryption_key: if self.encryptor.is_some() { 
                    Some("AES-256-GCM".to_string()) 
                } else { 
                    None 
                },
                location: None,
            };

            let segment = RecordingSegment {
                id: segment_id,
                incident_id: self.incident_id.clone(),
                device_id: self.device_id.clone(),
                start_time,
                end_time: None,
                duration: None,
                file_path: file_path.to_string_lossy().to_string(),
                file_size: None,
                metadata,
                uploaded: false,
                quality: quality_config.quality.clone(),
                pre_incident_segments: pre_incident_segments.clone(),
            };

            self.current_segments.insert(quality_config.quality.clone(), segment);
            
            if !self.config.simulation.enabled {
                self.start_real_recording(quality_config, &file_path).await?;
            } else {
                self.start_simulated_recording(quality_config, &file_path).await?;
            }
        }

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        let mut segments_to_upload = Vec::new();
        
        for (quality, mut segment) in self.current_segments.drain() {
            segment.end_time = Some(Utc::now());
            segment.duration = segment.end_time
                .map(|end| (end - segment.start_time).num_seconds() as u64);
            
            if let Some(mut process) = self.recording_processes.remove(&quality) {
                // Properly terminate the process and wait for cleanup
                if let Err(e) = process.kill().await {
                    tracing::warn!("Failed to kill recording process: {}", e);
                }
                
                // Wait for the process to fully terminate to prevent zombies
                if let Err(e) = process.wait().await {
                    tracing::warn!("Failed to wait for process cleanup: {}", e);
                }
                
                tracing::info!("Recording process properly terminated for quality: {:?}", quality);
            }

            if let Ok(metadata) = fs::metadata(&segment.file_path).await {
                segment.file_size = Some(metadata.len());
            }

            // Encrypt the recording if encryption is enabled
            if let Some(encryptor) = &self.encryptor {
                let original_path = PathBuf::from(&segment.file_path);
                let encrypted_path = original_path.with_extension("encrypted.mp4");
                
                match encryptor.encrypt_video_file(&original_path, &encrypted_path).await {
                    Ok(encryption_metadata) => {
                        // Verify encrypted file exists and has reasonable size before deleting original
                        if encrypted_path.exists() && encryption_metadata.encrypted_size > 0 {
                            // Safely remove original unencrypted file
                            if let Err(e) = fs::remove_file(&original_path).await {
                                tracing::error!("Failed to remove original file after encryption: {}", e);
                                // Keep both files rather than risk data loss
                            } else {
                                tracing::info!("Original unencrypted file safely removed after encryption");
                            }
                            
                            // Update segment to point to encrypted file
                            segment.file_path = encrypted_path.to_string_lossy().to_string();
                            segment.file_size = Some(encryption_metadata.encrypted_size);
                            
                            tracing::info!("Successfully encrypted recording segment: {}", segment.id);
                        } else {
                            tracing::error!("Encrypted file verification failed - keeping original file");
                            // Don't update segment path, keep original
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to encrypt recording segment {}: {}", segment.id, e);
                        // Continue without encryption rather than failing the entire operation
                    }
                }
            }

            // Create integrity record for the segment
            self.create_integrity_record(&mut segment).await?;
            
            // Verify integrity before saving
            let verification = self.verify_segment_integrity(&segment).await;
            if let Ok(verification) = verification {
                if !verification.is_valid {
                    tracing::error!("Integrity verification failed for segment {}", segment.id);
                }
            }

            // Save segment metadata
            self.save_segment_metadata(&segment).await?;
            
            // Upload based on default quality setting
            if quality == self.config.recording.default_quality && self.config.network.upload_bandwidth > 0 {
                segments_to_upload.push(segment);
            }
        }

        // Upload selected quality segments
        for segment in segments_to_upload {
            self.upload_segment(&segment).await?;
        }

        self.current_segments.clear();
        self.recording_processes.clear();
        Ok(())
    }

async fn start_real_recording(
        &mut self, 
        quality_config: &crate::config::VideoQualityConfig, 
        file_path: &PathBuf
    ) -> Result<()> {
        let duration_arg = self.duration
            .map(|d| format!("-t {}", d))
            .unwrap_or_default();

        let mut cmd = Command::new("ffmpeg");
        
        cmd.arg("-f")
           .arg("v4l2")
           .arg("-i")
           .arg(&quality_config.device_path)
           .arg("-framerate")
           .arg(quality_config.fps.to_string())
           .arg("-video_size")
           .arg(&quality_config.resolution)
           .arg("-b:v")
           .arg(quality_config.bitrate.to_string());

        if self.config.audio.enabled {
            cmd.arg("-f")
               .arg("alsa")
               .arg("-i");
            
            // Use configured device path or default
            if let Some(ref device_path) = self.config.audio.device_path {
                cmd.arg(device_path);
            } else {
                cmd.arg("default"); // Default ALSA device
            }
            
            cmd.arg("-c:a")
               .arg("aac")
               .arg("-b:a")
               .arg(format!("{}", self.config.audio.bitrate));
        }

        cmd.arg("-c:v")
           .arg(&quality_config.codec)
           .arg("-preset")
           .arg("ultrafast")
           .arg("-t")
           .arg(duration_arg)
           .arg("-f")
           .arg("mp4")
           .arg(file_path);

        let child = cmd.spawn()
            .context("Failed to start ffmpeg recording process")?;

        self.recording_processes.insert(quality_config.quality.clone(), child);
        Ok(())
    }

    async fn start_simulated_recording(
        &mut self, 
        quality_config: &crate::config::VideoQualityConfig, 
        file_path: &PathBuf
    ) -> Result<()> {
        println!("Starting simulated recording to: {}", file_path.display());
        
        // Create a dummy file for simulation
        let dummy_content = format!("Simulated recording data\nDevice: {}\nIncident: {}\nQuality: {:?}\nStart: {}", 
            self.device_id, 
            self.incident_id,
            quality_config.quality,
            Utc::now().to_rfc3339()
        );
        
        fs::write(file_path, dummy_content).await?;
        
        // Simulate recording duration
        let duration = self.duration.unwrap_or(300); // Default 5 minutes
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        Ok(())
    }

    pub async fn start_high_quality_upload(
        &self,
        incident_id: &str,
        quality: VideoQuality,
    ) -> Result<()> {
        // Find the segment for the requested quality
        for (_, segment) in &self.current_segments {
            if segment.incident_id == incident_id && segment.quality == quality {
                return self.upload_segment(segment).await;
            }
        }
        
        // If not found in current segments, check archived segments
        let archived_path = self.get_storage_path().await?;
        let pattern = format!("{}_{}_*_{}.mp4", self.device_id, incident_id, 
            match quality {
                VideoQuality::Low => "low",
                VideoQuality::Medium => "med",
                VideoQuality::High => "high",
                VideoQuality::Ultra => "ultra",
            }
        );
        
        // In a real implementation, we'd search the archived files
        // For now, return not found
        Err(anyhow::anyhow!("Segment not found for quality {:?}", quality))
    }

    async fn get_storage_path(&self) -> Result<PathBuf> {
        let storage_path = std::env::current_dir()?
            .join("recordings")
            .join(Utc::now().format("%Y-%m-%d").to_string());
        
        fs::create_dir_all(&storage_path).await?;
        Ok(storage_path)
    }

    async fn save_segment_metadata(&self, segment: &RecordingSegment) -> Result<()> {
        let metadata_path = std::env::current_dir()?
            .join("recordings")
            .join("metadata")
            .join(format!("{}.json", segment.id));
        
        fs::create_dir_all(metadata_path.parent().unwrap()).await?;
        
        let metadata_json = serde_json::to_string_pretty(segment)?;
        fs::write(metadata_path, metadata_json).await?;
        
        Ok(())
    }

    pub async fn verify_segment_integrity(
        &self,
        segment: &RecordingSegment,
    ) -> Result<IntegrityVerification> {
        if let Some(integrity) = &segment.integrity {
            let path = PathBuf::from(&segment.file_path);
            IntegrityManager::verify_file_integrity(&path, &integrity.sha256_hash).await
        } else {
            Err(anyhow::anyhow!("No integrity record for segment"))
        }
    }

    async fn create_integrity_record(
        &mut self,
        segment: &mut RecordingSegment,
    ) -> Result<()> {
        let path = PathBuf::from(&segment.file_path);
        
        if !path.exists() {
            return Err(anyhow::anyhow!("Segment file not found: {}", segment.file_path));
        }
        
        let metadata = serde_json::to_value(&segment.metadata)?;
        let integrity = IntegrityManager::create_integrity_record(&path, &metadata).await?;
        
        segment.integrity = Some(integrity);
        Ok(())
    }

    async fn upload_segment(&self, segment: &RecordingSegment) -> Result<()> {
        println!("Uploading segment {}...", segment.id);
        
        // Simulate upload delay based on file size
        if let Some(file_size) = segment.file_size {
            let upload_time = file_size / self.config.network.upload_bandwidth as u64;
            tokio::time::sleep(tokio::time::Duration::from_secs(upload_time)).await;
        }
        
        println!("Segment {} uploaded successfully", segment.id);
        
        // Auto-delete file after successful upload
        let file_path = PathBuf::from(&segment.file_path);
        if file_path.exists() {
            match fs::remove_file(&file_path).await {
                Ok(_) => {
                    println!("Successfully deleted uploaded file: {}", segment.file_path);
                    
                    // Also delete associated metadata file
                    let metadata_path = std::env::current_dir()?
                        .join("recordings")
                        .join("metadata")
                        .join(format!("{}.json", segment.id));
                    
                    if metadata_path.exists() {
                        let _ = fs::remove_file(&metadata_path).await;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to delete uploaded file {}: {}", segment.file_path, e);
                }
            }
        }
        
        Ok(())
    }

    pub fn is_recording(&self) -> bool {
        !self.recording_processes.is_empty()
    }

    pub fn get_current_segments(&self) -> &HashMap<VideoQuality, RecordingSegment> {
        &self.current_segments
    }

    pub async fn decrypt_recording(&self, segment: &RecordingSegment, output_path: &PathBuf) -> Result<()> {
        if let Some(encryptor) = &self.encryptor {
            let encrypted_path = PathBuf::from(&segment.file_path);
            encryptor.decrypt_video_file(&encrypted_path, output_path).await?;
            tracing::info!("Successfully decrypted recording segment: {}", segment.id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("No encryption configured - cannot decrypt recording"))
        }
    }

    pub fn is_encryption_enabled(&self) -> bool {
        self.encryptor.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageBreakdown {
    pub quality: String,
    pub video_count: usize,
    pub total_size_mb: u64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFileInfo {
    pub path: String,
    pub size_bytes: u64,
    pub quality: String,
    pub duration_seconds: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub incident_id: Option<String>,
}

pub async fn analyze_storage_usage(media_dir: &Path) -> Result<Vec<StorageBreakdown>> {
    if !media_dir.exists() {
        return Ok(vec![]);
    }

    let mut files = vec![];
    let mut reader = tokio::fs::read_dir(media_dir).await?;

    while let Some(entry) = reader.next_entry().await? {
        let path = entry.path();
        if path.is_file() {
            let metadata = entry.metadata().await?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            
            // Parse quality from filename
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

            files.push((quality.to_string(), metadata.len()));
        }
    }

    // Group by quality
    let mut breakdown = std::collections::HashMap::new();
    for (quality, size) in files {
        let entry = breakdown.entry(quality).or_insert((0, 0u64));
        entry.0 += 1;
        entry.1 += size;
    }

    // Calculate percentages
    let total_size: u64 = breakdown.values().map(|(_, size)| *size).sum();
    let total_mb = total_size / (1024 * 1024);

    let mut result: Vec<StorageBreakdown> = breakdown
        .into_iter()
        .map(|(quality, (count, size))| {
            let size_mb = size / (1024 * 1024);
            let percentage = if total_mb > 0 {
                (size_mb as f64 / total_mb as f64) * 100.0
            } else {
                0.0
            };

            StorageBreakdown {
                quality,
                video_count: count,
                total_size_mb: size_mb,
                percentage,
            }
        })
        .collect();

    // Sort by quality priority
    result.sort_by(|a, b| {
        let priority = |q: &str| match q {
            "Ultra" => 4,
            "High" => 3,
            "Medium" => 2,
            "Low" => 1,
            _ => 0,
        };
        priority(&b.quality).cmp(&priority(&a.quality))
    });

    Ok(result)
}

pub async fn get_media_files(media_dir: &Path) -> Result<Vec<MediaFileInfo>> {
    if !media_dir.exists() {
        return Ok(vec![]);
    }

    let mut files = vec![];
    let mut reader = tokio::fs::read_dir(media_dir).await?;

    while let Some(entry) = reader.next_entry().await? {
        let path = entry.path();
        if path.is_file() {
            let metadata = entry.metadata().await?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            
            // Parse metadata from filename
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

            let incident_id = if file_name.contains("_incident_") {
                file_name.split("_incident_")
                    .nth(1)
                    .and_then(|s| s.split('_').next())
                    .map(|s| s.to_string())
            } else {
                None
            };

            let created_at = chrono::DateTime::from(metadata.modified()?);

            files.push(MediaFileInfo {
                path: path.to_string_lossy().to_string(),
                size_bytes: metadata.len(),
                quality: quality.to_string(),
                duration_seconds: 0, // TODO: Parse from metadata
                created_at,
                incident_id,
            });
        }
    }

    Ok(files)
}