# Sentry Integration for Rust Body Camera Client

## Overview

The Rust body camera client now includes comprehensive Sentry integration for error tracking, performance monitoring, and operational observability. This integration provides real-time visibility into device operations, critical incidents, and system health.

## Features

### 1. Error Tracking
- **Automatic error capture**: All panics and errors are automatically sent to Sentry
- **Contextual information**: Each error includes device ID, operation context, and relevant metadata
- **Error categorization**: Custom error types with appropriate severity levels
- **Stack traces**: Full call stack information for debugging

### 2. Performance Monitoring
- **Transaction tracking**: Key operations like device registration, recording start/stop, incident triggering
- **Performance metrics**: Operation duration, success/failure rates
- **Custom instrumentation**: Device-specific performance monitoring

### 3. Breadcrumb Tracking
- **Operation tracking**: Detailed breadcrumb trail of device operations
- **Hardware events**: Camera, audio, GPS, and sensor status changes
- **Network operations**: API calls, streaming status, connectivity
- **User actions**: Button presses, configuration changes

### 4. Device Context
- **Device identification**: Automatic tagging with device ID, site ID, tenant ID
- **Hardware information**: Platform, architecture, hardware capabilities
- **Runtime context**: Rust version, application version, environment

## Configuration

### Environment Variables

```bash
# Required
SENTRY_DSN=https://your-sentry-dsn@o1234567.ingest.sentry.io/1234567

# Optional
SENTRY_ENVIRONMENT=development  # development, staging, production
SENTRY_RELEASE=bodycam-client@1.0.0
SENTRY_SAMPLE_RATE=1.0  # 0.0 to 1.0 (100% in development, lower in production)
SENTRY_TRACES_SAMPLE_RATE=0.1  # Performance monitoring sample rate
SENTRY_ENABLE_TRACING=true
SENTRY_ATTACH_STACKTRACE=true
SENTRY_DEBUG=false
```

### Configuration File

Add to `config.toml`:

```toml
[sentry]
dsn = "https://your-sentry-dsn@o1234567.ingest.sentry.io/1234567"
environment = "development"
sample_rate = 1.0
traces_sample_rate = 0.1
enable_tracing = true
debug = false
```

## Usage Examples

### Basic Error Capture

```rust
use crate::sentry_capture_error;

async fn some_operation() -> Result<()> {
    match risky_operation().await {
        Ok(result) => Ok(result),
        Err(e) => {
            sentry_capture_error!(&e, "operation" => "risky_operation", "context" => "additional_info");
            Err(e)
        }
    }
}
```

### Performance Monitoring

```rust
use crate::sentry_integration;

async fn critical_operation() -> Result<()> {
    let _transaction = sentry_integration::start_transaction("device.critical_operation", "device");
    
    sentry_integration::add_device_breadcrumb("operation_start", Some("critical"));
    
    // Perform operation...
    
    sentry_integration::add_device_breadcrumb("operation_complete", Some("success"));
    Ok(())
}
```

### Custom Error Handling

```rust
use crate::error_handling::{DeviceError, DeviceOperationWrapper};

async fn device_operation() -> Result<()> {
    let wrapper = DeviceOperationWrapper::new("register_device", Some(device_id));
    
    wrapper.execute_device_operation(async {
        // Your device operation here
        if !device_ready {
            return Err(DeviceError::Hardware { 
                message: "Camera not initialized".to_string() 
            });
        }
        Ok(())
    }).await
}
```

### Convenience Macros

```rust
use crate::{device_operation, sentry_capture_message};

// Wrap any async operation
let result = device_operation!("start_recording", Some(device_id), async {
    recorder.start().await
}).await?;

// Capture custom messages
sentry_capture_message!(
    "Device battery critically low", 
    sentry::Level::Fatal,
    "battery_level" => battery_level,
    "device_id" => device_id
);
```

## Integration Points

### 1. Application Startup
- Sentry is initialized in `main.rs` after configuration loading
- Device context is set when device ID becomes available
- Panic handler is installed for crash reporting

### 2. Device Operations
- **Registration**: Full transaction tracking with success/failure reporting
- **Recording**: Start/stop operations with performance monitoring
- **Incidents**: Automatic incident reporting to Sentry as warnings
- **Streaming**: Network operation monitoring
- **Hardware Events**: Sensor and hardware state changes

### 3. Error Categories
- **Hardware**: Camera, microphone, GPS, sensor failures
- **Network**: Connectivity, API call failures
- **Storage**: Disk space, file operations
- **Power**: Battery critical levels
- **Authentication**: Device provisioning, credential issues
- **Configuration**: Invalid settings, missing parameters

## Production Considerations

### 1. Sampling Rates
- **Development**: 100% error capture, 100% performance monitoring
- **Staging**: 100% error capture, 20% performance monitoring
- **Production**: 10% error capture, 5% performance monitoring

### 2. Data Privacy
- **Sensitive data filtering**: Automatic removal of credentials, keys, personal data
- **Form input masking**: Session replays mask sensitive inputs
- **Custom filtering**: Add new sensitive data patterns as needed

### 3. Performance Impact
- **Async operations**: All Sentry operations are non-blocking
- **Buffering**: Events are buffered and sent in batches
- **Graceful degradation**: Continues operation if Sentry is unavailable

## Debugging

### Enable Debug Mode
```bash
SENTRY_DEBUG=true
```

### View Local Events
Debug mode shows Sentry events in console before they're sent.

### Test Configuration
```bash
# Run with verbose logging
RUST_LOG=debug ./bodycam-client diagnose
```

## Security

### 1. DSN Protection
- Store DSN in environment variables or secure configuration
- Never commit DSN to version control
- Use different DSNs for different environments

### 2. Data Sanitization
- Automatic filtering of authentication headers
- Removal of API keys from request data
- Custom filters for domain-specific sensitive data

### 3. Access Control
- Limit Sentry project access to authorized team members
- Use Sentry's role-based permissions
- Regular audit of access permissions

## Monitoring and Alerting

### 1. Critical Alerts
- Battery critical levels
- Hardware failures
- Authentication failures
- Storage exhaustion

### 2. Performance Monitoring
- Operation duration trends
- Error rate increases
- Resource utilization spikes

### 3. Custom Dashboards
- Device health overview
- Incident response metrics
- Hardware performance trends

## Troubleshooting

### Common Issues

1. **Sentry not receiving events**
   - Check DSN configuration
   - Verify network connectivity
   - Enable debug mode to see local events

2. **High event volume**
   - Adjust sample rates
   - Add custom filtering
   - Review error patterns

3. **Missing context**
   - Ensure device context is set after registration
   - Check breadcrumb generation
   - Verify transaction scope

### Log Analysis
```bash
# Check for Sentry initialization
grep "Sentry initialization" logs/

# Check for error patterns
grep "capture_error" logs/

# Verify device context
grep "set_device_context" logs/
```

## Future Enhancements

1. **Custom Metrics**: Device-specific metrics (battery drain rate, recording quality)
2. **Release Tracking**: Automatic deployment tracking
3. **User Feedback**: Integration with device feedback mechanisms
4. **Advanced Filtering**: ML-based error classification
5. **Performance Baselines**: Automated performance regression detection

## Support

For issues with Sentry integration:
1. Check this documentation
2. Review Sentry project configuration
3. Examine device logs for Sentry-related messages
4. Test with debug mode enabled
5. Contact development team with specific error details