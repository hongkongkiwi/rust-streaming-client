use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::Utc;
use reqwest::Client;

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub id: String,
    pub device_id: String,
    pub incident_type: String,
    pub severity: IncidentSeverity,
    pub status: IncidentStatus,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub location: Option<LocationData>,
    pub description: String,
    pub metadata: serde_json::Value,
    pub video_segments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationData {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
    pub accuracy: Option<f64>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IncidentSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IncidentStatus {
    Active,
    Resolved,
    Escalated,
    FalseAlarm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentCreateRequest {
    pub device_id: String,
    pub incident_type: String,
    pub severity: String,
    pub description: String,
    pub location: Option<LocationData>,
    pub metadata: serde_json::Value,
}

pub struct IncidentManager {
    config: Config,
    client: Client,
}

impl IncidentManager {
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
        }
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

    pub async fn create_incident(
        &self,
        incident_id: &str,
        incident_type: &str,
        severity: &str,
        device_id: &str,
    ) -> Result<()> {
        self.create_incident_with_location(incident_id, incident_type, severity, device_id, None).await
    }

    pub async fn create_incident_with_location(
        &self,
        incident_id: &str,
        incident_type: &str,
        severity: &str,
        device_id: &str,
        location: Option<LocationData>,
    ) -> Result<()> {
        if !self.config.is_provisioned() {
            return Err(anyhow::anyhow!("Device not provisioned"));
        }

        let incident = IncidentCreateRequest {
            device_id: device_id.to_string(),
            incident_type: incident_type.to_string(),
            severity: severity.to_string(),
            description: format!("Automatic incident triggered by bodycam"),
            location,
            metadata: serde_json::json!({
                "trigger_type": "automatic",
                "device_model": "PatrolSight BodyCam Pro",
            }),
        };

        let url = format!("{}/api/incidents", self.config.server_url);
        
        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .json(&incident)
                .send()
                .await
                .context("Failed to create incident")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Incident creation failed: {}", error_text));
        }

        println!("Incident {} created successfully", incident_id);
        Ok(())
    }

    pub async fn update_incident(
        &self,
        incident_id: &str,
        status: IncidentStatus,
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        let url = format!("{}/api/incidents/{}", self.config.server_url, incident_id);
        
        let update_data = serde_json::json!({
            "status": status,
            "metadata": metadata,
            "updated_at": Utc::now().to_rfc3339(),
        });

        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .patch(&url)
                .headers(headers.clone())
                .json(&update_data)
                .send()
                .await
                .context("Failed to update incident")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Incident update failed: {}", error_text));
        }

        Ok(())
    }

    pub async fn add_video_segment(
        &self,
        incident_id: &str,
        segment_id: &str,
        quality: &str,
        duration: u64,
    ) -> Result<()> {
        let url = format!("{}/api/incidents/{}/segments", self.config.server_url, incident_id);
        
        let segment_data = serde_json::json!({
            "segment_id": segment_id,
            "quality": quality,
            "duration": duration,
            "uploaded": false,
        });

        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .json(&segment_data)
                .send()
                .await
                .context("Failed to add video segment")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Adding segment failed: {}", error_text));
        }

        Ok(())
    }

    pub async fn request_high_quality_upload(
        &self,
        incident_id: &str,
        quality: &str,
    ) -> Result<()> {
        let url = format!("{}/api/incidents/{}/request-upload", self.config.server_url, incident_id);
        
        let request_data = serde_json::json!({
            "quality": quality,
            "device_id": self.config.device_id,
        });

        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .post(&url)
                .headers(headers.clone())
                .json(&request_data)
                .send()
                .await
                .context("Failed to request high quality upload")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("High quality upload request failed: {}", error_text));
        }

        Ok(())
    }

    pub async fn get_incident(&self, incident_id: &str) -> Result<Incident> {
        let url = format!("{}/api/incidents/{}", self.config.server_url, incident_id);
        
        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .get(&url)
                .headers(headers.clone())
                .send()
                .await
                .context("Failed to get incident")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Getting incident failed: {}", error_text));
        }

        let incident = response.json().await?;
        Ok(incident)
    }

    pub async fn list_incidents(&self, device_id: &str) -> Result<Vec<Incident>> {
        let url = format!("{}/api/devices/{}/incidents", self.config.server_url, device_id);
        
        let headers = self.get_auth_headers()?;
        let response = self.make_request_with_retry(|| async {
            self.client
                .get(&url)
                .headers(headers.clone())
                .send()
                .await
                .context("Failed to list incidents")
        }, self.config.network.retry_attempts).await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Listing incidents failed: {}", error_text));
        }

        let incidents = response.json().await?;
        Ok(incidents)
    }
}