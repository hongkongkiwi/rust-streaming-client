use anyhow::{Result, Context};
use sha2::{Sha256, Digest};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoIntegrity {
    pub file_path: String,
    pub sha256_hash: String,
    pub file_size: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub metadata_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityVerification {
    pub is_valid: bool,
    pub expected_hash: String,
    pub actual_hash: String,
    pub verification_time: chrono::DateTime<chrono::Utc>,
    pub errors: Vec<String>,
}

pub struct IntegrityManager;

impl IntegrityManager {
    pub async fn calculate_file_hash(file_path: &Path) -> Result<String> {
        let mut file = File::open(file_path).await
            .context(format!("Failed to open file: {:?}", file_path))?;
        
        let mut hasher = Sha256::new();
        let mut buffer = vec![0; 1024 * 1024]; // 1MB buffer
        
        loop {
            let bytes_read = file.read(&mut buffer).await
                .context("Failed to read file")?;
            
            if bytes_read == 0 {
                break;
            }
            
            hasher.update(&buffer[..bytes_read]);
        }
        
        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }

    pub async fn verify_file_integrity(
        file_path: &Path,
        expected_hash: &str,
    ) -> Result<IntegrityVerification> {
        let actual_hash = Self::calculate_file_hash(file_path).await?;
        
        let is_valid = actual_hash.eq_ignore_ascii_case(expected_hash);
        let mut errors = Vec::new();
        
        if !is_valid {
            errors.push(format!("Hash mismatch: expected {}, got {}", expected_hash, actual_hash));
        }
        
        let metadata = tokio::fs::metadata(file_path).await;
        if let Err(e) = metadata {
            errors.push(format!("File metadata error: {}", e));
        }
        
        Ok(IntegrityVerification {
            is_valid,
            expected_hash: expected_hash.to_string(),
            actual_hash,
            verification_time: chrono::Utc::now(),
            errors,
        })
    }

    pub async fn create_integrity_record(
        file_path: &Path,
        metadata: &serde_json::Value,
    ) -> Result<VideoIntegrity> {
        let file_hash = Self::calculate_file_hash(file_path).await?;
        let metadata_str = serde_json::to_string(metadata)?;
        let metadata_hash = format!("{:x}", Sha256::digest(metadata_str.as_bytes()));
        
        let metadata = tokio::fs::metadata(file_path).await?;
        
        Ok(VideoIntegrity {
            file_path: file_path.to_string_lossy().to_string(),
            sha256_hash: file_hash,
            file_size: metadata.len(),
            created_at: chrono::Utc::now(),
            metadata_hash: metadata_hash,
        })
    }

    pub async fn batch_verify_integrity(
        records: Vec<VideoIntegrity>,
    ) -> Result<Vec<IntegrityVerification>> {
        let mut results = Vec::new();
        
        for record in records {
            let path = Path::new(&record.file_path);
            match Self::verify_file_integrity(path, &record.sha256_hash).await {
                Ok(verification) => results.push(verification),
                Err(e) => results.push(IntegrityVerification {
                    is_valid: false,
                    expected_hash: record.sha256_hash.clone(),
                    actual_hash: "ERROR".to_string(),
                    verification_time: chrono::Utc::now(),
                    errors: vec![e.to_string()],
                }),
            }
        }
        
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_calculate_file_hash() {
        let temp_file = NamedTempFile::new().unwrap();
        let content = b"test content for hashing";
        
        tokio::fs::write(temp_file.path(), content).await.unwrap();
        
        let hash = IntegrityManager::calculate_file_hash(temp_file.path()).await.unwrap();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA256 produces 64 hex characters
    }

    #[tokio::test]
    async fn test_verify_integrity() {
        let temp_file = NamedTempFile::new().unwrap();
        let content = b"test content";
        
        tokio::fs::write(temp_file.path(), content).await.unwrap();
        
        let hash = IntegrityManager::calculate_file_hash(temp_file.path()).await.unwrap();
        let verification = IntegrityManager::verify_file_integrity(temp_file.path(), &hash).await.unwrap();
        
        assert!(verification.is_valid);
        assert_eq!(verification.expected_hash, hash);
        assert_eq!(verification.actual_hash, hash);
        assert!(verification.errors.is_empty());
    }

    #[tokio::test]
    async fn test_create_integrity_record() {
        let temp_file = NamedTempFile::new().unwrap();
        let content = b"test content for integrity record";
        
        tokio::fs::write(temp_file.path(), content).await.unwrap();
        
        let metadata = serde_json::json!({
            "duration": 30,
            "resolution": "1920x1080",
            "fps": 30
        });
        
        let record = IntegrityManager::create_integrity_record(temp_file.path(), &metadata).await.unwrap();
        
        assert_eq!(record.file_path, temp_file.path().to_string_lossy().to_string());
        assert!(!record.sha256_hash.is_empty());
        assert_eq!(record.file_size, content.len() as u64);
        assert!(!record.metadata_hash.is_empty());
    }
}