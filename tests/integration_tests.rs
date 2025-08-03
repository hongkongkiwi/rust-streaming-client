use bodycam_client::*;
use tempfile::TempDir;
use tokio;

#[tokio::test]
async fn test_device_initialization() {
    let config = config::Config::default();
    let result = device::BodycamDevice::new(config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_encryption_roundtrip() {
    let mut encryptor = encryption::MediaEncryptor::new("test-device".to_string());
    encryptor.initialize_with_password("test-password").await.unwrap();

    let test_data = b"This is test video data for encryption testing";
    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.mp4");
    let encrypted_path = temp_dir.path().join("encrypted.mp4");
    let decrypted_path = temp_dir.path().join("decrypted.mp4");

    // Write test data
    std::fs::write(&input_path, test_data).unwrap();

    // Encrypt
    let metadata = encryptor.encrypt_video_file(&input_path, &encrypted_path).await.unwrap();
    assert_eq!(metadata.original_size, test_data.len() as u64);
    assert!(metadata.encrypted_size > 0);

    // Decrypt
    encryptor.decrypt_video_file(&encrypted_path, &decrypted_path).await.unwrap();

    // Verify
    let decrypted_data = std::fs::read(&decrypted_path).unwrap();
    assert_eq!(decrypted_data, test_data);
}

#[tokio::test]
async fn test_resource_manager() {
    let resource_manager = resource_manager::ResourceManager::new(
        "test-device".to_string(),
        Some(resource_manager::ResourceLimits::default())
    );

    // Test resource stats
    let stats = resource_manager.get_resource_stats().await;
    assert!(!stats.device_id.is_empty());

    // Test temp file registration
    let temp_file = std::env::temp_dir().join("test_temp_file.txt");
    std::fs::write(&temp_file, "test content").unwrap();
    
    resource_manager.register_temp_file(temp_file.clone()).await.unwrap();
    
    // Cleanup
    let _ = std::fs::remove_file(&temp_file);
}

#[cfg(test)]
mod validation_tests {
    use super::*;

    #[test]
    fn test_device_id_validation() {
        assert!(validation::InputValidator::validate_device_id("valid-device-123").is_ok());
        assert!(validation::InputValidator::validate_device_id("").is_err());
        assert!(validation::InputValidator::validate_device_id("invalid..device").is_err());
    }

    #[test]
    fn test_file_path_validation() {
        assert!(validation::InputValidator::validate_file_path("/valid/path/file.mp4").is_ok());
        assert!(validation::InputValidator::validate_file_path("../../../etc/passwd").is_err());
        assert!(validation::InputValidator::validate_file_path("/path/with/..").is_err());
    }

    #[test]
    fn test_uuid_validation() {
        assert!(validation::InputValidator::validate_uuid("550e8400-e29b-41d4-a716-446655440000").is_ok());
        assert!(validation::InputValidator::validate_uuid("invalid-uuid").is_err());
        assert!(validation::InputValidator::validate_uuid("").is_err());
    }
}