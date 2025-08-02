use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use reqwest::Client;

use crate::config::Config;
use crate::device::DeviceStatus;

pub struct StatusReporter {
    config: Config,
    client: Client,
}

impl StatusReporter {
    pub fn new(config: Config) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            client,
        }
    }

    pub async fn report_status(&self, status: DeviceStatus) -> Result<()> {
        if !self.config.is_provisioned() {
            return Ok(());
        }

        let url = format!("{}/api/devices/status", self.config.server_url);
        
        let response = self.client
            .post(url)
            .json(&status)
            .send()
            .await
            .context("Failed to send status update")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Status update failed: {}", error_text));
        }

        Ok(())
    }

    pub async fn send_heartbeat(&self, device_id: &str) -> Result<()> {
        let url = format!("{}/api/devices/heartbeat", self.config.server_url);
        
        let heartbeat = serde_json::json!({
            "device_id": device_id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "uptime": self.get_uptime(),
        });

        let response = self.client
            .post(url)
            .json(&heartbeat)
            .send()
            .await
            .context("Failed to send heartbeat")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Heartbeat failed: {}", error_text));
        }

        Ok(())
    }

    fn get_uptime(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}