use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::collections::HashMap;

use crate::config::Config;
use crate::device::{DeviceStatus, DiagnosticsReport};
use crate::media::RecordingSegment;
use crate::integrity::{VideoIntegrity, IntegrityVerification};
use std::path::PathBuf;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct SendSmsRequest {
    pub to: String,
    pub text: String,
    pub device_id: Option<String>,
    pub incident_id: Option<String>,
    pub priority: Option<String>, // "low", "medium", "high", "urgent"
    pub emergency: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendSmsResponse {
    pub success: bool,
    pub sms_id: String,
    pub message_uuid: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MakeCallRequest {
    pub to: String,
    pub device_id: Option<String>,
    pub incident_id: Option<String>,
    pub priority: Option<String>, // "low", "medium", "high", "urgent"
    pub emergency: Option<bool>,
    pub recording: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MakeCallResponse {
    pub success: bool,
    pub call_id: String,
    pub call_uuid: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommunicationContact {
    pub id: String,
    pub name: String,
    pub phone_number: String,
    pub email: Option<String>,
    pub contact_type: String, // "emergency", "dispatch", "supervisor", "support", "general"
    pub can_receive_sms: bool,
    pub can_receive_calls: bool,
    pub site_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SmsMessage {
    pub id: String,
    pub to: String,
    pub from: String,
    pub text: String,
    pub status: String,
    pub direction: String,
    pub sent_at: Option<i64>,
    pub delivered_at: Option<i64>,
    pub incident_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VoiceCall {
    pub id: String,
    pub to: String,
    pub from: String,
    pub status: String,
    pub direction: String,
    pub duration: Option<u64>,
    pub initiated_at: i64,
    pub answered_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub incident_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

// New structures for Plivo number management
#[derive(Debug, Serialize, Deserialize)]
pub struct PlivoNumber {
    pub id: String,
    pub phone_number: String,
    pub number_type: String, // "local", "toll-free", "mobile"
    pub country: String,
    pub region: Option<String>,
    pub is_allocated: bool,
    pub allocated_to_device_id: Option<String>,
    pub allocated_at: Option<i64>,
    pub sms_enabled: bool,
    pub voice_enabled: bool,
    pub whitelist_mode: String, // "disabled", "enabled"
    pub monthly_cost: Option<f64>,
    pub setup_cost: Option<f64>,
    pub currency: Option<String>,
    pub is_active: bool,
    pub notes: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceCommunicationCapabilities {
    pub id: String,
    pub device_id: String,
    pub plivo_number_id: Option<String>,
    pub sms_enabled: bool,
    pub voice_enabled: bool,
    pub daily_sms_limit: Option<u32>,
    pub daily_call_limit: Option<u32>,
    pub monthly_usage_limit: Option<f64>,
    pub current_month_sms: u32,
    pub current_month_calls: u32,
    pub current_month_cost: f64,
    pub last_reset_at: i64,
    pub emergency_bypass_limits: bool,
    pub emergency_contacts_only: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NumberWhitelistEntry {
    pub id: String,
    pub allowed_number: String,
    pub number_type: String, // "contact", "emergency", "custom"
    pub sms_allowed: bool,
    pub voice_allowed: bool,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddPlivoNumberRequest {
    pub phone_number: String,
    pub number_type: String,
    pub country: String,
    pub region: Option<String>,
    pub plivo_number_id: String,
    pub plivo_auth_id: String,
    pub plivo_auth_token: String,
    pub sms_enabled: bool,
    pub voice_enabled: bool,
    pub whitelist_mode: String,
    pub monthly_cost: Option<f64>,
    pub setup_cost: Option<f64>,
    pub currency: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AllocateNumberRequest {
    pub plivo_number_id: String,
    pub device_id: String,
    pub sms_enabled: bool,
    pub voice_enabled: bool,
    pub daily_sms_limit: Option<u32>,
    pub daily_call_limit: Option<u32>,
    pub monthly_usage_limit: Option<f64>,
    pub emergency_bypass_limits: Option<bool>,
    pub emergency_contacts_only: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddToWhitelistRequest {
    pub plivo_number_id: String,
    pub allowed_number: String,
    pub number_type: String,
    pub sms_allowed: bool,
    pub voice_allowed: bool,
    pub contact_id: Option<String>,
    pub description: Option<String>,
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

    // Communication Endpoints
    pub async fn send_sms(
        &self,
        to: &str,
        text: &str,
        device_id: Option<&str>,
        incident_id: Option<&str>,
        priority: Option<&str>,
        emergency: Option<bool>,
    ) -> Result<SendSmsResponse> {
        let url = format!("{}/api/communications/sms/send", self.config.server_url);
        
        let request = SendSmsRequest {
            to: to.to_string(),
            text: text.to_string(),
            device_id: device_id.map(|s| s.to_string()),
            incident_id: incident_id.map(|s| s.to_string()),
            priority: priority.map(|s| s.to_string()),
            emergency,
        };

        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .json(&request)
                .send()
                .await
                .context("Failed to send SMS")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("SMS send failed: {}", error_text));
        }

        let sms_response = response.json().await?;
        Ok(sms_response)
    }

    pub async fn make_call(
        &self,
        to: &str,
        device_id: Option<&str>,
        incident_id: Option<&str>,
        priority: Option<&str>,
        emergency: Option<bool>,
        recording: Option<bool>,
    ) -> Result<MakeCallResponse> {
        let url = format!("{}/api/communications/call/make", self.config.server_url);
        
        let request = MakeCallRequest {
            to: to.to_string(),
            device_id: device_id.map(|s| s.to_string()),
            incident_id: incident_id.map(|s| s.to_string()),
            priority: priority.map(|s| s.to_string()),
            emergency,
            recording,
        };

        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .json(&request)
                .send()
                .await
                .context("Failed to make call")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Call failed: {}", error_text));
        }

        let call_response = response.json().await?;
        Ok(call_response)
    }

    pub async fn get_sms_history(
        &self,
        device_id: Option<&str>,
        incident_id: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<SmsMessage>> {
        let mut url = format!("{}/api/communications/sms/history", self.config.server_url);
        let mut params = Vec::new();

        if let Some(device_id) = device_id {
            params.push(format!("device_id={}", device_id));
        }
        if let Some(incident_id) = incident_id {
            params.push(format!("incident_id={}", incident_id));
        }
        if let Some(limit) = limit {
            params.push(format!("limit={}", limit));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .get(&url)
                .headers(headers.clone())
                .send()
                .await
                .context("Failed to get SMS history")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("SMS history fetch failed: {}", error_text));
        }

        let sms_history = response.json().await?;
        Ok(sms_history)
    }

    pub async fn get_call_history(
        &self,
        device_id: Option<&str>,
        incident_id: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<VoiceCall>> {
        let mut url = format!("{}/api/communications/call/history", self.config.server_url);
        let mut params = Vec::new();

        if let Some(device_id) = device_id {
            params.push(format!("device_id={}", device_id));
        }
        if let Some(incident_id) = incident_id {
            params.push(format!("incident_id={}", incident_id));
        }
        if let Some(limit) = limit {
            params.push(format!("limit={}", limit));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .get(&url)
                .headers(headers.clone())
                .send()
                .await
                .context("Failed to get call history")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Call history fetch failed: {}", error_text));
        }

        let call_history = response.json().await?;
        Ok(call_history)
    }

    pub async fn get_contacts(
        &self,
        contact_type: Option<&str>,
        site_id: Option<&str>,
    ) -> Result<Vec<CommunicationContact>> {
        let mut url = format!("{}/api/communications/contacts", self.config.server_url);
        let mut params = Vec::new();

        if let Some(contact_type) = contact_type {
            params.push(format!("type={}", contact_type));
        }
        if let Some(site_id) = site_id {
            params.push(format!("site_id={}", site_id));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .get(&url)
                .headers(headers.clone())
                .send()
                .await
                .context("Failed to get contacts")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Contacts fetch failed: {}", error_text));
        }

        let contacts = response.json().await?;
        Ok(contacts)
    }

    // Convenience methods for emergency communications
    pub async fn send_emergency_sms(
        &self,
        to: &str,
        message: &str,
        device_id: Option<&str>,
        incident_id: Option<&str>,
    ) -> Result<SendSmsResponse> {
        self.send_sms(to, message, device_id, incident_id, Some("urgent"), Some(true)).await
    }

    pub async fn make_emergency_call(
        &self,
        to: &str,
        device_id: Option<&str>,
        incident_id: Option<&str>,
    ) -> Result<MakeCallResponse> {
        self.make_call(to, device_id, incident_id, Some("urgent"), Some(true), Some(true)).await
    }

    // Batch communication methods
    pub async fn send_broadcast_sms(
        &self,
        contacts: &[CommunicationContact],
        message: &str,
        device_id: Option<&str>,
        incident_id: Option<&str>,
        priority: Option<&str>,
    ) -> Result<Vec<Result<SendSmsResponse>>> {
        let mut results = Vec::new();

        for contact in contacts {
            if !contact.can_receive_sms {
                results.push(Err(anyhow::anyhow!("Contact {} cannot receive SMS", contact.name)));
                continue;
            }

            let result = self.send_sms(
                &contact.phone_number,
                message,
                device_id,
                incident_id,
                priority,
                priority == Some("urgent"),
            ).await;
            
            results.push(result);
        }

        Ok(results)
    }

    // Plivo Number Management Methods (Admin Only)
    pub async fn add_plivo_number(
        &self,
        number_data: AddPlivoNumberRequest,
    ) -> Result<String> {
        let url = format!("{}/api/plivo-management/add-number", self.config.server_url);
        
        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .json(&number_data)
                .send()
                .await
                .context("Failed to add Plivo number")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Add Plivo number failed: {}", error_text));
        }

        let result: serde_json::Value = response.json().await?;
        Ok(result.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string())
    }

    pub async fn allocate_number_to_device(
        &self,
        allocation_data: AllocateNumberRequest,
    ) -> Result<()> {
        let url = format!("{}/api/plivo-management/allocate-number", self.config.server_url);
        
        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .json(&allocation_data)
                .send()
                .await
                .context("Failed to allocate number to device")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Number allocation failed: {}", error_text));
        }

        Ok(())
    }

    pub async fn unallocate_number_from_device(
        &self,
        plivo_number_id: &str,
    ) -> Result<()> {
        let url = format!("{}/api/plivo-management/unallocate-number", self.config.server_url);
        
        let payload = serde_json::json!({
            "plivo_number_id": plivo_number_id
        });

        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .json(&payload)
                .send()
                .await
                .context("Failed to unallocate number from device")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Number unallocation failed: {}", error_text));
        }

        Ok(())
    }

    pub async fn get_tenant_plivo_numbers(
        &self,
        include_allocated: Option<bool>,
        include_unallocated: Option<bool>,
    ) -> Result<Vec<PlivoNumber>> {
        let mut url = format!("{}/api/plivo-management/numbers", self.config.server_url);
        let mut params = Vec::new();

        if let Some(allocated) = include_allocated {
            params.push(format!("include_allocated={}", allocated));
        }
        if let Some(unallocated) = include_unallocated {
            params.push(format!("include_unallocated={}", unallocated));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .get(&url)
                .headers(headers.clone())
                .send()
                .await
                .context("Failed to get tenant Plivo numbers")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Get Plivo numbers failed: {}", error_text));
        }

        let numbers = response.json().await?;
        Ok(numbers)
    }

    pub async fn get_device_allocated_number(
        &self,
        device_id: &str,
    ) -> Result<Option<(PlivoNumber, DeviceCommunicationCapabilities)>> {
        let url = format!("{}/api/plivo-management/device/{}/number", self.config.server_url, device_id);
        
        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .get(&url)
                .headers(headers.clone())
                .send()
                .await
                .context("Failed to get device allocated number")
        }, self.config.network.retry_attempts).await?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Get device number failed: {}", error_text));
        }

        let result: serde_json::Value = response.json().await?;
        if result.is_null() {
            return Ok(None);
        }

        let plivo_number: PlivoNumber = serde_json::from_value(result["plivoNumber"].clone())?;
        let capability: DeviceCommunicationCapabilities = serde_json::from_value(result["capability"].clone())?;
        
        Ok(Some((plivo_number, capability)))
    }

    pub async fn get_device_communication_capabilities(
        &self,
        device_id: &str,
    ) -> Result<Option<DeviceCommunicationCapabilities>> {
        let url = format!("{}/api/plivo-management/device/{}/capabilities", self.config.server_url, device_id);
        
        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .get(&url)
                .headers(headers.clone())
                .send()
                .await
                .context("Failed to get device communication capabilities")
        }, self.config.network.retry_attempts).await?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Get device capabilities failed: {}", error_text));
        }

        let capabilities = response.json().await?;
        Ok(Some(capabilities))
    }

    pub async fn update_device_communication_capabilities(
        &self,
        device_id: &str,
        sms_enabled: Option<bool>,
        voice_enabled: Option<bool>,
        daily_sms_limit: Option<u32>,
        daily_call_limit: Option<u32>,
        monthly_usage_limit: Option<f64>,
        emergency_bypass_limits: Option<bool>,
        emergency_contacts_only: Option<bool>,
    ) -> Result<()> {
        let url = format!("{}/api/plivo-management/device/{}/capabilities", self.config.server_url, device_id);
        
        let mut payload = serde_json::Map::new();
        if let Some(sms) = sms_enabled {
            payload.insert("sms_enabled".to_string(), serde_json::Value::Bool(sms));
        }
        if let Some(voice) = voice_enabled {
            payload.insert("voice_enabled".to_string(), serde_json::Value::Bool(voice));
        }
        if let Some(sms_limit) = daily_sms_limit {
            payload.insert("daily_sms_limit".to_string(), serde_json::Value::Number(sms_limit.into()));
        }
        if let Some(call_limit) = daily_call_limit {
            payload.insert("daily_call_limit".to_string(), serde_json::Value::Number(call_limit.into()));
        }
        if let Some(usage_limit) = monthly_usage_limit {
            payload.insert("monthly_usage_limit".to_string(), serde_json::json!(usage_limit));
        }
        if let Some(bypass) = emergency_bypass_limits {
            payload.insert("emergency_bypass_limits".to_string(), serde_json::Value::Bool(bypass));
        }
        if let Some(contacts_only) = emergency_contacts_only {
            payload.insert("emergency_contacts_only".to_string(), serde_json::Value::Bool(contacts_only));
        }

        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .patch(&url)
                .headers(headers.clone())
                .json(&payload)
                .send()
                .await
                .context("Failed to update device communication capabilities")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Update device capabilities failed: {}", error_text));
        }

        Ok(())
    }

    // Whitelist Management
    pub async fn add_to_whitelist(
        &self,
        whitelist_data: AddToWhitelistRequest,
    ) -> Result<String> {
        let url = format!("{}/api/plivo-management/whitelist/add", self.config.server_url);
        
        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .json(&whitelist_data)
                .send()
                .await
                .context("Failed to add to whitelist")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Add to whitelist failed: {}", error_text));
        }

        let result: serde_json::Value = response.json().await?;
        Ok(result.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string())
    }

    pub async fn remove_from_whitelist(
        &self,
        whitelist_id: &str,
    ) -> Result<()> {
        let url = format!("{}/api/plivo-management/whitelist/{}", self.config.server_url, whitelist_id);
        
        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .delete(&url)
                .headers(headers.clone())
                .send()
                .await
                .context("Failed to remove from whitelist")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Remove from whitelist failed: {}", error_text));
        }

        Ok(())
    }

    pub async fn get_number_whitelist(
        &self,
        plivo_number_id: &str,
    ) -> Result<Vec<NumberWhitelistEntry>> {
        let url = format!("{}/api/plivo-management/whitelist/{}", self.config.server_url, plivo_number_id);
        
        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .get(&url)
                .headers(headers.clone())
                .send()
                .await
                .context("Failed to get number whitelist")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Get whitelist failed: {}", error_text));
        }

        let whitelist = response.json().await?;
        Ok(whitelist)
    }

    // Convenience methods for checking device capabilities
    pub async fn can_device_send_sms(&self, device_id: &str) -> Result<bool> {
        match self.get_device_communication_capabilities(device_id).await? {
            Some(capabilities) => Ok(capabilities.sms_enabled && capabilities.plivo_number_id.is_some()),
            None => Ok(false),
        }
    }

    pub async fn can_device_make_calls(&self, device_id: &str) -> Result<bool> {
        match self.get_device_communication_capabilities(device_id).await? {
            Some(capabilities) => Ok(capabilities.voice_enabled && capabilities.plivo_number_id.is_some()),
            None => Ok(false),
        }
    }

    pub async fn get_device_usage_stats(&self, device_id: &str) -> Result<Option<(u32, u32, f64)>> {
        match self.get_device_communication_capabilities(device_id).await? {
            Some(capabilities) => Ok(Some((
                capabilities.current_month_sms,
                capabilities.current_month_calls,
                capabilities.current_month_cost,
            ))),
            None => Ok(None),
        }
    }
}