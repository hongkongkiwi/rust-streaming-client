use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use base64::{Engine as _, engine::general_purpose};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::convex_api::{ConvexApiClient, DeviceCredentials};

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientInfo {
    pub platform: String,
    pub os_version: String,
    pub build_number: String,
    pub device_model: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FactoryProvisioningData {
    pub device_serial: String,
    pub factory_secret: String,
    pub client_info: ClientInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthSession {
    pub token: String,
    pub expires_at: u64,
    pub refresh_token: String,
    pub user_id: String,
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub tenant_id: String,
    pub site_id: String,
    pub user_id: String,
}

pub struct ConvexAuthenticator {
    config: Config,
    convex_client: Option<Arc<RwLock<ConvexApiClient>>>,
    auth_session: Option<AuthSession>,
    device_identity: Option<DeviceIdentity>,
    signing_key: Option<SigningKey>,
}

impl ConvexAuthenticator {
    pub fn new(config: Config) -> Result<Self> {
        Ok(Self {
            config,
            convex_client: None,
            auth_session: None,
            device_identity: None,
            signing_key: None,
        })
    }

    pub async fn initialize_convex_client(&mut self) -> Result<()> {
        let convex_url = self.config.convex_url.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Convex URL not configured"))?;
            
        let client = ConvexApiClient::new(convex_url, self.config.clone()).await?;
        self.convex_client = Some(Arc::new(RwLock::new(client)));
        Ok(())
    }

    pub async fn factory_provision(&mut self, device_name: &str, site_id: &str) -> Result<DeviceCredentials> {
        if self.convex_client.is_none() {
            self.initialize_convex_client().await?;
        }

        let convex_client = self.convex_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Convex client not initialized"))?;

        // Generate device keypair for identity
        let (public_key, signing_key) = self.generate_device_keypair()?;
        self.signing_key = Some(signing_key);

        // Get factory provisioning data
        let factory_data = self.get_factory_provisioning_data()?;
        
        // Prepare client info for version check
        let mut client_info = HashMap::new();
        client_info.insert("platform".to_string(), factory_data.client_info.platform.clone());
        client_info.insert("osVersion".to_string(), factory_data.client_info.os_version.clone());
        client_info.insert("buildNumber".to_string(), factory_data.client_info.build_number.clone());
        client_info.insert("deviceModel".to_string(), factory_data.client_info.device_model.clone());
        client_info.insert("capabilities".to_string(), factory_data.client_info.capabilities.join(","));
        client_info.insert("deviceName".to_string(), device_name.to_string());
        client_info.insert("siteId".to_string(), site_id.to_string());
        client_info.insert("publicKey".to_string(), public_key.clone());

        // Use checkVersion for factory provisioning (device-specific, NOT better-auth)
        let credentials = {
            let client = convex_client.read().await;
            client.check_version_and_provision(
                "rust",
                env!("CARGO_PKG_VERSION"),
                &factory_data.device_serial,
                &factory_data.factory_secret,
                &client_info,
            ).await?
        };

        // Store device identity (no better-auth needed for devices)
        self.device_identity = Some(DeviceIdentity {
            device_id: credentials.device_id.clone(),
            tenant_id: credentials.tenant_id.clone(),
            site_id: credentials.site_id.clone(),
            user_id: credentials.device_id.clone(), // Device acts as its own user
        });

        // Configure client with device credentials directly
        if let Some(client) = &self.convex_client {
            let mut client = client.write().await;
            client.set_auth_token(credentials.auth_token.clone());
            client.set_device_id(credentials.device_id.clone());
            client.set_tenant_id(credentials.tenant_id.clone());
        }

        tracing::info!("Device successfully provisioned with Convex backend");
        tracing::info!("Device ID: {}", credentials.device_id);
        tracing::info!("Tenant ID: {}", credentials.tenant_id);
        tracing::info!("Site ID: {}", credentials.site_id);

        Ok(credentials)
    }

    #[deprecated(note = "Devices use direct device credentials, not better-auth")]
    pub async fn sign_in_with_device(&mut self, device_id: &str, device_key: &str) -> Result<AuthSession> {
        Err(anyhow::anyhow!("Devices should use direct device credentials, not better-auth"))
    }

    #[deprecated(note = "Devices use direct device credentials, not better-auth")]
    pub async fn refresh_auth_session(&mut self) -> Result<String> {
        Err(anyhow::anyhow!("Devices should use direct device credentials, not better-auth"))
    }

    pub async fn sign_out(&mut self) -> Result<()> {
        if self.auth_session.is_none() {
            return Ok(());
        }

        let session_id = self.auth_session.as_ref().unwrap().session_id.clone();
        
        if let Some(client) = &self.convex_client {
            let client = client.read().await;
            
            let _ = client.convex_client
                .mutation("auth:signOut", serde_json::json!({
                    "sessionId": session_id
                }))
                .await;
        }

        if let Some(client) = &self.convex_client {
            let mut client = client.write().await;
            client.set_auth_token("".to_string());
        }

        self.auth_session = None;
        
        tracing::info!("Successfully signed out from better-auth");
        Ok(())
    }

    pub async fn validate_auth_session(&self) -> Result<bool> {
        if self.auth_session.is_none() {
            return Ok(false);
        }

        let session = self.auth_session.as_ref().unwrap();
        let current_time = chrono::Utc::now().timestamp() as u64;
        
        if current_time >= session.expires_at.parse::<u64>().unwrap_or(0) {
            return Ok(false);
        }

        let result = {
            let client = self.convex_client.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Convex client not initialized"))?;
            let client = client.read().await;
            
            client.convex_client
                .query("auth:validateSession", serde_json::json!({
                    "sessionId": session.session_id
                }))
                .await
        };

        match result {
            Ok(response) => Ok(response["valid"].as_bool().unwrap_or(false)),
            Err(_) => Ok(false),
        }
    }

    pub fn get_convex_client(&self) -> Option<Arc<RwLock<ConvexApiClient>>> {
        self.convex_client.clone()
    }

    pub fn get_device_identity(&self) -> Option<&DeviceIdentity> {
        self.device_identity.as_ref()
    }

    pub fn get_auth_session(&self) -> Option<&AuthSession> {
        self.auth_session.as_ref()
    }

    pub fn is_authenticated(&self) -> bool {
        self.auth_session.is_some() && self.device_identity.is_some()
    }

    fn get_factory_provisioning_data(&self) -> Result<FactoryProvisioningData> {
        let device_serial = self.config.device_serial.clone()
            .or_else(|| std::env::var("DEVICE_SERIAL").ok())
            .unwrap_or_else(|| format!("RUST-BODYCAM-{}", Uuid::new_v4().to_string()[..8].to_uppercase()));

        let factory_secret = self.config.factory_secret.clone()
            .or_else(|| std::env::var("FACTORY_SECRET").ok())
            .ok_or_else(|| anyhow::anyhow!("Factory secret not configured - set FACTORY_SECRET env var or add to config"))?;

        let client_info = ClientInfo {
            platform: std::env::consts::OS.to_string(),
            os_version: self.get_os_version(),
            build_number: env!("CARGO_PKG_VERSION").to_string(),
            device_model: "PatrolSight BodyCam Pro".to_string(),
            capabilities: vec![
                "video_recording".to_string(),
                "audio_recording".to_string(),
                "gps_tracking".to_string(),
                "accelerometer".to_string(),
                "live_streaming".to_string(),
                "encryption".to_string(),
                "offline_storage".to_string(),
                "button_actions".to_string(),
                "wifi_management".to_string(),
                "sos_alerts".to_string(),
            ],
        };

        Ok(FactoryProvisioningData {
            device_serial,
            factory_secret,
            client_info,
        })
    }

    fn get_os_version(&self) -> String {
        if cfg!(target_os = "linux") {
            std::fs::read_to_string("/proc/version")
                .unwrap_or_else(|_| "Linux Unknown".to_string())
                .lines()
                .next()
                .unwrap_or("Linux Unknown")
                .to_string()
        } else if cfg!(target_os = "macos") {
            std::process::Command::new("sw_vers")
                .arg("-productVersion")
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .map(|v| format!("macOS {}", v.trim()))
                .unwrap_or_else(|| "macOS Unknown".to_string())
        } else if cfg!(target_os = "windows") {
            std::process::Command::new("cmd")
                .args(&["/C", "ver"])
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .unwrap_or_else(|| "Windows Unknown".to_string())
        } else {
            format!("{} Unknown", std::env::consts::OS)
        }
    }

    pub fn generate_device_keypair(&self) -> Result<(String, SigningKey)> {
        let mut csprng = OsRng {};
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key: VerifyingKey = (&signing_key).into();
        
        let public_key = general_purpose::STANDARD.encode(verifying_key.as_bytes());
        
        Ok((public_key, signing_key))
    }

    pub fn sign_data(&self, data: &[u8], private_key: &SigningKey) -> Result<String> {
        use ed25519_dalek::{Signature, Signer};
        
        let signature: Signature = private_key.sign(data);
        Ok(general_purpose::STANDARD.encode(signature.to_bytes()))
    }

    pub fn verify_signature(&self, data: &[u8], signature: &str, public_key: &str) -> Result<bool> {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};
        
        let signature_bytes = general_purpose::STANDARD.decode(signature)?;
        let public_key_bytes = general_purpose::STANDARD.decode(public_key)?;
        
        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes.try_into().map_err(|_| anyhow::anyhow!("Invalid public key length"))?)?;
        let signature = Signature::from_bytes(&signature_bytes.try_into().map_err(|_| anyhow::anyhow!("Invalid signature length"))?)?;
        
        Ok(verifying_key.verify(data, &signature).is_ok())
    }

    pub async fn save_credentials_to_config(&self, credentials: &DeviceCredentials) -> Result<()> {
        let mut config = self.config.clone();
        config.device_id = Some(credentials.device_id.clone());
        config.device_key = Some(credentials.device_key.clone());
        config.site_id = Some(credentials.site_id.clone());
        config.tenant_id = Some(credentials.tenant_id.clone());
        config.auth_token = Some(credentials.auth_token.clone());

        // Save to config file
        let config_path = std::path::Path::new("config.toml");
        config.save(config_path).await
            .context("Failed to save credentials to config file")?;

        tracing::info!("Device credentials saved to config file");
        Ok(())
    }

    pub fn is_device_provisioned(&self) -> bool {
        self.config.device_id.is_some() 
            && self.config.device_key.is_some()
            && self.config.tenant_id.is_some()
            && self.config.site_id.is_some()
    }

    pub fn get_device_info(&self) -> Option<(String, String, String)> {
        if let (Some(device_id), Some(tenant_id), Some(site_id)) = (
            &self.config.device_id,
            &self.config.tenant_id,
            &self.config.site_id,
        ) {
            Some((device_id.clone(), tenant_id.clone(), site_id.clone()))
        } else {
            None
        }
    }

    pub fn validate_config_for_convex(&self) -> Result<()> {
        if self.config.convex_url.is_none() {
            return Err(anyhow::anyhow!("Convex URL not configured"));
        }

        if self.config.factory_secret.is_none() && std::env::var("FACTORY_SECRET").is_err() {
            return Err(anyhow::anyhow!("Factory secret not configured"));
        }

        // Validate URL format
        if let Some(url) = &self.config.convex_url {
            if !url.starts_with("https://") {
                return Err(anyhow::anyhow!("Convex URL must use HTTPS"));
            }
            if !url.contains("convex.cloud") && !url.contains("localhost") {
                return Err(anyhow::anyhow!("Invalid Convex URL format"));
            }
        }

        Ok(())
    }

    pub async fn get_current_user(&self) -> Result<Option<serde_json::Value>> {
        if !self.is_authenticated() {
            return Ok(None);
        }

        let client = self.convex_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Convex client not initialized"))?;
        let client = client.read().await;
        
        let result = client.convex_client
            .query("auth:getCurrentUser", serde_json::json!({}))
            .await
            .context("Failed to get current user")?;

        Ok(Some(result))
    }

    pub async fn update_user_metadata(&self, metadata: serde_json::Value) -> Result<()> {
        if !self.is_authenticated() {
            return Err(anyhow::anyhow!("Not authenticated"));
        }

        let client = self.convex_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Convex client not initialized"))?;
        let client = client.read().await;
        
        client.convex_client
            .mutation("auth:updateUser", serde_json::json!({
                "metadata": metadata
            }))
            .await
            .context("Failed to update user metadata")?;

        Ok(())
    }
}