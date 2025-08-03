use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::config::Config;
use crate::validation::InputValidator;
use crate::api::ApiClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    pub quality: String,
    pub include_audio: bool,
    pub bitrate: u32,
    pub fps: u32,
    pub resolution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub stream_id: String,
    pub rtmp_url: String,
    pub stream_key: String,
    pub status: StreamStatus,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub config: StreamingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamStatus {
    Starting,
    Active,
    Stopping,
    Stopped,
    Error(String),
}

pub struct StreamingManager {
    config: Config,
    api_client: ApiClient,
    current_stream: Option<StreamInfo>,
    ffmpeg_process: Option<Child>,
    event_tx: Option<mpsc::UnboundedSender<StreamEvent>>,
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    StreamStarted { stream_id: String },
    StreamStopped { stream_id: String },
    StreamError { stream_id: String, error: String },
    BitrateChanged { bitrate: u32 },
}

impl StreamingManager {
    pub fn new(config: Config) -> Self {
        let api_client = ApiClient::new(config.clone());
        
        Self {
            config,
            api_client,
            current_stream: None,
            ffmpeg_process: None,
            event_tx: None,
        }
    }

    pub async fn start_streaming(
        &mut self,
        incident_id: Option<String>,
        quality: &str,
        include_audio: bool,
    ) -> Result<StreamInfo> {
        // Validate inputs
        if let Some(ref incident_id) = incident_id {
            InputValidator::validate_uuid(incident_id)?;
        }
        
        if self.is_streaming() {
            return Err(anyhow::anyhow!("Already streaming"));
        }

        if !self.config.is_provisioned() {
            return Err(anyhow::anyhow!("Device not provisioned"));
        }

        // Get streaming configuration from quality setting
        let streaming_config = self.get_streaming_config(quality, include_audio)?;

        // Request streaming URL from server
        let streaming_response = self.api_client
            .start_streaming(incident_id.clone(), quality, include_audio)
            .await
            .context("Failed to start streaming session")?;

        let stream_info = StreamInfo {
            stream_id: streaming_response.stream_id.clone(),
            rtmp_url: streaming_response.rtmp_url,
            stream_key: streaming_response.stream_key,
            status: StreamStatus::Starting,
            started_at: chrono::Utc::now(),
            config: streaming_config.clone(),
        };

        // Start FFmpeg process for streaming
        self.start_ffmpeg_stream(&stream_info).await?;
        
        // Update status to active
        let mut active_stream = stream_info.clone();
        active_stream.status = StreamStatus::Active;
        self.current_stream = Some(active_stream.clone());

        // Emit stream started event
        if let Some(ref event_tx) = self.event_tx {
            let _ = event_tx.send(StreamEvent::StreamStarted {
                stream_id: streaming_response.stream_id,
            });
        }

        tracing::info!("Live streaming started: {}", active_stream.stream_id);
        Ok(active_stream)
    }

    pub async fn stop_streaming(&mut self) -> Result<()> {
        if !self.is_streaming() {
            return Err(anyhow::anyhow!("Not currently streaming"));
        }

        let stream_id = self.current_stream
            .as_ref()
            .map(|s| s.stream_id.clone())
            .unwrap_or_default();

        // Stop FFmpeg process
        if let Some(mut process) = self.ffmpeg_process.take() {
            tracing::info!("Stopping FFmpeg streaming process");
            let _ = process.kill().await;
        }

        // Notify server that streaming has stopped
        if let Err(e) = self.api_client.stop_streaming(&stream_id).await {
            tracing::warn!("Failed to notify server of streaming stop: {}", e);
        }

        // Update stream status
        if let Some(ref mut stream) = self.current_stream {
            stream.status = StreamStatus::Stopped;
        }

        // Emit stream stopped event
        if let Some(ref event_tx) = self.event_tx {
            let _ = event_tx.send(StreamEvent::StreamStopped { stream_id: stream_id.clone() });
        }

        self.current_stream = None;
        tracing::info!("Live streaming stopped: {}", stream_id);
        Ok(())
    }

    pub fn is_streaming(&self) -> bool {
        self.current_stream.is_some() && 
        matches!(
            self.current_stream.as_ref().map(|s| &s.status),
            Some(StreamStatus::Active) | Some(StreamStatus::Starting)
        )
    }

    pub fn get_current_stream(&self) -> Option<&StreamInfo> {
        self.current_stream.as_ref()
    }

    pub fn set_event_channel(&mut self, tx: mpsc::UnboundedSender<StreamEvent>) {
        self.event_tx = Some(tx);
    }

    async fn start_ffmpeg_stream(&mut self, stream_info: &StreamInfo) -> Result<()> {
        let rtmp_url = format!("{}/{}", stream_info.rtmp_url, stream_info.stream_key);
        
        let mut cmd = Command::new("ffmpeg");
        
        // Input source
        if self.config.simulation.enabled {
            // Use test sources for simulation
            cmd.arg("-f").arg("lavfi")
               .arg("-i").arg(format!("testsrc2=size={}:rate={}", 
                   stream_info.config.resolution,
                   stream_info.config.fps
               ));
            
            if stream_info.config.include_audio {
                cmd.arg("-f").arg("lavfi")
                   .arg("-i").arg("sine=frequency=1000:sample_rate=44100");
            }
        } else {
            // Use real camera input
            cmd.arg("-f").arg("v4l2")
               .arg("-i").arg("/dev/video0")
               .arg("-framerate").arg(stream_info.config.fps.to_string())
               .arg("-video_size").arg(&stream_info.config.resolution);
            
            if stream_info.config.include_audio {
                cmd.arg("-f").arg("alsa")
                   .arg("-i").arg("hw:0,0");
            }
        }

        // Video encoding settings
        cmd.arg("-c:v").arg("libx264")
           .arg("-preset").arg("ultrafast")
           .arg("-tune").arg("zerolatency")
           .arg("-b:v").arg(format!("{}k", stream_info.config.bitrate / 1000))
           .arg("-maxrate").arg(format!("{}k", stream_info.config.bitrate / 1000))
           .arg("-bufsize").arg(format!("{}k", stream_info.config.bitrate / 500))
           .arg("-g").arg((stream_info.config.fps * 2).to_string()) // Keyframe interval
           .arg("-r").arg(stream_info.config.fps.to_string());

        // Audio encoding settings
        if stream_info.config.include_audio {
            cmd.arg("-c:a").arg("aac")
               .arg("-b:a").arg("128k")
               .arg("-ar").arg("44100");
        } else {
            cmd.arg("-an"); // No audio
        }

        // RTMP output settings
        cmd.arg("-f").arg("flv")
           .arg("-flvflags").arg("no_duration_filesize")
           .arg(&rtmp_url);

        // Logging
        cmd.arg("-loglevel").arg("warning");

        // Redirect streams
        cmd.stdout(Stdio::piped())
           .stderr(Stdio::piped());

        let child = cmd.spawn()
            .context("Failed to start FFmpeg streaming process")?;

        self.ffmpeg_process = Some(child);
        
        tracing::info!("FFmpeg streaming process started for stream: {}", stream_info.stream_id);
        Ok(())
    }

    fn get_streaming_config(&self, quality: &str, include_audio: bool) -> Result<StreamingConfig> {
        let (resolution, bitrate, fps) = match quality {
            "low" => ("640x480", 500_000, 15),
            "medium" => ("1280x720", 1_500_000, 30),
            "high" => ("1920x1080", 3_000_000, 30),
            "ultra" => ("1920x1080", 5_000_000, 60),
            _ => return Err(anyhow::anyhow!("Invalid quality setting: {}", quality)),
        };

        Ok(StreamingConfig {
            quality: quality.to_string(),
            include_audio,
            bitrate,
            fps,
            resolution: resolution.to_string(),
        })
    }

    pub async fn update_bitrate(&mut self, new_bitrate: u32) -> Result<()> {
        if !self.is_streaming() {
            return Err(anyhow::anyhow!("Not currently streaming"));
        }

        // For adaptive bitrate, we would need to restart the stream
        // This is a simplified implementation
        tracing::info!("Bitrate update requested: {} kbps", new_bitrate / 1000);
        
        if let Some(ref event_tx) = self.event_tx {
            let _ = event_tx.send(StreamEvent::BitrateChanged { bitrate: new_bitrate });
        }

        Ok(())
    }

    pub async fn get_stream_stats(&self) -> Result<StreamStats> {
        if let Some(ref stream) = self.current_stream {
            let uptime = chrono::Utc::now() - stream.started_at;
            
            Ok(StreamStats {
                stream_id: stream.stream_id.clone(),
                uptime_seconds: uptime.num_seconds() as u64,
                current_bitrate: stream.config.bitrate,
                fps: stream.config.fps,
                resolution: stream.config.resolution.clone(),
                status: stream.status.clone(),
            })
        } else {
            Err(anyhow::anyhow!("No active stream"))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStats {
    pub stream_id: String,
    pub uptime_seconds: u64,
    pub current_bitrate: u32,
    pub fps: u32,
    pub resolution: String,
    pub status: StreamStatus,
}

impl Drop for StreamingManager {
    fn drop(&mut self) {
        if let Some(mut process) = self.ffmpeg_process.take() {
            let _ = futures::executor::block_on(process.kill());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_config_creation() {
        let config = Config::default();
        let manager = StreamingManager::new(config);
        
        let streaming_config = manager.get_streaming_config("high", true).unwrap();
        assert_eq!(streaming_config.quality, "high");
        assert_eq!(streaming_config.resolution, "1920x1080");
        assert_eq!(streaming_config.fps, 30);
        assert!(streaming_config.include_audio);
    }

    #[test]
    fn test_invalid_quality() {
        let config = Config::default();
        let manager = StreamingManager::new(config);
        
        let result = manager.get_streaming_config("invalid", true);
        assert!(result.is_err());
    }
}