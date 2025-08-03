use anyhow::Result;
use sentry::ClientOptions;
use std::env;
use tracing::{info, warn, error};

/// Sentry configuration for the Rust body camera client
#[derive(Debug, Clone)]
pub struct SentryConfig {
    pub dsn: Option<String>,
    pub environment: String,
    pub release: String,
    pub sample_rate: f32,
    pub traces_sample_rate: f32,
    pub enable_tracing: bool,
    pub attach_stacktrace: bool,
    pub debug: bool,
}

impl Default for SentryConfig {
    fn default() -> Self {
        Self {
            dsn: None,
            environment: "development".to_string(),
            release: env!("CARGO_PKG_VERSION").to_string(),
            sample_rate: 1.0,
            traces_sample_rate: 0.1,
            enable_tracing: true,
            attach_stacktrace: true,
            debug: false,
        }
    }
}

impl SentryConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        let environment = env::var("SENTRY_ENVIRONMENT")
            .or_else(|_| env::var("ENVIRONMENT"))
            .unwrap_or_else(|_| "development".to_string());
        
        let debug = environment == "development";
        
        Self {
            dsn: env::var("SENTRY_DSN").ok(),
            environment,
            release: env::var("SENTRY_RELEASE")
                .unwrap_or_else(|_| format!("bodycam-client@{}", env!("CARGO_PKG_VERSION"))),
            sample_rate: env::var("SENTRY_SAMPLE_RATE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(if debug { 1.0 } else { 0.1 }),
            traces_sample_rate: env::var("SENTRY_TRACES_SAMPLE_RATE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(if debug { 1.0 } else { 0.05 }),
            enable_tracing: env::var("SENTRY_ENABLE_TRACING")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
            attach_stacktrace: env::var("SENTRY_ATTACH_STACKTRACE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
            debug,
        }
    }
    
    /// Create configuration from config file values
    pub fn from_config(config: &crate::config::Config) -> Self {
        let mut sentry_config = Self::from_env();
        
        // Override with config file values if present
        if let Some(ref sentry) = config.sentry {
            if let Some(ref dsn) = sentry.dsn {
                sentry_config.dsn = Some(dsn.clone());
            }
            if let Some(ref env) = sentry.environment {
                sentry_config.environment = env.clone();
            }
            if let Some(sample_rate) = sentry.sample_rate {
                sentry_config.sample_rate = sample_rate;
            }
            if let Some(traces_sample_rate) = sentry.traces_sample_rate {
                sentry_config.traces_sample_rate = traces_sample_rate;
            }
            if let Some(enable_tracing) = sentry.enable_tracing {
                sentry_config.enable_tracing = enable_tracing;
            }
            if let Some(debug) = sentry.debug {
                sentry_config.debug = debug;
            }
        }
        
        sentry_config
    }
}

/// Initialize Sentry with the given configuration
pub fn init_sentry(config: &SentryConfig) -> Result<Option<sentry::ClientInitGuard>> {
    let dsn = match &config.dsn {
        Some(dsn) => dsn,
        None => {
            warn!("Sentry DSN not configured, error tracking disabled");
            return Ok(None);
        }
    };
    
    info!("Initializing Sentry error tracking...");
    info!("Environment: {}", config.environment);
    info!("Release: {}", config.release);
    
    let options = ClientOptions {
        dsn: Some(dsn.parse()?),
        environment: Some(config.environment.clone().into()),
        release: Some(config.release.clone().into()),
        sample_rate: config.sample_rate,
        traces_sample_rate: config.traces_sample_rate,
        attach_stacktrace: config.attach_stacktrace,
        debug: config.debug,
        ..Default::default()
    };
    
    let guard = sentry::init(options);
    
    // Configure user context for device identification
    sentry::configure_scope(|scope| {
        scope.set_tag("component", "rust-client");
        scope.set_tag("platform", std::env::consts::OS);
        scope.set_tag("architecture", std::env::consts::ARCH);
        scope.set_context("runtime", sentry::protocol::Context::Other({
            let mut map = std::collections::BTreeMap::new();
            map.insert("name".to_string(), "rust".into());
            map.insert("version".to_string(), env!("CARGO_PKG_RUST_VERSION").into());
            map
        }));
    });
    
    // Set up panic handler - Sentry handles this automatically with the panic feature
    // The panic integration is enabled via the "panic" feature flag in Cargo.toml
    
    // Initialize tracing integration if enabled
    if config.enable_tracing {
        // The sentry tracing integration is automatically enabled via the feature flag
        info!("Sentry tracing integration enabled");
    }
    
    info!("Sentry initialization complete");
    Ok(Some(guard))
}

/// Set device context in Sentry scope
pub fn set_device_context(device_id: Option<&str>, site_id: Option<&str>, tenant_id: Option<&str>) {
    sentry::configure_scope(|scope| {
        if let Some(device_id) = device_id {
            scope.set_user(Some(sentry::User {
                id: Some(device_id.to_string()),
                ..Default::default()
            }));
            scope.set_tag("device_id", device_id);
        }
        
        if let Some(site_id) = site_id {
            scope.set_tag("site_id", site_id);
        }
        
        if let Some(tenant_id) = tenant_id {
            scope.set_tag("tenant_id", tenant_id);
        }
        
        scope.set_context("device", sentry::protocol::Context::Other({
            let mut map = std::collections::BTreeMap::new();
            if let Some(device_id) = device_id {
                map.insert("id".to_string(), device_id.into());
            }
            if let Some(site_id) = site_id {
                map.insert("site_id".to_string(), site_id.into());
            }
            if let Some(tenant_id) = tenant_id {
                map.insert("tenant_id".to_string(), tenant_id.into());
            }
            map
        }));
    });
}

/// Add breadcrumb for operation tracking
pub fn add_breadcrumb(message: &str, category: &str, level: sentry::Level) {
    sentry::add_breadcrumb(sentry::Breadcrumb {
        message: Some(message.to_string()),
        category: Some(category.to_string()),
        level,
        timestamp: chrono::Utc::now().into(),
        ..Default::default()
    });
}

/// Add breadcrumb for device operations
pub fn add_device_breadcrumb(operation: &str, details: Option<&str>) {
    let message = if let Some(details) = details {
        format!("{}: {}", operation, details)
    } else {
        operation.to_string()
    };
    
    add_breadcrumb(&message, "device", sentry::Level::Info);
}

/// Add breadcrumb for hardware operations
pub fn add_hardware_breadcrumb(component: &str, operation: &str, status: &str) {
    let message = format!("{} {}: {}", component, operation, status);
    add_breadcrumb(&message, "hardware", sentry::Level::Info);
}

/// Add breadcrumb for network operations
pub fn add_network_breadcrumb(operation: &str, endpoint: Option<&str>, status: &str) {
    let message = if let Some(endpoint) = endpoint {
        format!("{} {}: {}", operation, endpoint, status)
    } else {
        format!("{}: {}", operation, status)
    };
    add_breadcrumb(&message, "network", sentry::Level::Info);
}

/// Capture custom message with context
pub fn capture_message_with_context(
    message: &str,
    level: sentry::Level,
    context: Option<std::collections::BTreeMap<String, sentry::protocol::Value>>,
) {
    sentry::with_scope(|scope| {
        if let Some(context) = context {
            scope.set_context("operation", sentry::protocol::Context::Other(context));
        }
        sentry::capture_message(message, level);
    });
}

/// Capture error with additional context
pub fn capture_error_with_context(
    error: &anyhow::Error,
    context: Option<std::collections::BTreeMap<String, sentry::protocol::Value>>,
) {
    sentry::with_scope(|scope| {
        if let Some(context) = context {
            scope.set_context("error_context", sentry::protocol::Context::Other(context));
        }
        
        // Add error chain information
        let mut error_chain = Vec::new();
        let mut current_error = error.source();
        while let Some(err) = current_error {
            error_chain.push(err.to_string());
            current_error = err.source();
        }
        
        if !error_chain.is_empty() {
            scope.set_context("error_chain", sentry::protocol::Context::Other({
                let mut map = std::collections::BTreeMap::new();
                map.insert("chain".to_string(), error_chain.into());
                map
            }));
        }
        
        sentry::capture_error(error);
    });
}

/// Start a performance transaction
pub fn start_transaction(name: &str, operation: &str) -> sentry::Transaction {
    let ctx = sentry::TransactionContext::new(name, operation);
    sentry::start_transaction(ctx)
}

/// Convenience macro for capturing errors with context
#[macro_export]
macro_rules! sentry_capture_error {
    ($error:expr) => {
        $crate::sentry_integration::capture_error_with_context($error, None)
    };
    ($error:expr, $($key:expr => $value:expr),+) => {
        {
            let mut context = std::collections::BTreeMap::new();
            $(
                context.insert($key.to_string(), $value.into());
            )+
            $crate::sentry_integration::capture_error_with_context($error, Some(context))
        }
    };
}

/// Convenience macro for capturing messages with context
#[macro_export]
macro_rules! sentry_capture_message {
    ($message:expr, $level:expr) => {
        $crate::sentry_integration::capture_message_with_context($message, $level, None)
    };
    ($message:expr, $level:expr, $($key:expr => $value:expr),+) => {
        {
            let mut context = std::collections::BTreeMap::new();
            $(
                context.insert($key.to_string(), $value.into());
            )+
            $crate::sentry_integration::capture_message_with_context($message, $level, Some(context))
        }
    };
}