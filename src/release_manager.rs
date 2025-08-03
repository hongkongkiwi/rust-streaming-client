use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use chrono::{DateTime, Utc};
use sha2::{Sha256, Digest};
use base64;
use reqwest;
use tokio;
use tracing::{info, warn, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub version: String,
    pub release_date: DateTime<Utc>,
    pub changelog: Vec<String>,
    pub download_url: String,
    pub checksum: String,
    pub signature: Option<String>,
    pub size: u64,
    pub min_system_version: Option<String>,
    pub critical: bool,
    pub rollback_allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub current_version: String,
    pub latest_version: String,
    pub releases: Vec<ReleaseInfo>,
    pub update_channel: UpdateChannel,
    pub last_check: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateChannel {
    Stable,
    Beta,
    Alpha,
    Development,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub build: Option<String>,
}

impl std::fmt::Display for VersionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.build {
            Some(build) => write!(f, "{}.{}.{}-{}", self.major, self.minor, self.patch, build),
            None => write!(f, "{}.{}. {}", self.major, self.minor, self.patch),
        }
    }
}

impl std::str::FromStr for VersionInfo {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(&['.', '-'][..]).collect();
        if parts.len() < 3 {
            return Err(anyhow::anyhow!("Invalid version format"));
        }

        Ok(VersionInfo {
            major: parts[0].parse()?,
            minor: parts[1].parse()?,
            patch: parts[2].parse()?,
            build: if parts.len() > 3 { Some(parts[3].to_string()) } else { None },
        })
    }
}

impl VersionInfo {
    pub fn is_newer_than(&self, other: &VersionInfo) -> bool {
        if self.major != other.major {
            return self.major > other.major;
        }
        if self.minor != other.minor {
            return self.minor > other.minor;
        }
        if self.patch != other.patch {
            return self.patch > other.patch;
        }
        false
    }
}

pub struct ReleaseManager {
    config_dir: PathBuf,
    update_url: String,
    current_version: VersionInfo,
    update_channel: UpdateChannel,
    client: reqwest::Client,
}

impl ReleaseManager {
    pub fn new(config_dir: &Path, update_url: &str, current_version: &str, channel: UpdateChannel) -> Result<Self> {
        let current_version = VersionInfo::from_str(current_version)?;
        
        Ok(Self {
            config_dir: config_dir.to_path_buf(),
            update_url: update_url.to_string(),
            current_version,
            update_channel: channel,
            client: reqwest::Client::new(),
        })
    }

    pub async fn check_for_updates(&self) -> Result<Option<ReleaseInfo>> {
        let manifest = self.fetch_update_manifest().await?;
        let latest_release = manifest.releases.first();

        match latest_release {
            Some(release) => {
                let latest_version = VersionInfo::from_str(&release.version)?;
                if latest_version.is_newer_than(&self.current_version) {
                    info!("Update available: {} -> {}", self.current_version, latest_version);
                    Ok(Some(release.clone()))
                } else {
                    info!("No updates available. Current version is up to date.");
                    Ok(None)
                }
            }
            None => {
                warn!("No releases found in manifest");
                Ok(None)
            }
        }
    }

    async fn fetch_update_manifest(&self) -> Result<UpdateManifest> {
        let url = format!("{}/{}/manifest.json", self.update_url, self.get_channel_path());
        
        let response = self.client
            .get(&url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .context("Failed to fetch update manifest")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to fetch manifest: {}", response.status()));
        }

        let manifest: UpdateManifest = response
            .json()
            .await
            .context("Failed to parse manifest JSON")?;

        Ok(manifest)
    }

    fn get_channel_path(&self) -> &str {
        match self.update_channel {
            UpdateChannel::Stable => "stable",
            UpdateChannel::Beta => "beta",
            UpdateChannel::Alpha => "alpha",
            UpdateChannel::Development => "dev",
        }
    }

    pub async fn download_update(&self, release: &ReleaseInfo) -> Result<PathBuf> {
        let download_dir = self.config_dir.join("downloads");
        tokio::fs::create_dir_all(&download_dir).await?;

        let filename = Path::new(&release.download_url)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("update.zip");

        let download_path = download_dir.join(filename);

        info!("Downloading update from {}", release.download_url);
        
        let response = self.client
            .get(&release.download_url)
            .send()
            .await
            .context("Failed to start download")?;

        let bytes = response
            .bytes()
            .await
            .context("Failed to download update")?;

        tokio::fs::write(&download_path, &bytes).await?;

        // Verify checksum
        self.verify_checksum(&download_path, &release.checksum)?;

        info!("Update downloaded to {}", download_path.display());
        Ok(download_path)
    }

    fn verify_checksum(&self, file_path: &Path, expected_checksum: &str) -> Result<()> {
        let mut file = std::fs::File::open(file_path)?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher)?;
        let result = hasher.finalize();
        let computed_checksum = hex::encode(result);

        if computed_checksum != expected_checksum {
            return Err(anyhow::anyhow!(
                "Checksum mismatch: expected {}, got {}",
                expected_checksum, computed_checksum
            ));
        }

        info!("Checksum verification passed");
        Ok(())
    }

    pub async fn apply_update(&self, download_path: &Path, release: &ReleaseInfo) -> Result<()> {
        let backup_dir = self.config_dir.join("backups");
        tokio::fs::create_dir_all(&backup_dir).await?;

        // Create backup of current version
        let backup_path = backup_dir.join(format!("backup_{}", self.current_version));
        self.create_backup(&backup_path).await?;

        // Extract and apply update
        info!("Applying update...");
        self.extract_update(download_path).await?;

        // Update version file
        self.update_version_file(&release.version).await?;

        info!("Update applied successfully");
        Ok(())
    }

    async fn create_backup(&self, backup_path: &Path) -> Result<()> {
        info!("Creating backup at {}", backup_path.display());
        
        let current_exe = std::env::current_exe()?;
        let current_dir = current_exe.parent()
            .ok_or_else(|| anyhow::anyhow!("Could not determine executable directory"))?;

        // Simple backup - copy current executable
        tokio::fs::copy(&current_exe, backup_path).await?;
        
        Ok(())
    }

    async fn extract_update(&self, download_path: &Path) -> Result<()> {
        let current_exe = std::env::current_exe()?;
        let current_dir = current_exe.parent()
            .ok_or_else(|| anyhow::anyhow!("Could not determine executable directory"))?;

        // For now, simple file copy - in production, would extract from archive
        // This would need platform-specific handling
        #[cfg(target_os = "windows")]
        {
            // Windows: use .exe extension
            let new_exe = current_dir.join("patrolsight-client-new.exe");
            tokio::fs::copy(download_path, &new_exe).await?;
            
            // Schedule rename on next restart
            self.schedule_restart_update(&new_exe).await?;
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Unix-like systems
            let new_exe = current_dir.join("patrolsight-client-new");
            tokio::fs::copy(download_path, &new_exe).await?;
            
            // Make executable
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = tokio::fs::metadata(&new_exe).await?.permissions();
                perms.set_mode(0o755);
                tokio::fs::set_permissions(&new_exe, perms).await?;
            }
            
            // Schedule rename on next restart
            self.schedule_restart_update(&new_exe).await?;
        }

        Ok(())
    }

    async fn schedule_restart_update(&self, new_exe: &Path) -> Result<()> {
        let update_script = self.config_dir.join("update.sh");
        
        #[cfg(target_os = "windows")]
        let script_content = format!(r#"
@echo off
ping 127.0.0.1 -n 3 > nul
move /Y "{}" "{}"
echo Update applied successfully
"#, new_exe.display(), std::env::current_exe()?.display());

        #[cfg(not(target_os = "windows"))]
        let script_content = format!(r#"
#!/bin/bash
sleep 2
mv "{}" "{}"
echo "Update applied successfully"
"#, new_exe.display(), std::env::current_exe()?.display());

        tokio::fs::write(&update_script, script_content).await?;
        
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&update_script).await?.permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&update_script, perms).await?;
        }

        Ok(())
    }

    async fn update_version_file(&self, new_version: &str) -> Result<()> {
        let version_file = self.config_dir.join("version.json");
        let version_data = serde_json::json!({
            "version": new_version,
            "updated_at": chrono::Utc::now().to_rfc3339()
        });

        tokio::fs::write(version_file, serde_json::to_string_pretty(&version_data)?).await?;
        Ok(())
    }

    pub async fn rollback(&self) -> Result<()> {
        let backup_dir = self.config_dir.join("backups");
        let backup_path = backup_dir.join(format!("backup_{}", self.current_version));
        
        if !backup_path.exists() {
            return Err(anyhow::anyhow!("No backup found for rollback"));
        }

        info!("Rolling back to previous version...");
        
        let current_exe = std::env::current_exe()?;
        tokio::fs::copy(&backup_path, &current_exe).await?;
        
        info!("Rollback completed");
        Ok(())
    }

    pub fn get_current_version(&self) -> &VersionInfo {
        &self.current_version
    }

    pub fn get_update_channel(&self) -> &UpdateChannel {
        &self.update_channel
    }

    pub async fn set_update_channel(&mut self, channel: UpdateChannel) -> Result<()> {
        self.update_channel = channel;
        
        let config_file = self.config_dir.join("update_config.toml");
        let config_data = toml::to_string_pretty(&toml::toml! {
            update_channel = match channel {
                UpdateChannel::Stable => "stable",
                UpdateChannel::Beta => "beta", 
                UpdateChannel::Alpha => "alpha",
                UpdateChannel::Development => "development",
            }
            update_url = &self.update_url
        })?;

        tokio::fs::write(config_file, config_data).await?;
        Ok(())
    }
}