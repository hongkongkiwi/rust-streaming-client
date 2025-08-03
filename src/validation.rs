use anyhow::{Result, Context};
use std::path::Path;
use uuid::Uuid;

pub struct InputValidator;

impl InputValidator {
    /// Validate device ID format
    pub fn validate_device_id(device_id: &str) -> Result<()> {
        if device_id.is_empty() {
            return Err(anyhow::anyhow!("Device ID cannot be empty"));
        }
        
        if device_id.len() < 8 || device_id.len() > 64 {
            return Err(anyhow::anyhow!("Device ID must be between 8 and 64 characters"));
        }
        
        if !device_id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(anyhow::anyhow!("Device ID can only contain alphanumeric characters, hyphens, and underscores"));
        }
        
        Ok(())
    }
    
    /// Validate site ID format
    pub fn validate_site_id(site_id: &str) -> Result<()> {
        if site_id.is_empty() {
            return Err(anyhow::anyhow!("Site ID cannot be empty"));
        }
        
        if site_id.len() > 32 {
            return Err(anyhow::anyhow!("Site ID must be 32 characters or less"));
        }
        
        if !site_id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(anyhow::anyhow!("Site ID can only contain alphanumeric characters, hyphens, and underscores"));
        }
        
        Ok(())
    }
    
    /// Validate file path
    pub fn validate_file_path(file_path: &str) -> Result<()> {
        if file_path.is_empty() {
            return Err(anyhow::anyhow!("File path cannot be empty"));
        }
        
        let path = Path::new(file_path);
        
        // Check for directory traversal attacks
        if file_path.contains("..") {
            return Err(anyhow::anyhow!("File path cannot contain directory traversal sequences"));
        }
        
        // Check for null bytes
        if file_path.contains('\0') {
            return Err(anyhow::anyhow!("File path cannot contain null bytes"));
        }
        
        // Validate file extension for media files
        if let Some(extension) = path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            let allowed_extensions = ["mp4", "avi", "mov", "mkv", "wav", "mp3", "flac"];
            if !allowed_extensions.contains(&ext.as_str()) {
                return Err(anyhow::anyhow!("File extension '{}' is not allowed", ext));
            }
        }
        
        Ok(())
    }
    
    /// Validate URL format
    pub fn validate_url(url: &str) -> Result<()> {
        if url.is_empty() {
            return Err(anyhow::anyhow!("URL cannot be empty"));
        }
        
        let parsed_url = url::Url::parse(url)
            .context("Invalid URL format")?;
        
        match parsed_url.scheme() {
            "http" | "https" => {},
            _ => return Err(anyhow::anyhow!("URL must use HTTP or HTTPS protocol")),
        }
        
        if parsed_url.host().is_none() {
            return Err(anyhow::anyhow!("URL must have a valid host"));
        }
        
        Ok(())
    }
    
    /// Validate incident type
    pub fn validate_incident_type(incident_type: &str) -> Result<()> {
        if incident_type.is_empty() {
            return Err(anyhow::anyhow!("Incident type cannot be empty"));
        }
        
        let allowed_types = [
            "emergency", "manual", "motion", "sound", "tamper", 
            "battery_low", "storage_full", "button_press", "panic"
        ];
        
        if !allowed_types.contains(&incident_type) {
            return Err(anyhow::anyhow!("Invalid incident type: {}", incident_type));
        }
        
        Ok(())
    }
    
    /// Validate incident severity
    pub fn validate_incident_severity(severity: &str) -> Result<()> {
        if severity.is_empty() {
            return Err(anyhow::anyhow!("Incident severity cannot be empty"));
        }
        
        let allowed_severities = ["low", "medium", "high", "critical"];
        
        if !allowed_severities.contains(&severity) {
            return Err(anyhow::anyhow!("Invalid incident severity: {}", severity));
        }
        
        Ok(())
    }
    
    /// Validate device name
    pub fn validate_device_name(device_name: &str) -> Result<()> {
        if device_name.is_empty() {
            return Err(anyhow::anyhow!("Device name cannot be empty"));
        }
        
        if device_name.len() > 64 {
            return Err(anyhow::anyhow!("Device name must be 64 characters or less"));
        }
        
        if !device_name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == ' ') {
            return Err(anyhow::anyhow!("Device name can only contain alphanumeric characters, hyphens, underscores, and spaces"));
        }
        
        Ok(())
    }
    
    /// Validate UUID format
    pub fn validate_uuid(uuid_str: &str) -> Result<()> {
        Uuid::parse_str(uuid_str)
            .context("Invalid UUID format")?;
        Ok(())
    }
    
    /// Validate battery level
    pub fn validate_battery_level(level: f32) -> Result<()> {
        if level < 0.0 || level > 100.0 {
            return Err(anyhow::anyhow!("Battery level must be between 0.0 and 100.0"));
        }
        Ok(())
    }
    
    /// Validate temperature
    pub fn validate_temperature(temp: f32) -> Result<()> {
        if temp < -40.0 || temp > 85.0 {
            return Err(anyhow::anyhow!("Temperature must be between -40°C and 85°C"));
        }
        Ok(())
    }
    
    /// Validate recording duration
    pub fn validate_recording_duration(duration: u64) -> Result<()> {
        if duration == 0 {
            return Err(anyhow::anyhow!("Recording duration must be greater than 0"));
        }
        
        if duration > 86400 { // 24 hours in seconds
            return Err(anyhow::anyhow!("Recording duration cannot exceed 24 hours"));
        }
        
        Ok(())
    }
    
    /// Validate audio volume
    pub fn validate_volume(volume: f32) -> Result<()> {
        if volume < 0.0 || volume > 1.0 {
            return Err(anyhow::anyhow!("Volume must be between 0.0 and 1.0"));
        }
        Ok(())
    }
    
    /// Validate resolution string
    pub fn validate_resolution(resolution: &str) -> Result<()> {
        if resolution.is_empty() {
            return Err(anyhow::anyhow!("Resolution cannot be empty"));
        }
        
        let parts: Vec<&str> = resolution.split('x').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Resolution must be in format 'WIDTHxHEIGHT'"));
        }
        
        let width: u32 = parts[0].parse()
            .context("Width must be a valid number")?;
        let height: u32 = parts[1].parse()
            .context("Height must be a valid number")?;
        
        if width < 320 || height < 240 {
            return Err(anyhow::anyhow!("Resolution must be at least 320x240"));
        }
        
        if width > 7680 || height > 4320 { // 8K max
            return Err(anyhow::anyhow!("Resolution cannot exceed 7680x4320"));
        }
        
        Ok(())
    }
    
    /// Validate FPS value
    pub fn validate_fps(fps: u32) -> Result<()> {
        let allowed_fps = [15, 24, 25, 30, 50, 60, 120];
        
        if !allowed_fps.contains(&fps) {
            return Err(anyhow::anyhow!("FPS must be one of: {:?}", allowed_fps));
        }
        
        Ok(())
    }
    
    /// Validate GPS coordinates
    pub fn validate_gps_coordinates(latitude: f64, longitude: f64) -> Result<()> {
        if latitude < -90.0 || latitude > 90.0 {
            return Err(anyhow::anyhow!("Latitude must be between -90.0 and 90.0"));
        }
        
        if longitude < -180.0 || longitude > 180.0 {
            return Err(anyhow::anyhow!("Longitude must be between -180.0 and 180.0"));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_device_id() {
        assert!(InputValidator::validate_device_id("device-123").is_ok());
        assert!(InputValidator::validate_device_id("").is_err());
        assert!(InputValidator::validate_device_id("dev@ice").is_err());
        assert!(InputValidator::validate_device_id("a".repeat(65).as_str()).is_err());
    }

    #[test]
    fn test_validate_battery_level() {
        assert!(InputValidator::validate_battery_level(50.0).is_ok());
        assert!(InputValidator::validate_battery_level(-1.0).is_err());
        assert!(InputValidator::validate_battery_level(101.0).is_err());
    }

    #[test]
    fn test_validate_resolution() {
        assert!(InputValidator::validate_resolution("1920x1080").is_ok());
        assert!(InputValidator::validate_resolution("invalid").is_err());
        assert!(InputValidator::validate_resolution("100x100").is_err());
    }

    #[test]
    fn test_validate_gps_coordinates() {
        assert!(InputValidator::validate_gps_coordinates(37.7749, -122.4194).is_ok());
        assert!(InputValidator::validate_gps_coordinates(91.0, 0.0).is_err());
        assert!(InputValidator::validate_gps_coordinates(0.0, 181.0).is_err());
    }
}