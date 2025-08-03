use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::convex_api::{ConvexApiClient, ConvexDeviceStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantContext {
    pub tenant_id: String,
    pub device_id: String,
    pub site_id: String,
    pub user_id: String,
    pub permissions: Vec<String>,
    pub scopes: TenantScopes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantScopes {
    pub can_access_data: bool,
    pub can_upload_media: bool,
    pub can_create_incidents: bool,
    pub can_modify_settings: bool,
    pub can_view_other_devices: bool,
    pub can_access_admin_panel: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantValidationResult {
    pub valid: bool,
    pub tenant_id: String,
    pub device_id: String,
    pub permissions: Vec<String>,
    pub error: Option<String>,
}

pub struct TenantManager {
    convex_client: Arc<RwLock<ConvexApiClient>>,
    config: Arc<RwLock<Config>>,
    current_context: Arc<RwLock<Option<TenantContext>>>,
    tenant_cache: Arc<RwLock<HashMap<String, TenantContext>>>,
}

impl TenantManager {
    pub fn new(
        convex_client: Arc<RwLock<ConvexApiClient>>,
        config: Arc<RwLock<Config>>,
    ) -> Self {
        Self {
            convex_client,
            config,
            current_context: Arc::new(RwLock::new(None)),
            tenant_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn initialize_tenant_context(&self) -> Result<TenantContext> {
        let config = self.config.read().await;
        
        let tenant_id = config.tenant_id.clone()
            .ok_or_else(|| anyhow::anyhow!("Tenant ID not configured"))?;
            
        let device_id = config.device_id.clone()
            .ok_or_else(|| anyhow::anyhow!("Device ID not configured"))?;
            
        let site_id = config.site_id.clone()
            .ok_or_else(|| anyhow::anyhow!("Site ID not configured"))?;

        let context = self.validate_and_get_context(&tenant_id, &device_id, &site_id).await?;
        
        // Cache the context
        {
            let mut cache = self.tenant_cache.write().await;
            cache.insert(tenant_id.clone(), context.clone());
            
            let mut current = self.current_context.write().await;
            *current = Some(context.clone());
        }

        tracing::info!("Initialized tenant context for tenant: {}", tenant_id);
        Ok(context)
    }

    pub async fn validate_and_get_context(
        &self,
        tenant_id: &str,
        device_id: &str,
        site_id: &str,
    ) -> Result<TenantContext> {
        let client = self.convex_client.read().await;
        
        let validation = client.convex_client
            .query("tenant:validateDeviceAccess", serde_json::json!({
                "tenantId": tenant_id,
                "deviceId": device_id,
                "siteId": site_id
            }))
            .await
            .context("Failed to validate device access")?;

        let valid = validation["valid"].as_bool().unwrap_or(false);
        if !valid {
            return Err(anyhow::anyhow!(
                "Device {} does not have access to tenant {} in site {}",
                device_id, tenant_id, site_id
            ));
        }

        let permissions = validation["permissions"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|p| p.as_str().map(|s| s.to_string()))
            .collect();

        let scopes = TenantScopes {
            can_access_data: validation["scopes"]["canAccessData"].as_bool().unwrap_or(true),
            can_upload_media: validation["scopes"]["canUploadMedia"].as_bool().unwrap_or(true),
            can_create_incidents: validation["scopes"]["canCreateIncidents"].as_bool().unwrap_or(true),
            can_modify_settings: validation["scopes"]["canModifySettings"].as_bool().unwrap_or(false),
            can_view_other_devices: validation["scopes"]["canViewOtherDevices"].as_bool().unwrap_or(false),
            can_access_admin_panel: validation["scopes"]["canAccessAdminPanel"].as_bool().unwrap_or(false),
        };

        let user_id = validation["userId"].as_str()
            .unwrap_or("system")
            .to_string();

        Ok(TenantContext {
            tenant_id: tenant_id.to_string(),
            device_id: device_id.to_string(),
            site_id: site_id.to_string(),
            user_id,
            permissions,
            scopes,
        })
    }

    pub async fn get_current_context(&self) -> Result<TenantContext> {
        let context = self.current_context.read().await;
        context.clone().ok_or_else(|| anyhow::anyhow!("No tenant context available"))
    }

    pub async fn validate_tenant_scope(&self, required_scope: &str) -> Result<bool> {
        let context = self.get_current_context().await?;
        
        match required_scope {
            "data_access" => Ok(context.scopes.can_access_data),
            "media_upload" => Ok(context.scopes.can_upload_media),
            "incident_creation" => Ok(context.scopes.can_create_incidents),
            "settings_modification" => Ok(context.scopes.can_modify_settings),
            "device_viewing" => Ok(context.scopes.can_view_other_devices),
            "admin_access" => Ok(context.scopes.can_access_admin_panel),
            _ => Ok(false),
        }
    }

    pub async fn get_tenant_scoped_device_status(&self, status: ConvexDeviceStatus) -> Result<ConvexDeviceStatus> {
        let context = self.get_current_context().await?;
        
        let mut scoped_status = status;
        scoped_status.tenant_id = context.tenant_id;
        scoped_status.device_id = context.device_id;
        
        Ok(scoped_status)
    }

    pub async fn create_tenant_scoped_incident(
        &self,
        incident_type: &str,
        button_type: &str,
        metadata: serde_json::Value,
    ) -> Result<String> {
        let context = self.get_current_context().await?;
        
        if !context.scopes.can_create_incidents {
            return Err(anyhow::anyhow!("Device does not have permission to create incidents"));
        }

        let client = self.convex_client.read().await;
        
        let incident_id = client.convex_client
            .mutation("tenant:createIncident", serde_json::json!({
                "tenantId": context.tenant_id,
                "deviceId": context.device_id,
                "siteId": context.site_id,
                "incidentType": incident_type,
                "buttonType": button_type,
                "metadata": metadata
            }))
            .await
            .context("Failed to create tenant-scoped incident")?;

        incident_id["incidentId"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing incidentId in response"))
            .map(|s| s.to_string())
    }

    pub async fn get_tenant_device_settings(&self) -> Result<serde_json::Value> {
        let context = self.get_current_context().await?;
        
        let client = self.convex_client.read().await;
        
        let settings = client.convex_client
            .query("tenant:getDeviceSettings", serde_json::json!({
                "tenantId": context.tenant_id,
                "deviceId": context.device_id,
                "siteId": context.site_id
            }))
            .await
            .context("Failed to get tenant-scoped device settings")?;

        Ok(settings)
    }

    pub async fn update_tenant_device_settings(&self, settings: serde_json::Value) -> Result<()> {
        let context = self.get_current_context().await?;
        
        if !context.scopes.can_modify_settings {
            return Err(anyhow::anyhow!("Device does not have permission to modify settings"));
        }

        let client = self.convex_client.read().await;
        
        client.convex_client
            .mutation("tenant:updateDeviceSettings", serde_json::json!({
                "tenantId": context.tenant_id,
                "deviceId": context.device_id,
                "siteId": context.site_id,
                "settings": settings
            }))
            .await
            .context("Failed to update tenant-scoped device settings")?;

        Ok(())
    }

    pub async fn list_tenant_devices(&self) -> Result<Vec<String>> {
        let context = self.get_current_context().await?;
        
        if !context.scopes.can_view_other_devices {
            return Ok(vec![context.device_id.clone()]);
        }

        let client = self.convex_client.read().await;
        
        let devices = client.convex_client
            .query("tenant:listDevices", serde_json::json!({
                "tenantId": context.tenant_id,
                "siteId": context.site_id
            }))
            .await
            .context("Failed to list tenant devices")?;

        let device_list = devices["devices"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|d| d.as_str().map(|s| s.to_string()))
            .collect();

        Ok(device_list)
    }

    pub async fn get_tenant_analytics(&self) -> Result<serde_json::Value> {
        let context = self.get_current_context().await?;
        
        let client = self.convex_client.read().await;
        
        let analytics = client.convex_client
            .query("tenant:getAnalytics", serde_json::json!({
                "tenantId": context.tenant_id,
                "siteId": context.site_id,
                "deviceId": context.device_id
            }))
            .await
            .context("Failed to get tenant analytics")?;

        Ok(analytics)
    }

    pub async fn switch_tenant_context(&self, new_tenant_id: &str) -> Result<TenantContext> {
        let config = self.config.read().await;
        
        let device_id = config.device_id.clone()
            .ok_or_else(|| anyhow::anyhow!("Device ID not configured"))?;
            
        let site_id = config.site_id.clone()
            .ok_or_else(|| anyhow::anyhow!("Site ID not configured"))?;

        // Check if we already have this context cached
        {
            let cache = self.tenant_cache.read().await;
            if let Some(cached_context) = cache.get(new_tenant_id) {
                let mut current = self.current_context.write().await;
                *current = Some(cached_context.clone());
                return Ok(cached_context.clone());
            }
        }

        // Otherwise validate and create new context
        let new_context = self.validate_and_get_context(new_tenant_id, &device_id, &site_id).await?;

        // Update cache and current context
        {
            let mut cache = self.tenant_cache.write().await;
            cache.insert(new_tenant_id.to_string(), new_context.clone());
            
            let mut current = self.current_context.write().await;
            *current = Some(new_context.clone());
        }

        tracing::info!("Switched to tenant context: {}", new_tenant_id);
        Ok(new_context)
    }

    pub async fn clear_tenant_cache(&self) {
        let mut cache = self.tenant_cache.write().await;
        cache.clear();
        
        let mut current = self.current_context.write().await;
        *current = None;
        
        tracing::info!("Tenant cache cleared");
    }

    pub async fn get_cached_contexts(&self) -> Vec<TenantContext> {
        let cache = self.tenant_cache.read().await;
        cache.values().cloned().collect()
    }

    pub async fn validate_device_ownership(&self, target_device_id: &str) -> Result<bool> {
        let context = self.get_current_context().await?;
        
        // Device can always access its own data
        if target_device_id == context.device_id {
            return Ok(true);
        }

        // Check if device has permission to access other devices
        if !context.scopes.can_view_other_devices {
            return Ok(false);
        }

        let client = self.convex_client.read().await;
        
        let result = client.convex_client
            .query("tenant:validateDeviceOwnership", serde_json::json!({
                "tenantId": context.tenant_id,
                "siteId": context.site_id,
                "deviceId": target_device_id
            }))
            .await
            .context("Failed to validate device ownership")?;

        Ok(result["valid"].as_bool().unwrap_or(false))
    }
}