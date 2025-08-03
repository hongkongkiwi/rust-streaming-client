# Rust Bodycam Client Testing Guide

## Power Efficiency & Testing Strategy

This document outlines our comprehensive testing approach optimized for low-power embedded systems.

## 🔋 Power Efficiency Optimizations

### CPU Optimizations
- **Adaptive monitoring intervals**: Monitoring frequency adjusts based on system load
- **Task yielding**: Long-running operations yield CPU every 8 iterations
- **Small chunk processing**: 64KB chunks instead of 1MB for encryption
- **Memory pre-allocation**: Vectors and strings pre-allocate capacity
- **CPU scaling**: Automatic frequency scaling in low-power mode

### Memory Optimizations
- **Efficient data structures**: HashMap for active processes instead of Vec
- **Memory pressure detection**: Adaptive monitoring based on usage
- **Automatic cleanup**: Temp files and old recordings cleaned periodically
- **Zero-copy operations**: Where possible, avoid unnecessary data copies
- **Smart resource limits**: Default to 128MB max memory for bodycams

### Network Optimizations
- **Exponential backoff**: Reduces network retry frequency when offline
- **Adaptive timeouts**: Longer timeouts during low-power periods
- **Batch operations**: Group multiple API calls when possible
- **Connection pooling**: Reuse HTTP connections

## 🧪 Testing Framework

### Test Types

#### 1. Unit Tests (`src/*/mod.rs`)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function() {
        // Test implementation
    }

    #[tokio::test]
    async fn test_async_function() {
        // Async test implementation
    }
}
```

#### 2. Integration Tests (`tests/`)
- `integration_tests.rs` - Cross-module functionality
- `power_efficiency_tests.rs` - Power consumption validation

#### 3. Benchmarks (`benches/`)
- `power_benchmarks.rs` - Performance and power consumption benchmarks

### Running Tests

#### Quick Test Run
```bash
cargo test
```

#### Comprehensive Test Suite
```bash
./scripts/test_runner.sh
```

#### Power Efficiency Tests Only
```bash
cargo test --test power_efficiency_tests
```

#### With Benchmarks
```bash
RUN_BENCHMARKS=true ./scripts/test_runner.sh
```

#### Coverage Report
```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html --output-dir coverage/
```

## 📊 Performance Benchmarks

### Running Benchmarks
```bash
cargo bench
```

Results are saved to `target/criterion/` with HTML reports.

### Key Metrics
- **Encryption throughput**: Target >10MB/s for 1080p video
- **Memory usage**: <128MB during normal operation
- **CPU usage**: <15% average, <25% peak
- **Battery life**: Target 8+ hours continuous recording

### Benchmark Categories
1. **Encryption Performance**: Chunk size optimization
2. **Resource Monitoring**: Frequency vs accuracy tradeoffs
3. **Input Validation**: Performance of security checks
4. **Memory Patterns**: Allocation efficiency
5. **Async Yielding**: Task cooperation

## 🔒 Security Testing

### Encryption Tests
```bash
cargo test encryption::tests
```

### Input Validation Tests
```bash
cargo test validation::tests
```

### Security Audit
```bash
cargo install cargo-audit
cargo audit
```

## 💾 Memory Testing

### Memory Leak Detection (Linux/macOS)
```bash
cargo install cargo-valgrind
cargo valgrind test
```

### Memory Usage Profiling
```bash
cargo install cargo-instruments  # macOS only
cargo instruments -t "Time Profiler" --bin bodycam-client
```

## ⚡ Power Testing Strategies

### 1. Configuration Validation
Verify power-efficient defaults:
```rust
#[test]
fn test_power_efficient_defaults() {
    let config = Config::default();
    assert!(config.power_management.low_power_mode);
    assert!(config.power_management.adaptive_monitoring);
    assert_eq!(config.power_management.max_cpu_usage_percent, 15.0);
}
```

### 2. Resource Usage Monitoring
```rust
#[tokio::test]
async fn test_resource_manager_efficiency() {
    let manager = ResourceManager::new("test".to_string(), None);
    let start = Instant::now();
    manager.start_monitoring().await.unwrap();
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_millis(100));
}
```

### 3. Task Yielding Verification
```rust
#[tokio::test]
async fn test_cpu_yielding() {
    // Verify long operations yield CPU appropriately
    for i in 0..1000 {
        do_work();
        if i % 8 == 0 {
            tokio::task::yield_now().await;
        }
    }
}
```

## 🏗️ Best Practices

### Writing Power-Efficient Tests
1. **Measure actual resource usage** in tests
2. **Use timeouts** to catch infinite loops
3. **Verify cleanup** after each test
4. **Test edge cases** like low battery, low memory
5. **Mock expensive operations** when testing logic

### Test Organization
```
tests/
├── integration_tests.rs       # Cross-module tests
├── power_efficiency_tests.rs  # Power consumption tests
├── security_tests.rs          # Security validation
└── hardware_simulation_tests.rs # Hardware mocking

benches/
└── power_benchmarks.rs        # Performance benchmarks

src/
├── lib.rs                     # Library exports for testing
└── */mod.rs                   # Module-specific unit tests
```

### Continuous Integration
```yaml
# .github/workflows/test.yml
name: Test Suite
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run tests
        run: ./scripts/test_runner.sh
      - name: Run benchmarks
        run: RUN_BENCHMARKS=true ./scripts/test_runner.sh
```

## 🔧 Debugging Power Issues

### 1. CPU Usage Monitoring
```bash
# Monitor CPU usage during tests
top -p $(pgrep bodycam-client)
```

### 2. Memory Usage Tracking
```bash
# Track memory usage
/usr/bin/time -v ./target/debug/bodycam-client
```

### 3. Power Profiling (macOS)
```bash
sudo powermetrics -i 1000 -n 10 --samplers cpu_power,gpu_power
```

### 4. Battery Testing
```bash
# Simulate low battery conditions
sudo echo 15 > /sys/class/power_supply/BAT0/capacity
```

## 📈 Performance Targets

### Low-Power Hardware Specifications
- **CPU**: ARM Cortex-A7 1.2GHz or equivalent
- **RAM**: 512MB-1GB
- **Storage**: 16GB+ eMMC/SD
- **Battery**: 3000mAh target 8+ hours

### Performance Goals
- **Startup time**: <5 seconds
- **Recording start**: <1 second
- **Memory usage**: <128MB steady state
- **CPU usage**: <15% average
- **Storage efficiency**: >90% usable space
- **Network efficiency**: <1MB/hour idle

## 🚀 Deployment Testing

### Pre-deployment Checklist
- [ ] All unit tests pass
- [ ] Integration tests pass
- [ ] Power efficiency tests pass
- [ ] Memory leak tests pass
- [ ] Security audit clean
- [ ] Performance benchmarks meet targets
- [ ] Hardware simulation tests pass

### Field Testing
1. **Real hardware validation** on target devices
2. **Long-term battery testing** (24+ hours)
3. **Thermal testing** under load
4. **Network connectivity testing** in poor conditions
5. **Storage endurance testing** with continuous recording

## 📚 Additional Resources

- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Tokio Performance Guide](https://tokio.rs/tokio/topics/performance)
- [Criterion Benchmarking Guide](https://bheisler.github.io/criterion.rs/book/)
- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)

---

This testing framework ensures our Rust bodycam client is optimized for low-power operation while maintaining security and reliability standards required for professional security monitoring systems.