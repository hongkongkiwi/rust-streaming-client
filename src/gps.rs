use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsLocation {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
    pub accuracy: Option<f64>,
    pub speed: Option<f64>,
    pub heading: Option<f64>,
    pub timestamp: DateTime<Utc>,
    pub satellites: Option<u32>,
}

impl GpsLocation {
    pub fn is_valid(&self) -> bool {
        self.latitude.abs() <= 90.0 && self.longitude.abs() <= 180.0
    }
}

pub struct GpsManager {
    enabled: bool,
    last_location: Arc<Mutex<Option<GpsLocation>>>,
    update_interval: std::time::Duration,
}

impl GpsManager {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            last_location: Arc::new(Mutex::new(None)),
            update_interval: std::time::Duration::from_secs(5),
        }
    }

    pub async fn start_monitoring(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let last_location = self.last_location.clone();
        let update_interval = self.update_interval;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(update_interval);
            
            loop {
                interval.tick().await;
                
                match Self::get_current_location().await {
                    Ok(location) => {
                        *last_location.lock().await = Some(location);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to get GPS location: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn get_location(&self) -> Option<GpsLocation> {
        self.last_location.lock().await.clone()
    }

    async fn get_current_location() -> Result<GpsLocation> {
        // Try multiple methods to get GPS location
        
        // Method 1: GPSD (Linux GPS daemon)
        if let Ok(location) = Self::get_location_from_gpsd().await {
            return Ok(location);
        }
        
        // Method 2: CoreLocation (macOS)
        if let Ok(location) = Self::get_location_from_corelocation().await {
            return Ok(location);
        }
        
        // Method 3: GeoClue (Linux)
        if let Ok(location) = Self::get_location_from_geoclue().await {
            return Ok(location);
        }
        
        // Method 4: Fallback to IP geolocation
        Self::get_location_from_ip().await
    }

    async fn get_location_from_gpsd() -> Result<GpsLocation> {
        let output = Command::new("gpspipe")
            .arg("-w")
            .arg("-n")
            .arg("1")
            .output()
            .await
            .context("GPSD not available")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("GPSD command failed"));
        }

        let data = String::from_utf8_lossy(&output.stdout);
        
        // Parse GPSD JSON response
        for line in data.lines() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(tpv) = json.get("TPV") {
                    if let (Some(lat), Some(lon)) = (tpv.get("lat"), tpv.get("lon")) {
                        return Ok(GpsLocation {
                            latitude: lat.as_f64().unwrap_or(0.0),
                            longitude: lon.as_f64().unwrap_or(0.0),
                            altitude: tpv.get("alt").and_then(|v| v.as_f64()),
                            accuracy: tpv.get("epx").and_then(|v| v.as_f64()),
                            speed: tpv.get("speed").and_then(|v| v.as_f64()),
                            heading: tpv.get("track").and_then(|v| v.as_f64()),
                            timestamp: Utc::now(),
                            satellites: tpv.get("satellites").and_then(|v| v.as_u64().map(|v| v as u32)),
                        });
                    }
                }
            }
        }

        Err(anyhow::anyhow!("No GPS data from GPSD"))
    }

    async fn get_location_from_corelocation() -> Result<GpsLocation> {
        // macOS CoreLocation CLI
        let output = Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to tell location preferences to get location")
            .output()
            .await
            .context("CoreLocation not available")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("CoreLocation command failed"));
        }

        // Parse macOS location data
        let data = String::from_utf8_lossy(&output.stdout);
        
        // This is a simplified implementation
        // In practice, you'd need to parse the actual AppleScript response
        Err(anyhow::anyhow!("CoreLocation parsing not implemented"))
    }

    async fn get_location_from_geoclue() -> Result<GpsLocation> {
        let output = Command::new("geoclue-test-gps")
            .arg("-t")
            .arg("1")
            .output()
            .await
            .context("GeoClue not available")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("GeoClue command failed"));
        }

        let data = String::from_utf8_lossy(&output.stdout);
        
        // Parse GeoClue JSON response
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
            if let (Some(lat), Some(lon)) = (json.get("latitude"), json.get("longitude")) {
                return Ok(GpsLocation {
                    latitude: lat.as_f64().unwrap_or(0.0),
                    longitude: lon.as_f64().unwrap_or(0.0),
                    altitude: json.get("altitude").and_then(|v| v.as_f64()),
                    accuracy: json.get("accuracy").and_then(|v| v.as_f64()),
                    speed: json.get("speed").and_then(|v| v.as_f64()),
                    heading: json.get("heading").and_then(|v| v.as_f64()),
                    timestamp: Utc::now(),
                    satellites: None,
                });
            }
        }

        Err(anyhow::anyhow!("No GPS data from GeoClue"))
    }

    async fn get_location_from_ip() -> Result<GpsLocation> {
        // Fallback to IP geolocation
        let response = reqwest::get("http://ip-api.com/json/")
            .await
            .context("IP geolocation request failed")?
            .json::<serde_json::Value>()
            .await
            .context("Failed to parse IP geolocation response")?;

        if response.get("status").and_then(|s| s.as_str()) == Some("success") {
            let lat = response.get("lat").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let lon = response.get("lon").and_then(|v| v.as_f64()).unwrap_or(0.0);

            if lat != 0.0 || lon != 0.0 {
                return Ok(GpsLocation {
                    latitude: lat,
                    longitude: lon,
                    altitude: None,
                    accuracy: response.get("accuracy").and_then(|v| v.as_f64()),
                    speed: None,
                    heading: None,
                    timestamp: Utc::now(),
                    satellites: None,
                });
            }
        }

        Err(anyhow::anyhow!("IP geolocation failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gps_manager_creation() {
        let manager = GpsManager::new(true);
        assert!(manager.enabled);
    }

    #[tokio::test]
    async fn test_gps_location_validation() {
        let location = GpsLocation {
            latitude: 37.7749,
            longitude: -122.4194,
            altitude: Some(50.0),
            accuracy: Some(5.0),
            speed: Some(10.0),
            heading: Some(45.0),
            timestamp: Utc::now(),
            satellites: Some(8),
        };

        assert!(location.is_valid());
    }

    #[tokio::test]
    async fn test_invalid_gps_location() {
        let location = GpsLocation {
            latitude: 95.0, // Invalid
            longitude: -122.4194,
            altitude: None,
            accuracy: None,
            speed: None,
            heading: None,
            timestamp: Utc::now(),
            satellites: None,
        };

        assert!(!location.is_valid());
    }
}