use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use sha2::{Sha256, Digest};
use hmac::{Hmac, Mac};
use base64::{Engine as _, engine::general_purpose};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;

use crate::config::Config;

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceCredentials {
    pub device_id: String,
    pub device_key: String,
    pub site_id: String,
    pub tenant_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProvisionRequest {
    pub device_name: String,
    pub site_id: String,
    pub hardware_info: HardwareInfo,
    pub public_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub model: String,
    pub serial_number: String,
    pub firmware_version: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProvisionResponse {
    pub device_id: String,
    pub device_key: String,
    pub tenant_id: String,
    pub api_endpoint: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthToken {
    pub token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

type HmacSha256 = Hmac<Sha256>;

pub struct Authenticator {
    config: Config,
    http_client: reqwest::Client,
}

impl Authenticator {
    pub fn new(config: Config) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            http_client,
        }
    }

    pub async fn provision_device(&self, device_name: &str, site_id: &str) -> Result<DeviceCredentials> {
        let hardware_info = self.get_hardware_info();
        let keypair = self.generate_keypair();
        
        let request = ProvisionRequest {
            device_name: device_name.to_string(),
            site_id: site_id.to_string(),
            hardware_info,
            public_key: keypair.public_key,
        };

        let response = self.http_client
            .post(format!("{}/api/devices/provision", self.config.server_url))
            .json(&request)
            .send()
            .await
            .context("Failed to send provision request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Provision failed: {}", error_text));
        }

        let provision_response: ProvisionResponse = response
            .json()
            .await
            .context("Failed to parse provision response")?;

        Ok(DeviceCredentials {
            device_id: provision_response.device_id,
            device_key: provision_response.device_key,
            site_id: site_id.to_string(),
            tenant_id: provision_response.tenant_id,
        })
    }

    pub async fn authenticate(&self, device_id: &str, device_key: &str) -> Result<AuthToken> {
        let timestamp = chrono::Utc::now().timestamp();
        let nonce = Uuid::new_v4().to_string();
        
        let message = format!("{}:{}:{}", device_id, timestamp, nonce);
        let signature = self.sign_message(&message, device_key);

        let auth_request = serde_json::json!({
            "device_id": device_id,
            "timestamp": timestamp,
            "nonce": nonce,
            "signature": signature,
        });

        let response = self.http_client
            .post(format!("{}/api/devices/auth", self.config.server_url))
            .json(&auth_request)
            .send()
            .await
            .context("Failed to send auth request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Authentication failed: {}", error_text));
        }

        let token: AuthToken = response
            .json()
            .await
            .context("Failed to parse auth response")?;

        Ok(token)
    }

    pub fn sign_request(&self, device_id: &str, device_key: &str, payload: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(device_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(payload);
        let result = mac.finalize();
        general_purpose::STANDARD.encode(result.into_bytes())
    }

    fn get_hardware_info(&self) -> HardwareInfo {
        let capabilities = vec![
            "video_recording".to_string(),
            "audio_recording".to_string(),
            "gps_tracking".to_string(),
            "accelerometer".to_string(),
            "live_streaming".to_string(),
        ];

        HardwareInfo {
            model: "PatrolSight BodyCam Pro".to_string(),
            serial_number: Uuid::new_v4().to_string(),
            firmware_version: env!("CARGO_PKG_VERSION").to_string(),
            capabilities,
        }
    }

    fn generate_keypair(&self) -> KeyPair {
        let mut csprng = OsRng {};
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key: VerifyingKey = (&signing_key).into();
        
        let public_key = general_purpose::STANDARD.encode(verifying_key.as_bytes());
        
        KeyPair { 
            public_key,
            private_key: signing_key,
        }
    }

    fn sign_message(&self, message: &str, key: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(message.as_bytes());
        let result = mac.finalize();
        general_purpose::STANDARD.encode(result.into_bytes())
    }
}

struct KeyPair {
    public_key: String,
    private_key: SigningKey,
}