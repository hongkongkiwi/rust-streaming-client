use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use base64::{Engine as _, engine::general_purpose};

use crate::config::Config;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConvexDeviceStatus {
    pub device_id: String,
    pub tenant_id: String,
    
    // Location tracking (enhanced)
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub location_accuracy: Option<f64>,
    pub location_timestamp: Option<u64>,
    
    // Power management (enhanced)
    pub battery_level: Option<f64>,
    pub is_charging: Option<bool>,
    pub power_source: Option<String>, // "battery" | "charging" | "external"
    
    // Connectivity (new)
    pub signal_strength: Option<i32>,
    pub connection_type: Option<String>, // "wifi" | "lte" | "offline"
    pub wifi_ssid: Option<String>,
    
    // Storage (enhanced)
    pub storage_used: Option<u64>,
    pub storage_available: Option<u64>,
    
    // Recording status (enhanced)
    pub recording_status: Option<String>, // "idle" | "recording" | "uploading" | "processing"
    pub pending_uploads: Option<u32>,
    
    // Health monitoring (new)
    pub temperature: Option<f64>,
    pub uptime: Option<u64>,
    pub memory_usage: Option<u64>,
    
    // Error reporting (new)
    pub errors: Option<Vec<String>>,
    pub warnings: Option<Vec<String>>,
    
    pub timestamp: u64, // Unix timestamp
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceCredentials {
    pub device_id: String,
    pub device_key: String,
    pub site_id: String,
    pub tenant_id: String,
    pub auth_token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoCreateRequest {
    pub device_id: String,
    pub filename: String,
    pub duration: Option<u64>,
    pub quality: String,
    pub incident_id: Option<String>,
    pub metadata: VideoMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoMetadata {
    pub codec: String,
    pub bitrate: u32,
    pub resolution: String,
    pub is_encrypted: bool,
    pub encryption_algorithm: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IncidentCreateRequest {
    pub device_id: String,
    pub incident_type: String,
    pub button_type: String, // "single" | "double" | "long" | "triple"
    pub gps_latitude: Option<f64>,
    pub gps_longitude: Option<f64>,
    pub gps_accuracy: Option<f64>,
    pub metadata: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceSettings {
    pub video_quality: String,
    pub video_bitrate: u32,
    pub audio_enabled: bool,
    pub button_actions: HashMap<String, String>,
    pub sos_settings: SOSSettings,
    pub wifi_networks: Vec<WiFiNetwork>,
    pub power_management: PowerManagementSettings,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SOSSettings {
    pub enabled: bool,
    pub emergency_contacts: Vec<String>,
    pub auto_call_timeout: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WiFiNetwork {
    pub ssid: String,
    pub password: String,
    pub priority: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PowerManagementSettings {
    pub low_power_mode: bool,
    pub auto_shutdown_timeout: u32,
    pub brightness_level: u32,
}

pub struct ConvexApiClient {
    convex_client: convex::ConvexClient,
    device_id: Option<String>,
    tenant_id: Option<String>,
    auth_token: Option<String>,
    config: Config,
}

impl ConvexApiClient {
    pub async fn new(convex_url: &str, config: Config) -> Result<Self> {
        let convex_client = convex::ConvexClient::new(convex_url).await
            .context("Failed to create Convex client")?;
            
        Ok(Self {
            convex_client,
            device_id: config.device_id.clone(),
            tenant_id: config.tenant_id.clone(),
            auth_token: config.auth_token.clone(),
            config,
        })
    }

    pub async fn check_version_and_provision(
        &self,
        app_type: &str,
        current_version: &str,
        device_serial: &str,
        factory_secret: &str,
        client_info: &HashMap<String, String>,
    ) -> Result<DeviceCredentials> {
        let args = json!({
            "appType": app_type,
            "currentVersion": current_version,
            "deviceSerial": device_serial,
            "factorySecret": factory_secret,
            "clientInfo": client_info
        });

        let result = self.convex_client
            .query("checkVersion", args)
            .await
            .context("Failed to check version with Convex")?;

        // Parse the response to extract device credentials
        let device_id = result["deviceId"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing deviceId in response"))?
            .to_string();
        
        let device_key = result["deviceKey"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing deviceKey in response"))?
            .to_string();
            
        let site_id = result["siteId"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing siteId in response"))?
            .to_string();
            
        let tenant_id = result["tenantId"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing tenantId in response"))?
            .to_string();
            
        let auth_token = result["authToken"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing authToken in response"))?
            .to_string();

        Ok(DeviceCredentials {
            device_id,
            device_key,
            site_id,
            tenant_id,
            auth_token,
        })
    }

    pub async fn record_device_status(&self, status: &ConvexDeviceStatus) -> Result<()> {
        let args = json!({
            "deviceId": status.device_id,
            "tenantId": status.tenant_id,
            "latitude": status.latitude,
            "longitude": status.longitude,
            "locationAccuracy": status.location_accuracy,
            "locationTimestamp": status.location_timestamp,
            "batteryLevel": status.battery_level,
            "isCharging": status.is_charging,
            "powerSource": status.power_source,
            "signalStrength": status.signal_strength,
            "connectionType": status.connection_type,
            "wifiSsid": status.wifi_ssid,
            "storageUsed": status.storage_used,
            "storageAvailable": status.storage_available,
            "recordingStatus": status.recording_status,
            "pendingUploads": status.pending_uploads,
            "temperature": status.temperature,
            "uptime": status.uptime,
            "memoryUsage": status.memory_usage,
            "errors": status.errors,
            "warnings": status.warnings,
            "timestamp": status.timestamp
        });

        self.convex_client
            .mutation("recordDeviceStatus", args)
            .await
            .context("Failed to record device status")?;

        Ok(())
    }

    pub async fn create_video(&self, video_request: &VideoCreateRequest) -> Result<String> {
        let args = json!({
            "deviceId": video_request.device_id,
            "filename": video_request.filename,
            "duration": video_request.duration,
            "quality": video_request.quality,
            "incidentId": video_request.incident_id,
            "metadata": {
                "codec": video_request.metadata.codec,
                "bitrate": video_request.metadata.bitrate,
                "resolution": video_request.metadata.resolution,
                "isEncrypted": video_request.metadata.is_encrypted,
                "encryptionAlgorithm": video_request.metadata.encryption_algorithm
            }
        });

        let result = self.convex_client
            .mutation("createVideo", args)
            .await
            .context("Failed to create video record")?;

        let video_id = result["videoId"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing videoId in response"))?
            .to_string();

        Ok(video_id)
    }

    pub async fn upload_video_chunk(
        &self,
        video_id: &str,
        chunk_index: usize,
        chunk_data: &[u8],
        is_last_chunk: bool,
    ) -> Result<()> {
        let chunk_base64 = general_purpose::STANDARD.encode(chunk_data);
        
        let args = json!({
            "videoId": video_id,
            "chunkIndex": chunk_index,
            "chunkData": chunk_base64,
            "isLastChunk": is_last_chunk
        });

        self.convex_client
            .mutation("uploadVideoChunk", args)
            .await
            .context("Failed to upload video chunk")?;

        Ok(())
    }

    pub async fn complete_video_upload(&self, video_id: &str) -> Result<()> {
        let args = json!({
            "videoId": video_id
        });

        self.convex_client
            .mutation("completeVideoUpload", args)
            .await
            .context("Failed to complete video upload")?;

        Ok(())
    }

    pub async fn create_incident(&self, incident_request: &IncidentCreateRequest) -> Result<String> {
        let args = json!({
            "deviceId": incident_request.device_id,
            "incidentType": incident_request.incident_type,
            "buttonType": incident_request.button_type,
            "gpsLatitude": incident_request.gps_latitude,
            "gpsLongitude": incident_request.gps_longitude,
            "gpsAccuracy": incident_request.gps_accuracy,
            "metadata": incident_request.metadata
        });

        let result = self.convex_client
            .mutation("createIncident", args)
            .await
            .context("Failed to create incident")?;

        let incident_id = result["incidentId"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing incidentId in response"))?
            .to_string();

        Ok(incident_id)
    }

    pub async fn get_device_settings(&self, device_id: &str) -> Result<DeviceSettings> {
        let args = json!({
            "deviceId": device_id
        });

        let result = self.convex_client
            .query("getDeviceSettings", args)
            .await
            .context("Failed to get device settings")?;

        // Parse the complex nested settings structure
        let video_quality = result["videoQuality"].as_str()
            .unwrap_or("medium").to_string();
        let video_bitrate = result["videoBitrate"].as_u64()
            .unwrap_or(2000) as u32;
        let audio_enabled = result["audioEnabled"].as_bool()
            .unwrap_or(true);

        // Parse button actions
        let mut button_actions = HashMap::new();
        if let Some(actions) = result["buttonActions"].as_object() {
            for (key, value) in actions {
                if let Some(action_str) = value.as_str() {
                    button_actions.insert(key.clone(), action_str.to_string());
                }
            }
        }

        // Parse SOS settings
        let sos_settings = if let Some(sos) = result["sosSettings"].as_object() {
            SOSSettings {
                enabled: sos["enabled"].as_bool().unwrap_or(false),
                emergency_contacts: sos["emergencyContacts"]
                    .as_array()
                    .map(|arr| arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect())
                    .unwrap_or_default(),
                auto_call_timeout: sos["autoCallTimeout"].as_u64().unwrap_or(30) as u32,
            }
        } else {
            SOSSettings {
                enabled: false,
                emergency_contacts: vec![],
                auto_call_timeout: 30,
            }
        };

        // Parse WiFi networks
        let wifi_networks = if let Some(networks) = result["wifiNetworks"].as_array() {
            networks.iter()
                .filter_map(|network| {
                    if let Some(obj) = network.as_object() {
                        Some(WiFiNetwork {
                            ssid: obj["ssid"].as_str()?.to_string(),
                            password: obj["password"].as_str()?.to_string(),
                            priority: obj["priority"].as_u64().unwrap_or(0) as u32,
                        })
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            vec![]
        };

        // Parse power management
        let power_management = if let Some(pm) = result["powerManagement"].as_object() {
            PowerManagementSettings {
                low_power_mode: pm["lowPowerMode"].as_bool().unwrap_or(false),
                auto_shutdown_timeout: pm["autoShutdownTimeout"].as_u64().unwrap_or(3600) as u32,
                brightness_level: pm["brightnessLevel"].as_u64().unwrap_or(80) as u32,
            }
        } else {
            PowerManagementSettings {
                low_power_mode: false,
                auto_shutdown_timeout: 3600,
                brightness_level: 80,
            }
        };

        Ok(DeviceSettings {
            video_quality,
            video_bitrate,
            audio_enabled,
            button_actions,
            sos_settings,
            wifi_networks,
            power_management,
        })
    }

    // Chunked video upload helper method
    pub async fn upload_video_file(&self, file_path: &str, video_request: &VideoCreateRequest) -> Result<String> {
        // 1. Create video record
        let video_id = self.create_video(video_request).await?;

        // 2. Read file and upload in chunks
        let file_data = tokio::fs::read(file_path).await
            .context("Failed to read video file")?;

        const CHUNK_SIZE: usize = 1024 * 1024; // 1MB chunks
        let total_chunks = (file_data.len() + CHUNK_SIZE - 1) / CHUNK_SIZE;

        for (index, chunk) in file_data.chunks(CHUNK_SIZE).enumerate() {
            let is_last_chunk = index == total_chunks - 1;
            
            self.upload_video_chunk(&video_id, index, chunk, is_last_chunk).await
                .with_context(|| format!("Failed to upload chunk {}/{}", index + 1, total_chunks))?;
            
            tracing::info!("Uploaded chunk {}/{} for video {}", index + 1, total_chunks, video_id);
        }

        // 3. Complete upload
        self.complete_video_upload(&video_id).await?;

        tracing::info!("Successfully uploaded video file {} as {}", file_path, video_id);
        Ok(video_id)
    }

    // New chunked upload methods for the upload manager
    pub async fn start_chunked_upload(
        &self,
        filename: &str,
        file_size: u64,
        chunk_size: u64,
        metadata: serde_json::Value,
        incident_id: Option<String>,
    ) -> Result<String> {
        let args = json!({
            "filename": filename,
            "fileSize": file_size,
            "chunkSize": chunk_size,
            "metadata": metadata,
            "incidentId": incident_id
        });

        let result = self.convex_client
            .mutation("startChunkedUpload", args)
            .await
            .context("Failed to start chunked upload")?;

        let upload_session_id = result["uploadSessionId"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing uploadSessionId in response"))?
            .to_string();

        Ok(upload_session_id)
    }

    pub async fn upload_chunk(
        &self,
        upload_session_id: &str,
        chunk_index: u32,
        chunk_data: &[u8],
        md5_hash: &str,
    ) -> Result<()> {
        let chunk_base64 = general_purpose::STANDARD.encode(chunk_data);
        
        let args = json!({
            "uploadSessionId": upload_session_id,
            "chunkIndex": chunk_index,
            "chunkData": chunk_base64,
            "md5Hash": md5_hash
        });

        self.convex_client
            .mutation("uploadChunk", args)
            .await
            .context("Failed to upload chunk")?;

        Ok(())
    }

    pub async fn complete_chunked_upload(&self, upload_session_id: &str) -> Result<String> {
        let args = json!({
            "uploadSessionId": upload_session_id
        });

        let result = self.convex_client
            .mutation("completeChunkedUpload", args)
            .await
            .context("Failed to complete chunked upload")?;

        let video_id = result["videoId"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing videoId in response"))?
            .to_string();

        Ok(video_id)
    }

    pub async fn get_upload_session_status(&self, upload_session_id: &str) -> Result<serde_json::Value> {
        let args = json!({
            "uploadSessionId": upload_session_id
        });

        let result = self.convex_client
            .query("getUploadSessionStatus", args)
            .await
            .context("Failed to get upload session status")?;

        Ok(result)
    }

    pub fn set_auth_token(&mut self, token: String) {
        self.auth_token = Some(token.clone());
        // Auth token will be used in future API calls
        tracing::info!("Auth token updated for Convex client");
    }

    pub fn set_device_id(&mut self, device_id: String) {
        self.device_id = Some(device_id);
    }

    pub fn set_tenant_id(&mut self, tenant_id: String) {
        self.tenant_id = Some(tenant_id);
    }
}

// Helper function to convert from legacy DeviceStatus to ConvexDeviceStatus
impl From<crate::device::DeviceStatus> for ConvexDeviceStatus {
    fn from(status: crate::device::DeviceStatus) -> Self {
        ConvexDeviceStatus {
            device_id: status.device_id,
            tenant_id: "unknown".to_string(), // Will be set from config
            latitude: status.location.as_ref().map(|loc| loc.latitude),
            longitude: status.location.as_ref().map(|loc| loc.longitude),
            location_accuracy: status.location.as_ref().and_then(|loc| loc.accuracy),
            location_timestamp: Some(status.last_seen.timestamp() as u64),
            battery_level: Some(status.battery_level as f64),
            is_charging: Some(status.is_charging),
            power_source: Some(if status.is_charging { "charging".to_string() } else { "battery".to_string() }),
            signal_strength: None, // Not available in legacy status
            connection_type: Some("wifi".to_string()), // Default assumption
            wifi_ssid: None,
            storage_used: Some(status.storage_info.used),
            storage_available: Some(status.storage_info.available),
            recording_status: Some(if status.recording { "recording".to_string() } else { "idle".to_string() }),
            pending_uploads: None, // Not tracked in legacy status
            temperature: Some(status.temperature as f64),
            uptime: None, // Not tracked in legacy status
            memory_usage: None, // Not tracked in legacy status
            errors: None,
            warnings: None,
            timestamp: status.last_seen.timestamp() as u64,
        }
    }
}