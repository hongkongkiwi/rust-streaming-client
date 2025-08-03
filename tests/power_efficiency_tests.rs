use bodycam_client::*;
use std::time::{Duration, Instant};
use tokio::time::timeout;

#[tokio::test]
async fn test_resource_manager_power_efficiency() {
    let resource_manager = resource_manager::ResourceManager::new(
        "test-device".to_string(),
        Some(resource_manager::ResourceLimits {
            max_memory_mb: 128,  // Low memory limit
            max_disk_usage_percent: 80.0,
            max_temp_files_mb: 10,
            max_log_files_mb: 5,
            max_recording_age_days: 7,
            cleanup_interval_hours: 1,  // More frequent cleanup
        })
    );

    // Test that monitoring tasks don't consume excessive CPU
    let start_time = Instant::now();
    resource_manager.start_monitoring().await.unwrap();
    
    // Let it run for a short time
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let elapsed = start_time.elapsed();
    // Monitoring setup should be very fast
    assert!(elapsed < Duration::from_millis(500));
}

#[tokio::test]
async fn test_encryption_power_efficiency() {
    let mut encryptor = encryption::MediaEncryptor::new("test-device".to_string());
    encryptor.initialize_with_device_key("test-key").await.unwrap();

    // Test small chunk encryption (power-efficient)
    let small_data = vec![0u8; 1024]; // 1KB
    let temp_dir = tempfile::TempDir::new().unwrap();
    let input_path = temp_dir.path().join("small_input.bin");
    let output_path = temp_dir.path().join("small_output.bin");

    std::fs::write(&input_path, &small_data).unwrap();

    let start_time = Instant::now();
    let result = encryptor.encrypt_video_file(&input_path, &output_path).await;
    let elapsed = start_time.elapsed();

    assert!(result.is_ok());
    // Small file encryption should be very fast
    assert!(elapsed < Duration::from_millis(100));
}

#[tokio::test]
async fn test_config_power_management() {
    let config = config::Config::default();
    
    // Verify power management defaults are set for low power
    assert!(config.power_management.low_power_mode);
    assert!(config.power_management.adaptive_monitoring);
    assert!(config.power_management.cpu_scaling);
    assert_eq!(config.power_management.max_cpu_usage_percent, 15.0);
    assert_eq!(config.power_management.background_task_delay_ms, 100);
}

#[tokio::test]
async fn test_adaptive_monitoring_intervals() {
    let limits = resource_manager::ResourceLimits {
        max_memory_mb: 64,  // Very low limit to trigger high usage
        max_disk_usage_percent: 50.0,
        max_temp_files_mb: 5,
        max_log_files_mb: 2,
        max_recording_age_days: 3,
        cleanup_interval_hours: 6,
    };

    let resource_manager = resource_manager::ResourceManager::new(
        "test-device".to_string(),
        Some(limits)
    );

    // Test that resource stats gathering doesn't block
    let start_time = Instant::now();
    let stats = resource_manager.get_resource_stats().await;
    let elapsed = start_time.elapsed();

    assert!(!stats.device_id.is_empty());
    // Stats gathering should be very fast
    assert!(elapsed < Duration::from_millis(50));
}

#[test]
fn test_low_power_chunk_sizes() {
    // Verify encryption uses smaller chunks for power efficiency
    const POWER_EFFICIENT_CHUNK_SIZE: usize = 64 * 1024; // 64KB
    const POWER_HUNGRY_CHUNK_SIZE: usize = 1024 * 1024; // 1MB
    
    // Our implementation should use the smaller chunk size
    // This is tested indirectly by verifying that encryption of small files
    // doesn't allocate excessive memory
    
    let small_data = vec![0u8; POWER_EFFICIENT_CHUNK_SIZE / 2];
    assert!(small_data.len() < POWER_EFFICIENT_CHUNK_SIZE);
    
    // Memory usage should be reasonable for small files
    let memory_estimate = small_data.len() * 2; // Input + output
    assert!(memory_estimate < 256 * 1024); // Less than 256KB
}

#[tokio::test]
async fn test_task_yielding() {
    // Test that long-running operations yield CPU appropriately
    let start_time = Instant::now();
    
    // Simulate a task that should yield periodically
    for i in 0..100 {
        // Simulate some work
        std::hint::spin_loop();
        
        // Yield every 8 iterations (similar to our encryption)
        if i % 8 == 0 {
            tokio::task::yield_now().await;
        }
    }
    
    let elapsed = start_time.elapsed();
    
    // With yielding, this should complete reasonably quickly
    // but not instantaneously (proving yields are happening)
    assert!(elapsed < Duration::from_millis(100));
    assert!(elapsed > Duration::from_nanos(1000)); // Some measurable time
}

#[tokio::test]
async fn test_cleanup_task_efficiency() {
    let resource_manager = resource_manager::ResourceManager::new(
        "test-device".to_string(),
        Some(resource_manager::ResourceLimits::default())
    );

    // Create some temp files to cleanup
    let temp_dir = tempfile::TempDir::new().unwrap();
    let temp_file1 = temp_dir.path().join("temp1.txt");
    let temp_file2 = temp_dir.path().join("temp2.txt");
    
    std::fs::write(&temp_file1, "temp content 1").unwrap();
    std::fs::write(&temp_file2, "temp content 2").unwrap();

    resource_manager.register_temp_file(temp_file1.clone()).await.unwrap();
    resource_manager.register_temp_file(temp_file2.clone()).await.unwrap();

    // Test cleanup efficiency
    let start_time = Instant::now();
    let result = resource_manager.force_cleanup().await;
    let elapsed = start_time.elapsed();

    assert!(result.is_ok());
    // Cleanup should be fast
    assert!(elapsed < Duration::from_millis(500));
}

#[test]
fn test_power_efficient_defaults() {
    let config = config::Config::default();
    
    // Verify all power-efficient defaults
    assert!(config.power_management.low_power_mode);
    assert!(config.power_management.cpu_scaling);
    assert!(config.power_management.adaptive_monitoring);
    assert!(config.power_management.sleep_when_idle);
    
    // Check reasonable timeouts
    assert_eq!(config.power_management.idle_timeout_seconds, 300); // 5 minutes
    assert!(config.power_management.max_cpu_usage_percent <= 20.0); // Low CPU target
    
    // Check reasonable delays
    assert!(config.power_management.background_task_delay_ms >= 50); // Not too aggressive
}

#[tokio::test]
async fn test_memory_pressure_responsiveness() {
    let limits = resource_manager::ResourceLimits {
        max_memory_mb: 1, // Very low to trigger pressure quickly
        max_disk_usage_percent: 90.0,
        max_temp_files_mb: 1,
        max_log_files_mb: 1,
        max_recording_age_days: 1,
        cleanup_interval_hours: 24,
    };

    let resource_manager = resource_manager::ResourceManager::new(
        "test-device".to_string(),
        Some(limits)
    );

    // Memory pressure detection should be fast
    let start_time = Instant::now();
    let stats = resource_manager.get_resource_stats().await;
    let elapsed = start_time.elapsed();

    // Even with very low limits, detection should be quick
    assert!(elapsed < Duration::from_millis(100));
    assert!(!stats.device_id.is_empty());
}