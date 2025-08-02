use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::config::{Config, VideoQuality};
use crate::integrity::{IntegrityManager, VideoIntegrity};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferSegment {
    pub id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration: u64,
    pub file_path: String,
    pub file_size: Option<u64>,
    pub quality: VideoQuality,
    pub metadata: BufferMetadata,
    pub integrity: Option<VideoIntegrity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferMetadata {
    pub resolution: String,
    pub fps: u32,
    pub bitrate: u32,
    pub codec: String,
    pub audio_enabled: bool,
    pub location: Option<LocationData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationData {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
    pub timestamp: DateTime<Utc>,
}

pub struct CircularBuffer {
    config: Config,
    device_id: String,
    buffer_duration: u64,
    segments: Arc<Mutex<VecDeque<BufferSegment>>>,
    recording_processes: Arc<Mutex<Vec<(VideoQuality, tokio::process::Child)>>>,
    active: Arc<Mutex<bool>>,
    cleanup_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    last_cleanup: Arc<Mutex<DateTime<Utc>>>,
}

impl CircularBuffer {
    pub fn new(config: Config, device_id: String) -> Self {
        let buffer_duration = config.recording.pre_incident_buffer_seconds;
        Self {
            config,
            device_id,
            buffer_duration,
            segments: Arc::new(Mutex::new(VecDeque::new())),
            recording_processes: Arc::new(Mutex::new(Vec::new())),
            active: Arc::new(Mutex::new(false)),
            cleanup_task: Arc::new(Mutex::new(None)),
            last_cleanup: Arc::new(Mutex::new(Utc::now())),
        }
    }

    pub async fn start_buffering(&self) -> Result<()> {
        let mut active = self.active.lock().await;
        if *active {
            return Ok(()); // Already running
        }
        *active = true;
        drop(active);

        let config = self.config.clone();
        let device_id = self.device_id.clone();
        let segments = self.segments.clone();
        let recording_processes = self.recording_processes.clone();
        let active = self.active.clone();
        let cleanup_task = self.cleanup_task.clone();

        // Start cleanup task
        let cleanup_segments = segments.clone();
        let cleanup_config = config.clone();
        let cleanup_handle = tokio::spawn(async move {
            let mut cleanup_interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            
            loop {
                cleanup_interval.tick().await;
                
                let is_active = *active.lock().await;
                if !is_active {
                    break;
                }

                if let Err(e) = Self::cleanup_old_segments(
                    cleanup_config.clone(),
                    cleanup_segments.clone(),
                ).await {
                    tracing::error!("Failed to cleanup old segments: {}", e);
                }
            }
        });

        *cleanup_task.lock().await = Some(cleanup_handle);

        // Start recording task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                let is_active = *active.lock().await;
                if !is_active {
                    break;
                }

                if let Err(e) = Self::record_buffer_segment(
                    config.clone(),
                    device_id.clone(),
                    segments.clone(),
                    recording_processes.clone(),
                ).await {
                    tracing::error!("Failed to record buffer segment: {}", e);
                }
            }
        });

        Ok(())
    }

    pub async fn stop_buffering(&self) -> Result<()> {
        let mut active = self.active.lock().await;
        *active = false;
        
        let mut processes = self.recording_processes.lock().await;
        for (_, mut process) in processes.drain(..) {
            let _ = process.kill().await;
        }
        
        // Stop cleanup task
        if let Some(handle) = self.cleanup_task.lock().await.take() {
            handle.abort();
        }
        
        // Cleanup all remaining segments
        self.cleanup_all_segments().await?;
        
        Ok(())
    }

    async fn cleanup_old_segments(
        config: Config,
        segments: Arc<Mutex<VecDeque<BufferSegment>>>,
    ) -> Result<()> {
        let max_age = chrono::Duration::seconds(config.recording.pre_incident_buffer_seconds as i64 * 2);
        let cutoff_time = Utc::now() - max_age;
        
        let mut segments_lock = segments.lock().await;
        let mut removed_segments = Vec::new();
        
        segments_lock.retain(|segment| {
            if segment.start_time < cutoff_time {
                removed_segments.push(segment.file_path.clone());
                false
            } else {
                true
            }
        });
        
        // Clean up files asynchronously
        for file_path in removed_segments {
            tokio::spawn(async move {
                let _ = tokio::fs::remove_file(file_path).await;
            });
        }
        
        Ok(())
    }

    async fn cleanup_all_segments(&self) -> Result<()> {
        let segments = self.segments.lock().await;
        let file_paths: Vec<String> = segments.iter().map(|s| s.file_path.clone()).collect();
        drop(segments);
        
        for file_path in file_paths {
            tokio::spawn(async move {
                let _ = tokio::fs::remove_file(file_path).await;
            });
        }
        
        Ok(())
    }

    pub async fn get_buffer_segments(&self, duration: u64) -> Result<Vec<BufferSegment>> {
        let segments = self.segments.lock().await;
        let mut result = Vec::new();
        
        let target_duration = duration.min(self.buffer_duration);
        let mut accumulated_duration = 0;
        
        for segment in segments.iter().rev() {
            if accumulated_duration + segment.duration <= target_duration {
                result.push(segment.clone());
                accumulated_duration += segment.duration;
            } else {
                break;
            }
        }
        
        result.reverse();
        Ok(result)
    }

    pub async fn clear_buffer(&self) -> Result<()> {
        let mut segments = self.segments.lock().await;
        segments.clear();
        Ok(())
    }

    async fn record_buffer_segment(
        config: Config,
        device_id: String,
        segments: Arc<Mutex<VecDeque<BufferSegment>>>,
        recording_processes: Arc<Mutex<Vec<(VideoQuality, tokio::process::Child)>>>,
    ) -> Result<()> {
        let segment_duration = 5; // 5-second segments
        let segment_id = Uuid::new_v4().to_string();
        let start_time = Utc::now();
        
        for quality_config in &config.recording.available_qualities {
            let storage_path = Self::get_buffer_storage_path().await?;
            let file_name = format!("buffer_{}_{}_{}.mp4", device_id, segment_id, 
                match quality_config.quality {
                    VideoQuality::Low => "low",
                    VideoQuality::Medium => "med",
                    VideoQuality::High => "high",
                    VideoQuality::Ultra => "ultra",
                });
            let file_path = storage_path.join(file_name);

            let metadata = BufferMetadata {
                resolution: quality_config.resolution.clone(),
                fps: quality_config.fps,
                bitrate: quality_config.bitrate,
                codec: quality_config.codec.clone(),
                audio_enabled: config.audio.enabled,
                location: None, // TODO: Add GPS location
            };

            let segment = BufferSegment {
                id: segment_id.clone(),
                start_time,
                end_time: start_time + chrono::Duration::seconds(segment_duration as i64),
                duration: segment_duration,
                file_path: file_path.to_string_lossy().to_string(),
                file_size: None,
                quality: quality_config.quality.clone(),
                metadata,
            };

            // Start recording for this quality
            if !config.simulation.enabled {
                Self::start_buffer_recording(&quality_config, &file_path, segment_duration).await?;
            }

            let mut segments_lock = segments.lock().await;
            segments_lock.push_back(segment);
            
            // Maintain buffer size
            let max_segments = (config.recording.pre_incident_buffer_seconds / segment_duration) as usize;
            while segments_lock.len() > max_segments {
                if let Some(old_segment) = segments_lock.pop_front() {
                    // Clean up old file
                    let _ = tokio::fs::remove_file(old_segment.file_path).await;
                }
            }
        }

        Ok(())
    }

    async fn start_buffer_recording(
        quality_config: &crate::config::VideoQualityConfig,
        file_path: &PathBuf,
        duration: u64,
    ) -> Result<()> {
        let mut cmd = tokio::process::Command::new("ffmpeg");
        
        cmd.arg("-f")
           .arg("v4l2")
           .arg("-i")
           .arg(&quality_config.device_path)
           .arg("-framerate")
           .arg(quality_config.fps.to_string())
           .arg("-video_size")
           .arg(&quality_config.resolution)
           .arg("-b:v")
           .arg(quality_config.bitrate.to_string())
           .arg("-c:v")
           .arg(&quality_config.codec)
           .arg("-preset")
           .arg("ultrafast")
           .arg("-t")
           .arg(duration.to_string())
           .arg("-f")
           .arg("mp4")
           .arg(file_path);

        let child = cmd.spawn()
            .context("Failed to start ffmpeg buffer recording")?;

        // In a real implementation, we'd store the process to manage it
        // For now, we'll let it complete
        
        Ok(())
    }

    async fn get_buffer_storage_path() -> Result<PathBuf> {
        let storage_path = std::env::current_dir()?
            .join("buffer")
            .join(Utc::now().format("%Y-%m-%d").to_string());
        
        tokio::fs::create_dir_all(&storage_path).await?;
        Ok(storage_path)
    }
}