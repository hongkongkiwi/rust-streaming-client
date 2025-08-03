use anyhow::Result;
use sentry::Level;
use std::collections::BTreeMap;
use tracing::{warn, error};

use crate::sentry_integration;

/// Custom error types for device operations
#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("Hardware error: {message}")]
    Hardware { message: String },
    
    #[error("Authentication error: {message}")]
    Authentication { message: String },
    
    #[error("Recording error: {message}")]
    Recording { message: String },
    
    #[error("Network error: {message}")]
    Network { message: String },
    
    #[error("Configuration error: {message}")]
    Configuration { message: String },
    
    #[error("Storage error: {message}")]
    Storage { message: String },
    
    #[error("Battery critical: {level}%")]
    BatteryCritical { level: f32 },
    
    #[error("Device not provisioned")]
    NotProvisioned,
    
    #[error("Operation timeout: {operation}")]
    Timeout { operation: String },
    
    #[error("Resource exhausted: {resource}")]
    ResourceExhausted { resource: String },
}

impl DeviceError {
    /// Get the error category for Sentry breadcrumbs
    pub fn category(&self) -> &'static str {
        match self {
            DeviceError::Hardware { .. } => "hardware",
            DeviceError::Authentication { .. } => "auth",
            DeviceError::Recording { .. } => "recording",
            DeviceError::Network { .. } => "network",
            DeviceError::Configuration { .. } => "config",
            DeviceError::Storage { .. } => "storage",
            DeviceError::BatteryCritical { .. } => "power",
            DeviceError::NotProvisioned => "provisioning",
            DeviceError::Timeout { .. } => "timeout",
            DeviceError::ResourceExhausted { .. } => "resources",
        }
    }
    
    /// Get the Sentry level for this error
    pub fn sentry_level(&self) -> Level {
        match self {
            DeviceError::BatteryCritical { .. } => Level::Fatal,
            DeviceError::Hardware { .. } => Level::Error,
            DeviceError::Recording { .. } => Level::Error,
            DeviceError::Storage { .. } => Level::Error,
            DeviceError::ResourceExhausted { .. } => Level::Error,
            DeviceError::Authentication { .. } => Level::Warning,
            DeviceError::Network { .. } => Level::Warning,
            DeviceError::Configuration { .. } => Level::Warning,
            DeviceError::NotProvisioned => Level::Info,
            DeviceError::Timeout { .. } => Level::Warning,
        }
    }
    
    /// Convert to Sentry context
    pub fn to_sentry_context(&self) -> BTreeMap<String, sentry::protocol::Value> {
        let mut context = BTreeMap::new();
        context.insert("error_category".to_string(), self.category().into());
        
        match self {
            DeviceError::Hardware { message } => {
                context.insert("hardware_message".to_string(), message.clone().into());
            }
            DeviceError::Authentication { message } => {
                context.insert("auth_message".to_string(), message.clone().into());
            }
            DeviceError::Recording { message } => {
                context.insert("recording_message".to_string(), message.clone().into());
            }
            DeviceError::Network { message } => {
                context.insert("network_message".to_string(), message.clone().into());
            }
            DeviceError::Configuration { message } => {
                context.insert("config_message".to_string(), message.clone().into());
            }
            DeviceError::Storage { message } => {
                context.insert("storage_message".to_string(), message.clone().into());
            }
            DeviceError::BatteryCritical { level } => {
                context.insert("battery_level".to_string(), (*level as f64).into());
            }
            DeviceError::Timeout { operation } => {
                context.insert("timeout_operation".to_string(), operation.clone().into());
            }
            DeviceError::ResourceExhausted { resource } => {
                context.insert("exhausted_resource".to_string(), resource.clone().into());
            }
            DeviceError::NotProvisioned => {
                context.insert("provisioning_status".to_string(), "not_provisioned".into());
            }
        }
        
        context
    }
}

/// Result type for device operations
pub type DeviceResult<T> = Result<T, DeviceError>;

/// Wrapper for device operations with automatic error handling and Sentry integration
pub struct DeviceOperationWrapper {
    operation_name: String,
    device_id: Option<String>,
}

impl DeviceOperationWrapper {
    pub fn new(operation_name: &str, device_id: Option<String>) -> Self {
        Self {
            operation_name: operation_name.to_string(),
            device_id,
        }
    }
    
    /// Execute an operation with full error handling and monitoring
    pub async fn execute<F, T, E>(&self, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let transaction = sentry_integration::start_transaction(
            &format!("device.{}", self.operation_name),
            "device_operation",
        );
        
        // Add operation breadcrumb
        sentry_integration::add_device_breadcrumb(
            &format!("{}_start", self.operation_name),
            self.device_id.as_deref(),
        );
        
        let start_time = std::time::Instant::now();
        let result = operation.await;
        let duration = start_time.elapsed();
        
        match result {
            Ok(value) => {
                // Log successful operation
                tracing::info!(
                    operation = %self.operation_name,
                    duration_ms = %duration.as_millis(),
                    device_id = %self.device_id.as_deref().unwrap_or("unknown"),
                    "Device operation completed successfully"
                );
                
                sentry_integration::add_device_breadcrumb(
                    &format!("{}_success", self.operation_name),
                    Some(&format!("duration: {}ms", duration.as_millis())),
                );
                
                Ok(value)
            }
            Err(error) => {
                // Log and report error
                error!(
                    operation = %self.operation_name,
                    duration_ms = %duration.as_millis(),
                    device_id = %self.device_id.as_deref().unwrap_or("unknown"),
                    error = %error,
                    "Device operation failed"
                );
                
                sentry_integration::add_device_breadcrumb(
                    &format!("{}_error", self.operation_name),
                    Some(&format!("error: {}", error)),
                );
                
                // Create context for Sentry
                let mut context = BTreeMap::new();
                context.insert("operation".to_string(), self.operation_name.clone().into());
                context.insert("duration_ms".to_string(), (duration.as_millis() as u64).into());
                if let Some(ref device_id) = self.device_id {
                    context.insert("device_id".to_string(), device_id.clone().into());
                }
                
                sentry_integration::capture_error_with_context(
                    &anyhow::anyhow!(error),
                    Some(context),
                );
                
                Err(anyhow::anyhow!(error).context(format!("Operation '{}' failed", self.operation_name)))
            }
        }
    }
    
    /// Execute a device-specific operation that returns DeviceError
    pub async fn execute_device_operation<F, T>(&self, operation: F) -> DeviceResult<T>
    where
        F: std::future::Future<Output = DeviceResult<T>>,
    {
        let transaction = sentry_integration::start_transaction(
            &format!("device.{}", self.operation_name),
            "device_operation",
        );
        
        sentry_integration::add_device_breadcrumb(
            &format!("{}_start", self.operation_name),
            self.device_id.as_deref(),
        );
        
        let start_time = std::time::Instant::now();
        let result = operation.await;
        let duration = start_time.elapsed();
        
        match result {
            Ok(value) => {
                tracing::info!(
                    operation = %self.operation_name,
                    duration_ms = %duration.as_millis(),
                    device_id = %self.device_id.as_deref().unwrap_or("unknown"),
                    "Device operation completed successfully"
                );
                
                sentry_integration::add_device_breadcrumb(
                    &format!("{}_success", self.operation_name),
                    Some(&format!("duration: {}ms", duration.as_millis())),
                );
                
                Ok(value)
            }
            Err(device_error) => {
                tracing::error!(
                    operation = %self.operation_name,
                    duration_ms = %duration.as_millis(),
                    device_id = %self.device_id.as_deref().unwrap_or("unknown"),
                    error = %device_error,
                    error_category = %device_error.category(),
                    "Device operation failed"
                );
                
                sentry_integration::add_breadcrumb(
                    &format!("{} operation failed: {}", self.operation_name, device_error),
                    device_error.category(),
                    device_error.sentry_level(),
                );
                
                // Create enhanced context
                let mut context = device_error.to_sentry_context();
                context.insert("operation".to_string(), self.operation_name.clone().into());
                context.insert("duration_ms".to_string(), (duration.as_millis() as u64).into());
                if let Some(ref device_id) = self.device_id {
                    context.insert("device_id".to_string(), device_id.clone().into());
                }
                
                sentry_integration::capture_message_with_context(
                    &device_error.to_string(),
                    device_error.sentry_level(),
                    Some(context),
                );
                
                Err(device_error)
            }
        }
    }
}

/// Convenience macro for wrapping device operations
#[macro_export]
macro_rules! device_operation {
    ($operation_name:expr, $device_id:expr, $operation:expr) => {
        $crate::error_handling::DeviceOperationWrapper::new($operation_name, $device_id)
            .execute($operation)
            .await
    };
}

/// Convenience macro for wrapping device operations that return DeviceError
#[macro_export]
macro_rules! device_operation_result {
    ($operation_name:expr, $device_id:expr, $operation:expr) => {
        $crate::error_handling::DeviceOperationWrapper::new($operation_name, $device_id)
            .execute_device_operation($operation)
            .await
    };
}

/// Hardware operation helpers
pub mod hardware {
    use super::*;
    
    /// Wrap hardware-related operations
    pub async fn with_hardware_error_handling<F, T, E>(
        operation_name: &str,
        device_id: Option<String>,
        operation: F,
    ) -> Result<T>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: std::error::Error + Send + Sync + 'static,
    {
        sentry_integration::add_hardware_breadcrumb("hardware", operation_name, "starting");
        
        match DeviceOperationWrapper::new(operation_name, device_id).execute(operation).await {
            Ok(result) => {
                sentry_integration::add_hardware_breadcrumb("hardware", operation_name, "success");
                Ok(result)
            }
            Err(error) => {
                sentry_integration::add_hardware_breadcrumb("hardware", operation_name, "failed");
                Err(error)
            }
        }
    }
}

/// Network operation helpers
pub mod network {
    use super::*;
    
    /// Wrap network-related operations
    pub async fn with_network_error_handling<F, T, E>(
        operation_name: &str,
        endpoint: Option<&str>,
        operation: F,
    ) -> Result<T>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: std::error::Error + Send + Sync + 'static,
    {
        sentry_integration::add_network_breadcrumb(operation_name, endpoint, "starting");
        
        match DeviceOperationWrapper::new(operation_name, None).execute(operation).await {
            Ok(result) => {
                sentry_integration::add_network_breadcrumb(operation_name, endpoint, "success");
                Ok(result)
            }
            Err(error) => {
                sentry_integration::add_network_breadcrumb(operation_name, endpoint, "failed");
                Err(error)
            }
        }
    }
}