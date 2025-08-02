use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::collections::HashMap;

use crate::config::Config;
use crate::device::{DeviceStatus, DiagnosticsReport};
use crate::media::RecordingSegment;
use crate::integrity::{VideoIntegrity, IntegrityVerification};

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceRegistrationRequest {
    pub device_name: String,
    pub site_id: String,
    pub device_type: String,
    pub hardware_info: HardwareInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub camera_resolution: String,
    pub storage_capacity: u64,
    pub battery_capacity: u32,
    pub os_version: String,
    pub firmware_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceRegistrationResponse {
    pub device_id: String,
    pub device_key: String,
    pub site_id: String,
    pub tenant_id: String,
    pub server_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaUploadRequest {
    pub segment_id: String,
    pub incident_id: String,
    pub quality: String,
    pub file_size: u64,
    pub duration: u64,
    pub checksum: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaUploadResponse {
    pub upload_url: String,
    pub upload_id: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StreamingStartRequest {
    pub incident_id: Option<String>,
    pub quality: String,
    pub include_audio: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StreamingStartResponse {
    pub stream_id: String,
    pub rtmp_url: String,
    pub stream_key: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceMetrics {
    pub device_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub cpu_usage: f32,
    pub memory_usage: f32,
    pub storage_usage: f32,
    pub battery_level: f32,
    pub temperature: f32,
    pub network_quality: String,
    pub active_incidents: u32,
}

pub struct ApiClient {
    config: Config,
    client: Client,
    base_url: String,
}

impl ApiClient {
    pub fn new(config: Config) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.network.timeout))
            .https_only(true)
            .danger_accept_invalid_certs(false)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            client,
            base_url: String::new(),
        }
    }

    fn get_auth_headers(&self) -> Result<reqwest::header::HeaderMap> {
        let mut headers = reqwest::header::HeaderMap::new();
        
        if let Some(token) = &self.config.auth_token {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token))
                    .context("Invalid auth token")?
            );
        }
        
        if let Some(api_key) = &self.config.api_key {
            headers.insert(
                "X-API-Key",
                reqwest::header::HeaderValue::from_str(api_key)
                    .context("Invalid API key")?
            );
        }
        
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json")
        );
        
        Ok(headers)
    }

    async fn make_request_with_retry<F, Fut, T>(
        &self,
        make_request: F,
        max_retries: u32,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut retries = 0;
        let mut last_error = None;

        while retries <= max_retries {
            match make_request().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                    retries += 1;
                    
                    if retries <= max_retries {
                        let delay = std::time::Duration::from_millis(500 * retries as u64);
                        tokio::time::sleep(delay).await;
                        tracing::warn!("Request failed, retrying {}/{}: {}", retries, max_retries, last_error.as_ref().unwrap());
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Request failed after retries")))
    }

    // Device Management Endpoints
    pub async fn register_device(
        &self,
        device_name: &str,
        site_id: &str,
        hardware_info: HardwareInfo,
    ) -> Result<DeviceRegistrationResponse> {
        let url = format!("{}/api/devices/register", self.config.server_url);
        
        let request = DeviceRegistrationRequest {
            device_name: device_name.to_string(),
            site_id: site_id.to_string(),
            device_type: "bodycam".to_string(),
            hardware_info,
        };

        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .json(&request)
                .send()
                .await
                .context("Failed to register device")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Device registration failed: {}", error_text));
        }

        let registration_response = response.json().await?;
        Ok(registration_response)
    }

    pub async fn update_device_status(
        &self,
        device_id: &str,
        status: &DeviceStatus,
    ) -> Result<()> {
        let url = format!("{}/api/devices/{}/status", self.config.server_url, device_id);
        
        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .json(status)
                .send()
                .await
                .context("Failed to update device status")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Device status update failed: {}", error_text));
        }

        Ok(())
    }

    pub async fn report_diagnostics(
        &self,
        diagnostics: &DiagnosticsReport,
    ) -> Result<()> {
        let url = format!("{}/api/devices/{}/diagnostics", self.config.server_url, diagnostics.device_id);
        
        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .json(diagnostics)
                .send()
                .await
                .context("Failed to report diagnostics")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Diagnostics report failed: {}", error_text));
        }

        Ok(())
    }

    // Media Management Endpoints
    pub async fn request_upload_url(
        &self,
        segment: &RecordingSegment,
    ) -> Result<MediaUploadResponse> {
        let url = format!("{}/api/media/upload-request", self.config.server_url);
        
        let checksum = if let Some(integrity) = &segment.integrity {
            integrity.sha256_hash.clone()
        } else {
            return Err(anyhow::anyhow!("No integrity record for segment"));
        };

        let request = MediaUploadRequest {
            segment_id: segment.id.clone(),
            incident_id: segment.incident_id.clone(),
            quality: format!("{:?}", segment.quality).to_lowercase(),
            file_size: segment.file_size.unwrap_or(0),
            duration: segment.duration.unwrap_or(0),
            checksum,
            metadata: serde_json::to_value(&segment.metadata)?,
        };

        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .json(&request)
                .send()
                .await
                .context("Failed to request upload URL")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Upload request failed: {}", error_text));
        }

        let upload_response = response.json().await?;
        Ok(upload_response)
    }

    pub async fn upload_segment(
        &self,
        segment: &RecordingSegment,
        upload_url: &str,
    ) -> Result<()> {
        let file_path = PathBuf::from(&segment.file_path);
        if !file_path.exists() {
            return Err(anyhow::anyhow!("Segment file not found: {}", segment.file_path));
        }

        let file_data = tokio::fs::read(&file_path).await
            .context("Failed to read segment file")?;

        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .put(upload_url)
                .headers(headers.clone())
                .body(file_data.clone())
                .send()
                .await
                .context("Failed to upload segment")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Segment upload failed: {}", error_text));
        }

        Ok(())
    }

    pub async fn confirm_upload(
        &self,
        segment_id: &str,
    ) -> Result<()> {
        let url = format!("{}/api/media/{}/confirm", self.config.server_url, segment_id);
        
        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .send()
                .await
                .context("Failed to confirm upload")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Upload confirmation failed: {}", error_text));
        }

        Ok(())
    }

    // Streaming Endpoints
    pub async fn start_streaming(
        &self,
        incident_id: Option<String>,
        quality: &str,
        include_audio: bool,
    ) -> Result<StreamingStartResponse> {
        let url = format!("{}/api/streaming/start", self.config.server_url);
        
        let request = StreamingStartRequest {
            incident_id,
            quality: quality.to_string(),
            include_audio,
        };

        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .json(&request)
                .send()
                .await
                .context("Failed to start streaming")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Streaming start failed: {}", error_text));
        }

        let streaming_response = response.json().await?;
        Ok(streaming_response)
    }

    pub async fn stop_streaming(
        &self,
        stream_id: &str,
    ) -> Result<()> {
        let url = format!("{}/api/streaming/{}/stop", self.config.server_url, stream_id);
        
        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .send()
                .await
                .context("Failed to stop streaming")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Streaming stop failed: {}", error_text));
        }

        Ok(())
    }

    // Metrics Endpoints
    pub async fn send_metrics(
        &self,
        metrics: &DeviceMetrics,
    ) -> Result<()> {
        let url = format!("{}/api/devices/{}/metrics", self.config.server_url, metrics.device_id);
        
        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .json(metrics)
                .send()
                .await
                .context("Failed to send metrics")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Metrics send failed: {}", error_text));
        }

        Ok(())
    }

    pub async fn get_device_config(
        &self,
        device_id: &str,
    ) -> Result<Config> {
        let url = format!("{}/api/devices/{}/config", self.config.server_url, device_id);
        
        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .get(&url)
                .headers(headers.clone())
                .send()
                .await
                .context("Failed to get device config")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Config fetch failed: {}", error_text));
        }

        let config = response.json().await?;
        Ok(config)
    }
}