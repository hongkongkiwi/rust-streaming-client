use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Duration, Instant};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct ConnectionState {
    pub is_connected: bool,
    pub last_successful_connection: Option<chrono::DateTime<chrono::Utc>>,
    pub consecutive_failures: u32,
    pub total_failures: u64,
    pub reconnect_attempts: u32,
    pub next_retry_at: Option<Instant>,
    pub current_backoff: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkHealth {
    pub latency_ms: Option<u64>,
    pub packet_loss: Option<f32>,
    pub bandwidth_kbps: Option<u32>,
    pub dns_resolution_time: Option<u64>,
    pub last_measured: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct PendingOperation {
    pub id: String,
    pub operation_type: OperationType,
    pub payload: Vec<u8>,
    pub retry_count: u32,
    pub created_at: Instant,
    pub priority: OperationPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationType {
    StatusUpdate,
    IncidentReport,
    MediaUpload,
    ConfigSync,
    HeartBeat,
    StreamStart,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum OperationPriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

pub struct RecoveryManager {
    state: Arc<RwLock<ConnectionState>>,
    network_health: Arc<RwLock<NetworkHealth>>,
    pending_operations: Arc<Mutex<VecDeque<PendingOperation>>>,
    config: RecoveryConfig,
    client: reqwest::Client,
    server_url: String,
}

#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    pub max_retry_attempts: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub backoff_multiplier: f64,
    pub connection_timeout: Duration,
    pub health_check_interval: Duration,
    pub max_pending_operations: usize,
    pub operation_timeout: Duration,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_retry_attempts: 5,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(300), // 5 minutes
            backoff_multiplier: 2.0,
            connection_timeout: Duration::from_secs(30),
            health_check_interval: Duration::from_secs(60),
            max_pending_operations: 1000,
            operation_timeout: Duration::from_secs(300), // 5 minutes
        }
    }
}

impl RecoveryManager {
    pub fn new(server_url: String, config: Option<RecoveryConfig>) -> Self {
        let config = config.unwrap_or_default();
        
        let client = reqwest::Client::builder()
            .timeout(config.connection_timeout)
            .connect_timeout(config.connection_timeout)
            .build()
            .expect("Failed to create HTTP client");

        let initial_state = ConnectionState {
            is_connected: false,
            last_successful_connection: None,
            consecutive_failures: 0,
            total_failures: 0,
            reconnect_attempts: 0,
            next_retry_at: None,
            current_backoff: config.initial_backoff,
        };

        let initial_health = NetworkHealth {
            latency_ms: None,
            packet_loss: None,
            bandwidth_kbps: None,
            dns_resolution_time: None,
            last_measured: chrono::Utc::now(),
        };

        Self {
            state: Arc::new(RwLock::new(initial_state)),
            network_health: Arc::new(RwLock::new(initial_health)),
            pending_operations: Arc::new(Mutex::new(VecDeque::new())),
            config,
            client,
            server_url,
        }
    }

    pub async fn start_monitoring(&self) -> Result<()> {
        let state = Arc::clone(&self.state);
        let health = Arc::clone(&self.network_health);
        let pending = Arc::clone(&self.pending_operations);
        let config = self.config.clone();
        let client = self.client.clone();
        let server_url = self.server_url.clone();

        // Start connection monitoring task
        let monitor_state = Arc::clone(&state);
        let monitor_client = client.clone();
        let monitor_server_url = server_url.clone();
        let monitor_config = config.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.health_check_interval);
            
            loop {
                interval.tick().await;
                
                if let Err(e) = Self::check_connection_health(
                    &monitor_state,
                    &monitor_client,
                    &monitor_server_url,
                ).await {
                    tracing::warn!("Connection health check failed: {}", e);
                    Self::handle_connection_failure(&monitor_state, &monitor_config).await;
                } else {
                    Self::handle_connection_success(&monitor_state).await;
                }
            }
        });

        // Start retry mechanism for pending operations
        let retry_state = Arc::clone(&state);
        let retry_pending = Arc::clone(&pending);
        let retry_client = client.clone();
        let retry_server_url = server_url.clone();
        let retry_config = config.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            
            loop {
                interval.tick().await;
                
                let is_connected = retry_state.read().await.is_connected;
                if is_connected {
                    Self::process_pending_operations(
                        &retry_pending,
                        &retry_client,
                        &retry_server_url,
                        &retry_config,
                    ).await;
                }
            }
        });

        // Start network health monitoring
        let health_client = client.clone();
        let health_server_url = server_url.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                if let Ok(health_data) = Self::measure_network_health(&health_client, &health_server_url).await {
                    *health.write().await = health_data;
                }
            }
        });

        // Start cleanup task for old operations
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(120));
            
            loop {
                interval.tick().await;
                Self::cleanup_expired_operations(&pending, &config).await;
            }
        });

        Ok(())
    }

    pub async fn queue_operation(&self, operation: PendingOperation) -> Result<()> {
        let mut queue = self.pending_operations.lock().await;
        
        // Check if queue is full
        if queue.len() >= self.config.max_pending_operations {
            // Remove lowest priority items if needed
            queue.retain(|op| op.priority >= OperationPriority::High);
            
            if queue.len() >= self.config.max_pending_operations {
                return Err(anyhow::anyhow!("Operation queue is full"));
            }
        }
        
        // Insert operation in priority order
        let insert_pos = queue.binary_search_by(|op| op.priority.cmp(&operation.priority).reverse())
            .unwrap_or_else(|pos| pos);
        queue.insert(insert_pos, operation);
        
        Ok(())
    }

    pub async fn execute_with_recovery<F, T>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>> + Send + Sync,
        T: Send,
    {
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < self.config.max_retry_attempts {
            match operation().await {
                Ok(result) => {
                    self.handle_operation_success().await;
                    return Ok(result);
                }
                Err(e) => {
                    last_error = Some(e);
                    attempts += 1;
                    
                    if attempts < self.config.max_retry_attempts {
                        let backoff = self.calculate_backoff(attempts).await;
                        tracing::warn!("Operation failed (attempt {}), retrying in {:?}: {}", 
                            attempts, backoff, last_error.as_ref().unwrap());
                        tokio::time::sleep(backoff).await;
                    }
                }
            }
        }

        self.handle_operation_failure().await;
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Operation failed after {} attempts", attempts)))
    }

    async fn check_connection_health(
        state: &Arc<RwLock<ConnectionState>>,
        client: &reqwest::Client,
        server_url: &str,
    ) -> Result<()> {
        let health_url = format!("{}/api/health", server_url);
        
        let response = client
            .get(&health_url)
            .send()
            .await
            .context("Health check request failed")?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Health check returned status: {}", response.status()))
        }
    }

    async fn handle_connection_success(state: &Arc<RwLock<ConnectionState>>) {
        let mut conn_state = state.write().await;
        conn_state.is_connected = true;
        conn_state.last_successful_connection = Some(chrono::Utc::now());
        conn_state.consecutive_failures = 0;
        conn_state.current_backoff = Duration::from_secs(1); // Reset backoff
        conn_state.next_retry_at = None;
    }

    async fn handle_connection_failure(
        state: &Arc<RwLock<ConnectionState>>,
        config: &RecoveryConfig,
    ) {
        let mut conn_state = state.write().await;
        conn_state.is_connected = false;
        conn_state.consecutive_failures += 1;
        conn_state.total_failures += 1;
        
        // Calculate exponential backoff
        let backoff_secs = (config.initial_backoff.as_secs() as f64 
            * config.backoff_multiplier.powi(conn_state.consecutive_failures as i32)) as u64;
        conn_state.current_backoff = Duration::from_secs(backoff_secs.min(config.max_backoff.as_secs()));
        conn_state.next_retry_at = Some(Instant::now() + conn_state.current_backoff);
    }

    async fn handle_operation_success(&self) {
        // Update connection state on successful operation
        let mut state = self.state.write().await;
        state.is_connected = true;
        state.last_successful_connection = Some(chrono::Utc::now());
        if state.consecutive_failures > 0 {
            state.consecutive_failures = 0;
            state.current_backoff = self.config.initial_backoff;
        }
    }

    async fn handle_operation_failure(&self) {
        let mut state = self.state.write().await;
        state.consecutive_failures += 1;
        state.total_failures += 1;
    }

    async fn calculate_backoff(&self, attempt: u32) -> Duration {
        let state = self.state.read().await;
        let backoff_secs = (self.config.initial_backoff.as_secs() as f64 
            * self.config.backoff_multiplier.powi(attempt as i32)) as u64;
        Duration::from_secs(backoff_secs.min(self.config.max_backoff.as_secs()))
    }

    async fn process_pending_operations(
        pending: &Arc<Mutex<VecDeque<PendingOperation>>>,
        client: &reqwest::Client,
        server_url: &str,
        config: &RecoveryConfig,
    ) {
        let mut queue = pending.lock().await;
        let mut completed_ops = Vec::new();
        
        for (index, operation) in queue.iter_mut().enumerate() {
            if operation.retry_count >= config.max_retry_attempts {
                completed_ops.push(index);
                continue;
            }
            
            // Try to execute the operation
            match Self::execute_pending_operation(operation, client, server_url).await {
                Ok(_) => {
                    tracing::info!("Successfully executed pending operation: {}", operation.id);
                    completed_ops.push(index);
                }
                Err(e) => {
                    operation.retry_count += 1;
                    tracing::warn!("Failed to execute pending operation {} (attempt {}): {}", 
                        operation.id, operation.retry_count, e);
                }
            }
        }
        
        // Remove completed operations (in reverse order to maintain indices)
        for &index in completed_ops.iter().rev() {
            queue.remove(index);
        }
    }

    async fn execute_pending_operation(
        operation: &PendingOperation,
        client: &reqwest::Client,
        server_url: &str,
    ) -> Result<()> {
        let url = match operation.operation_type {
            OperationType::StatusUpdate => format!("{}/api/devices/status", server_url),
            OperationType::IncidentReport => format!("{}/api/incidents", server_url),
            OperationType::MediaUpload => format!("{}/api/media/upload", server_url),
            OperationType::ConfigSync => format!("{}/api/devices/config", server_url),
            OperationType::HeartBeat => format!("{}/api/devices/heartbeat", server_url),
            OperationType::StreamStart => format!("{}/api/streaming/start", server_url),
        };

        let response = client
            .post(&url)
            .body(operation.payload.clone())
            .header("Content-Type", "application/json")
            .send()
            .await
            .context("Failed to send pending operation")?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Operation failed with status: {}", response.status()))
        }
    }

    async fn cleanup_expired_operations(
        pending: &Arc<Mutex<VecDeque<PendingOperation>>>,
        config: &RecoveryConfig,
    ) {
        let mut queue = pending.lock().await;
        let now = Instant::now();
        
        queue.retain(|op| {
            let age = now.duration_since(op.created_at);
            age < config.operation_timeout
        });
    }

    async fn measure_network_health(
        client: &reqwest::Client,
        server_url: &str,
    ) -> Result<NetworkHealth> {
        let start = Instant::now();
        let health_url = format!("{}/api/health", server_url);
        
        let response = client
            .get(&health_url)
            .send()
            .await
            .context("Network health measurement failed")?;
        
        let latency = start.elapsed();
        
        Ok(NetworkHealth {
            latency_ms: Some(latency.as_millis() as u64),
            packet_loss: None, // Would need more sophisticated measurement
            bandwidth_kbps: None, // Would need bandwidth test
            dns_resolution_time: None, // Would need DNS timing
            last_measured: chrono::Utc::now(),
        })
    }

    pub async fn get_connection_state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    pub async fn get_network_health(&self) -> NetworkHealth {
        self.network_health.read().await.clone()
    }

    pub async fn get_pending_operations_count(&self) -> usize {
        self.pending_operations.lock().await.len()
    }

    pub async fn force_reconnect(&self) -> Result<()> {
        let mut state = self.state.write().await;
        state.is_connected = false;
        state.consecutive_failures = 0;
        state.current_backoff = self.config.initial_backoff;
        state.next_retry_at = Some(Instant::now());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_recovery_manager_creation() {
        let manager = RecoveryManager::new("http://localhost:3000".to_string(), None);
        let state = manager.get_connection_state().await;
        assert!(!state.is_connected);
        assert_eq!(state.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn test_operation_queuing() {
        let manager = RecoveryManager::new("http://localhost:3000".to_string(), None);
        
        let operation = PendingOperation {
            id: Uuid::new_v4().to_string(),
            operation_type: OperationType::StatusUpdate,
            payload: vec![1, 2, 3],
            retry_count: 0,
            created_at: Instant::now(),
            priority: OperationPriority::Normal,
        };
        
        assert!(manager.queue_operation(operation).await.is_ok());
        assert_eq!(manager.get_pending_operations_count().await, 1);
    }
}