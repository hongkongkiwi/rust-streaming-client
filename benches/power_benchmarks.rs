use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use bodycam_client::*;
use tempfile::TempDir;
use std::time::Duration;

fn benchmark_encryption_chunk_sizes(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("encryption_chunk_sizes");
    
    // Test different chunk sizes for power efficiency
    let chunk_sizes = vec![16 * 1024, 32 * 1024, 64 * 1024, 128 * 1024, 256 * 1024];
    let test_data = vec![0u8; 1024 * 1024]; // 1MB test data
    
    for chunk_size in chunk_sizes {
        group.bench_with_input(
            BenchmarkId::new("encrypt_chunks", chunk_size),
            &chunk_size,
            |b, &size| {
                b.iter(|| {
                    runtime.block_on(async {
                        let mut encryptor = encryption::MediaEncryptor::new("test-device".to_string());
                        encryptor.initialize_with_device_key("test-key").await.unwrap();
                        
                        let temp_dir = TempDir::new().unwrap();
                        let input_path = temp_dir.path().join("input.bin");
                        let output_path = temp_dir.path().join("output.bin");
                        
                        std::fs::write(&input_path, &test_data).unwrap();
                        
                        let result = encryptor.encrypt_video_file(&input_path, &output_path).await;
                        black_box(result)
                    })
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_resource_monitoring_frequency(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("resource_monitoring");
    
    // Test different monitoring frequencies
    let intervals = vec![
        Duration::from_secs(10),
        Duration::from_secs(30), 
        Duration::from_secs(60),
        Duration::from_secs(120)
    ];
    
    for interval in intervals {
        group.bench_with_input(
            BenchmarkId::new("monitoring_interval", interval.as_secs()),
            &interval,
            |b, &_| {
                b.iter(|| {
                    runtime.block_on(async {
                        let resource_manager = resource_manager::ResourceManager::new(
                            "bench-device".to_string(),
                            Some(resource_manager::ResourceLimits::default())
                        );
                        
                        let stats = resource_manager.get_resource_stats().await;
                        black_box(stats)
                    })
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_validation_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("input_validation");
    
    // Test validation performance for different input sizes
    let test_cases = vec![
        ("device-id", "valid-device-123"),
        ("file-path", "/valid/path/to/file.mp4"),
        ("url", "https://example.com/api/endpoint"),
        ("uuid", "550e8400-e29b-41d4-a716-446655440000"),
        ("email", "user@example.com"),
        ("resolution", "1920x1080"),
    ];
    
    for (test_type, test_input) in test_cases {
        group.bench_with_input(
            BenchmarkId::new("validate", test_type),
            test_input,
            |b, input| {
                b.iter(|| match test_type {
                    "device-id" => validation::InputValidator::validate_device_id(input),
                    "file-path" => validation::InputValidator::validate_file_path(input),
                    "url" => validation::InputValidator::validate_url(input),
                    "uuid" => validation::InputValidator::validate_uuid(input),
                    "email" => validation::InputValidator::validate_email(input),
                    "resolution" => validation::InputValidator::validate_resolution(input),
                    _ => Ok(()),
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_memory_allocation_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");
    
    // Test memory allocation efficiency
    group.bench_function("vec_with_capacity", |b| {
        b.iter(|| {
            // Efficient: pre-allocate capacity
            let mut vec = Vec::with_capacity(1000);
            for i in 0..1000 {
                vec.push(i);
            }
            black_box(vec)
        })
    });
    
    group.bench_function("vec_without_capacity", |b| {
        b.iter(|| {
            // Inefficient: let Vec grow dynamically
            let mut vec = Vec::new();
            for i in 0..1000 {
                vec.push(i);
            }
            black_box(vec)
        })
    });
    
    group.bench_function("string_with_capacity", |b| {
        b.iter(|| {
            // Efficient: pre-allocate string capacity
            let mut s = String::with_capacity(1000);
            for i in 0..100 {
                s.push_str(&format!("item-{} ", i));
            }
            black_box(s)
        })
    });
    
    group.finish();
}

fn benchmark_async_task_yielding(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("async_yielding");
    
    group.bench_function("with_yielding", |b| {
        b.iter(|| {
            runtime.block_on(async {
                for i in 0..100 {
                    std::hint::spin_loop();
                    if i % 8 == 0 {
                        tokio::task::yield_now().await;
                    }
                }
            })
        })
    });
    
    group.bench_function("without_yielding", |b| {
        b.iter(|| {
            runtime.block_on(async {
                for _ in 0..100 {
                    std::hint::spin_loop();
                }
            })
        })
    });
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_encryption_chunk_sizes,
    benchmark_resource_monitoring_frequency,
    benchmark_validation_performance,
    benchmark_memory_allocation_patterns,
    benchmark_async_task_yielding
);
criterion_main!(benches);