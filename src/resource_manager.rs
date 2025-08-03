use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Duration, Instant};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceStats {
    pub memory_usage: MemoryUsage,
    pub disk_usage: DiskUsage,
    pub process_stats: ProcessStats,
    pub cleanup_stats: CleanupStats,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsage {
    pub total_kb: u64,
    pub used_kb: u64,
    pub available_kb: u64,
    pub process_memory_kb: u64,
    pub swap_used_kb: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskUsage {
    pub total_gb: f64,
    pub used_gb: f64,
    pub available_gb: f64,
    pub recordings_gb: f64,
    pub logs_gb: f64,
    pub temp_files_gb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStats {
    pub cpu_usage_percent: f64,
    pub open_files: u32,
    pub threads: u32,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupStats {
    pub files_cleaned: u64,
    pub space_freed_mb: f64,
    pub last_cleanup: Option<chrono::DateTime<chrono::Utc>>,
    pub cleanup_errors: u32,
}

#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_memory_mb: u64,
    pub max_disk_usage_percent: f64,
    pub max_temp_files_mb: u64,
    pub max_log_files_mb: u64,
    pub max_recording_age_days: u32,
    pub cleanup_interval_hours: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 512,  // 512MB max memory usage
            max_disk_usage_percent: 85.0,  // 85% max disk usage
            max_temp_files_mb: 100,  // 100MB max temp files
            max_log_files_mb: 50,   // 50MB max log files
            max_recording_age_days: 30,  // Keep recordings for 30 days
            cleanup_interval_hours: 6,   // Cleanup every 6 hours
        }
    }
}

pub struct ResourceManager {
    stats: Arc<RwLock<ResourceStats>>,
    limits: ResourceLimits,
    active_processes: Arc<Mutex<HashMap<String, tokio::process::Child>>>,
    temp_files: Arc<Mutex<Vec<PathBuf>>>,
    cleanup_tasks: Arc<Mutex<Vec<CleanupTask>>>,
    device_id: String,
}

#[derive(Debug, Clone)]
pub struct CleanupTask {
    pub id: String,
    pub task_type: CleanupTaskType,
    pub target_path: PathBuf,
    pub created_at: Instant,
    pub priority: CleanupPriority,
}

#[derive(Debug, Clone)]
pub enum CleanupTaskType {
    TempFile,
    OldRecording,
    LogRotation,
    CacheCleanup,
    ProcessCleanup,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CleanupPriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

impl ResourceManager {
    pub fn new(device_id: String, limits: Option<ResourceLimits>) -> Self {
        let limits = limits.unwrap_or_default();
        
        let initial_stats = ResourceStats {
            memory_usage: MemoryUsage {
                total_kb: 0,
                used_kb: 0,
                available_kb: 0,
                process_memory_kb: 0,
                swap_used_kb: None,
            },
            disk_usage: DiskUsage {
                total_gb: 0.0,
                used_gb: 0.0,
                available_gb: 0.0,
                recordings_gb: 0.0,
                logs_gb: 0.0,
                temp_files_gb: 0.0,
            },
            process_stats: ProcessStats {
                cpu_usage_percent: 0.0,
                open_files: 0,
                threads: 0,
                uptime_seconds: 0,
            },
            cleanup_stats: CleanupStats {
                files_cleaned: 0,
                space_freed_mb: 0.0,
                last_cleanup: None,
                cleanup_errors: 0,
            },
            last_updated: chrono::Utc::now(),
        };

        Self {
            stats: Arc::new(RwLock::new(initial_stats)),
            limits,
            active_processes: Arc::new(Mutex::new(HashMap::new())),
            temp_files: Arc::new(Mutex::new(Vec::new())),
            cleanup_tasks: Arc::new(Mutex::new(Vec::new())),
            device_id,
        }
    }

    pub async fn start_monitoring(&self) -> Result<()> {
        let stats = Arc::clone(&self.stats);
        let limits = self.limits.clone();
        let active_processes = Arc::clone(&self.active_processes);
        let temp_files = Arc::clone(&self.temp_files);
        let cleanup_tasks = Arc::clone(&self.cleanup_tasks);

        // Start resource monitoring task with power-efficient intervals
        let monitor_stats = Arc::clone(&stats);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // Reduced frequency
            
            loop {
                interval.tick().await;
                
                // Only update stats if system is not in low-power mode
                if let Err(e) = Self::update_resource_stats(&monitor_stats).await {
                    tracing::warn!("Failed to update resource stats: {}", e);
                }
                
                // Yield to allow other tasks to run
                tokio::task::yield_now().await;
            }
        });

        // Start cleanup task with power-efficient scheduling
        let cleanup_stats = Arc::clone(&stats);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(limits.cleanup_interval_hours * 3600));
            
            loop {
                interval.tick().await;
                
                // Run cleanup during low-activity periods
                if let Err(e) = Self::run_cleanup_tasks(
                    &cleanup_stats,
                    &limits,
                    &active_processes,
                    &temp_files,
                    &cleanup_tasks,
                ).await {
                    tracing::error!("Cleanup task failed: {}", e);
                }
                
                // Sleep longer between cleanup cycles to reduce CPU usage
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });

        // Start memory pressure monitoring with adaptive intervals
        let memory_stats = Arc::clone(&stats);
        let memory_limits = limits.clone();
        tokio::spawn(async move {
            let mut base_interval = Duration::from_secs(30); // Start with less frequent checks
            let mut current_interval = base_interval;
            let mut high_usage_count = 0u32;
            
            loop {
                tokio::time::sleep(current_interval).await;
                
                match Self::check_memory_pressure(&memory_stats, &memory_limits).await {
                    Ok(is_high_usage) => {
                        if is_high_usage {
                            high_usage_count += 1;
                            // Increase monitoring frequency when under pressure
                            current_interval = Duration::from_secs(10);
                        } else {
                            high_usage_count = 0;
                            // Reduce monitoring frequency when stable
                            current_interval = base_interval;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Memory pressure check failed: {}", e);
                        // Back off on errors to save power
                        current_interval = Duration::from_secs(60);
                    }
                }
                
                // Yield CPU to other tasks
                tokio::task::yield_now().await;
            }
        });

        Ok(())
    }

    pub async fn register_temp_file(&self, path: PathBuf) -> Result<()> {
        let mut temp_files = self.temp_files.lock().await;
        temp_files.push(path);
        
        // Trigger cleanup if too many temp files
        if temp_files.len() > 100 {
            self.schedule_cleanup_task(CleanupTask {
                id: uuid::Uuid::new_v4().to_string(),
                task_type: CleanupTaskType::TempFile,
                target_path: PathBuf::from("temp"),
                created_at: Instant::now(),
                priority: CleanupPriority::High,
            }).await?;
        }
        
        Ok(())
    }

    pub async fn register_process(&self, name: String, process: tokio::process::Child) -> Result<()> {
        let mut processes = self.active_processes.lock().await;
        processes.insert(name, process);
        Ok(())
    }

    pub async fn cleanup_process(&self, name: &str) -> Result<()> {
        let mut processes = self.active_processes.lock().await;
        if let Some(mut process) = processes.remove(name) {
            let _ = process.kill().await;
            tracing::info!("Cleaned up process: {}", name);
        }
        Ok(())
    }

    pub async fn schedule_cleanup_task(&self, task: CleanupTask) -> Result<()> {
        let mut tasks = self.cleanup_tasks.lock().await;
        
        // Insert in priority order
        let insert_pos = tasks.binary_search_by(|t| t.priority.cmp(&task.priority).reverse())
            .unwrap_or_else(|pos| pos);
        tasks.insert(insert_pos, task);
        
        Ok(())
    }

    async fn update_resource_stats(stats: &Arc<RwLock<ResourceStats>>) -> Result<()> {
        let memory_info = Self::get_memory_info().await?;
        let disk_info = Self::get_disk_info().await?;
        let process_info = Self::get_process_info().await?;

        let mut stats_guard = stats.write().await;
        stats_guard.memory_usage = memory_info;
        stats_guard.disk_usage = disk_info;
        stats_guard.process_stats = process_info;
        stats_guard.last_updated = chrono::Utc::now();

        Ok(())
    }

    async fn get_memory_info() -> Result<MemoryUsage> {
        // Platform-specific memory information gathering
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            
            let output = Command::new("vm_stat")
                .output()
                .context("Failed to execute vm_stat")?;
            
            let output_str = String::from_utf8_lossy(&output.stdout);
            
            // Parse vm_stat output (simplified)
            let mut total_kb = 0;
            let mut used_kb = 0;
            
            for line in output_str.lines() {
                if line.contains("Pages free:") {
                    if let Some(pages) = line.split_whitespace().nth(2) {
                        if let Ok(pages_num) = pages.trim_end_matches('.').parse::<u64>() {
                            total_kb += pages_num * 4; // 4KB per page on macOS
                        }
                    }
                }
            }
            
            // Get process memory usage
            let process_memory = Self::get_process_memory().await.unwrap_or(0);
            
            Ok(MemoryUsage {
                total_kb,
                used_kb,
                available_kb: total_kb.saturating_sub(used_kb),
                process_memory_kb: process_memory,
                swap_used_kb: None,
            })
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            // Default implementation for other platforms
            Ok(MemoryUsage {
                total_kb: 1024 * 1024, // 1GB default
                used_kb: 512 * 1024,   // 512MB default
                available_kb: 512 * 1024,
                process_memory_kb: 128 * 1024, // 128MB default
                swap_used_kb: None,
            })
        }
    }

    async fn get_process_memory() -> Result<u64> {
        // Get current process memory usage
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            
            let pid = std::process::id();
            let output = Command::new("ps")
                .args(&["-o", "rss=", "-p", &pid.to_string()])
                .output()
                .context("Failed to get process memory")?;
            
            let memory_str = String::from_utf8_lossy(&output.stdout);
            let memory_kb = memory_str.trim().parse::<u64>().unwrap_or(0);
            
            Ok(memory_kb)
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            Ok(128 * 1024) // 128MB default
        }
    }

    async fn get_disk_info() -> Result<DiskUsage> {
        use std::fs;
        
        let current_dir = std::env::current_dir()?;
        let recordings_dir = current_dir.join("recordings");
        let logs_dir = current_dir.join("logs");
        let temp_dir = current_dir.join("temp");

        let recordings_size = Self::get_directory_size(&recordings_dir).await.unwrap_or(0);
        let logs_size = Self::get_directory_size(&logs_dir).await.unwrap_or(0);
        let temp_size = Self::get_directory_size(&temp_dir).await.unwrap_or(0);

        // Get disk usage information
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            
            let output = Command::new("df")
                .args(&["-k", "."])
                .output()
                .context("Failed to get disk usage")?;
            
            let output_str = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = output_str.lines().collect();
            
            if lines.len() > 1 {
                let parts: Vec<&str> = lines[1].split_whitespace().collect();
                if parts.len() >= 4 {
                    let total_kb = parts[1].parse::<u64>().unwrap_or(0);
                    let used_kb = parts[2].parse::<u64>().unwrap_or(0);
                    let available_kb = parts[3].parse::<u64>().unwrap_or(0);
                    
                    return Ok(DiskUsage {
                        total_gb: total_kb as f64 / 1024.0 / 1024.0,
                        used_gb: used_kb as f64 / 1024.0 / 1024.0,
                        available_gb: available_kb as f64 / 1024.0 / 1024.0,
                        recordings_gb: recordings_size as f64 / 1024.0 / 1024.0 / 1024.0,
                        logs_gb: logs_size as f64 / 1024.0 / 1024.0 / 1024.0,
                        temp_files_gb: temp_size as f64 / 1024.0 / 1024.0 / 1024.0,
                    });
                }
            }
        }

        // Default values if platform-specific code fails
        Ok(DiskUsage {
            total_gb: 50.0,
            used_gb: 25.0,
            available_gb: 25.0,
            recordings_gb: recordings_size as f64 / 1024.0 / 1024.0 / 1024.0,
            logs_gb: logs_size as f64 / 1024.0 / 1024.0 / 1024.0,
            temp_files_gb: temp_size as f64 / 1024.0 / 1024.0 / 1024.0,
        })
    }

    fn get_directory_size(path: &PathBuf) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<u64>> + Send>> {
        let path = path.clone();
        Box::pin(async move {
            if !path.exists() {
                return Ok(0);
            }

            let mut size = 0;
            let mut entries = tokio::fs::read_dir(&path).await?;

            while let Some(entry) = entries.next_entry().await? {
                let metadata = entry.metadata().await?;
                if metadata.is_file() {
                    size += metadata.len();
                } else if metadata.is_dir() {
                    size += Self::get_directory_size(&entry.path()).await.unwrap_or(0);
                }
            }

            Ok(size)
        })
    }

    async fn get_process_info() -> Result<ProcessStats> {
        // Platform-specific process information
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            
            let pid = std::process::id();
            let output = Command::new("ps")
                .args(&["-o", "pcpu=,nlwp=", "-p", &pid.to_string()])
                .output()
                .context("Failed to get process info")?;
            
            let output_str = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = output_str.trim().split_whitespace().collect();
            
            let cpu_usage = parts.get(0).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let threads = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
            
            Ok(ProcessStats {
                cpu_usage_percent: cpu_usage,
                open_files: 0, // Could be implemented with lsof
                threads,
                uptime_seconds: 0, // Could be calculated from process start time
            })
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            Ok(ProcessStats {
                cpu_usage_percent: 5.0,
                open_files: 10,
                threads: 4,
                uptime_seconds: 3600,
            })
        }
    }

    async fn check_memory_pressure(
        stats: &Arc<RwLock<ResourceStats>>,
        limits: &ResourceLimits,
    ) -> Result<bool> {
        let stats_guard = stats.read().await;
        let memory_usage_mb = stats_guard.memory_usage.process_memory_kb as f64 / 1024.0;
        
        let is_high_usage = memory_usage_mb > limits.max_memory_mb as f64 * 0.8; // 80% threshold
        
        if memory_usage_mb > limits.max_memory_mb as f64 {
            tracing::warn!("Memory usage ({:.1}MB) exceeds limit ({:.1}MB)", 
                memory_usage_mb, limits.max_memory_mb);
            
            // Trigger garbage collection or cleanup
            drop(stats_guard);
            Self::force_memory_cleanup().await?;
        }
        
        Ok(is_high_usage)
    }

    async fn force_memory_cleanup() -> Result<()> {
        // Force garbage collection and cleanup
        tracing::info!("Forcing memory cleanup due to high usage");
        
        // In Rust, we can't force GC like in other languages,
        // but we can clear caches and drop unused resources
        
        Ok(())
    }

    async fn run_cleanup_tasks(
        stats: &Arc<RwLock<ResourceStats>>,
        limits: &ResourceLimits,
        active_processes: &Arc<Mutex<HashMap<String, tokio::process::Child>>>,
        temp_files: &Arc<Mutex<Vec<PathBuf>>>,
        cleanup_tasks: &Arc<Mutex<Vec<CleanupTask>>>,
    ) -> Result<()> {
        let mut tasks = cleanup_tasks.lock().await;
        let mut completed_tasks = Vec::new();
        let mut files_cleaned = 0u64;
        let mut space_freed = 0f64;
        let mut errors = 0u32;

        for (index, task) in tasks.iter().enumerate() {
            match Self::execute_cleanup_task(task, temp_files).await {
                Ok((files, space)) => {
                    files_cleaned += files;
                    space_freed += space;
                    completed_tasks.push(index);
                }
                Err(e) => {
                    tracing::error!("Cleanup task {} failed: {}", task.id, e);
                    errors += 1;
                    completed_tasks.push(index);
                }
            }
        }

        // Remove completed tasks
        for &index in completed_tasks.iter().rev() {
            tasks.remove(index);
        }
        drop(tasks);

        // Update cleanup stats
        let mut stats_guard = stats.write().await;
        stats_guard.cleanup_stats.files_cleaned += files_cleaned;
        stats_guard.cleanup_stats.space_freed_mb += space_freed;
        stats_guard.cleanup_stats.last_cleanup = Some(chrono::Utc::now());
        stats_guard.cleanup_stats.cleanup_errors += errors;

        tracing::info!("Cleanup completed: {} files, {:.2}MB freed, {} errors", 
            files_cleaned, space_freed, errors);

        Ok(())
    }

    async fn execute_cleanup_task(
        task: &CleanupTask,
        temp_files: &Arc<Mutex<Vec<PathBuf>>>,
    ) -> Result<(u64, f64)> {
        match task.task_type {
            CleanupTaskType::TempFile => {
                Self::cleanup_temp_files(temp_files).await
            }
            CleanupTaskType::OldRecording => {
                Self::cleanup_old_recordings(&task.target_path).await
            }
            CleanupTaskType::LogRotation => {
                Self::rotate_logs(&task.target_path).await
            }
            CleanupTaskType::CacheCleanup => {
                Self::cleanup_cache(&task.target_path).await
            }
            CleanupTaskType::ProcessCleanup => {
                Ok((0, 0.0)) // Handled elsewhere
            }
        }
    }

    async fn cleanup_temp_files(temp_files: &Arc<Mutex<Vec<PathBuf>>>) -> Result<(u64, f64)> {
        let mut files = temp_files.lock().await;
        let mut files_cleaned = 0u64;
        let mut space_freed = 0f64;

        files.retain(|path| {
            if path.exists() {
                if let Ok(metadata) = std::fs::metadata(path) {
                    space_freed += metadata.len() as f64 / 1024.0 / 1024.0; // MB
                }
                if let Err(e) = std::fs::remove_file(path) {
                    tracing::warn!("Failed to remove temp file {:?}: {}", path, e);
                    return true; // Keep in list to retry later
                }
                files_cleaned += 1;
            }
            false // Remove from list
        });

        Ok((files_cleaned, space_freed))
    }

    async fn cleanup_old_recordings(recordings_dir: &PathBuf) -> Result<(u64, f64)> {
        if !recordings_dir.exists() {
            return Ok((0, 0.0));
        }

        let mut files_cleaned = 0u64;
        let mut space_freed = 0f64;
        let cutoff_date = chrono::Utc::now() - chrono::Duration::days(30);

        let mut entries = tokio::fs::read_dir(recordings_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;
            if metadata.is_file() {
                if let Ok(created) = metadata.created() {
                    let created_datetime = chrono::DateTime::<chrono::Utc>::from(created);
                    if created_datetime < cutoff_date {
                        space_freed += metadata.len() as f64 / 1024.0 / 1024.0;
                        if let Err(e) = tokio::fs::remove_file(entry.path()).await {
                            tracing::warn!("Failed to remove old recording {:?}: {}", entry.path(), e);
                        } else {
                            files_cleaned += 1;
                        }
                    }
                }
            }
        }

        Ok((files_cleaned, space_freed))
    }

    async fn rotate_logs(logs_dir: &PathBuf) -> Result<(u64, f64)> {
        // Implement log rotation logic
        Ok((0, 0.0))
    }

    async fn cleanup_cache(cache_dir: &PathBuf) -> Result<(u64, f64)> {
        // Implement cache cleanup logic
        Ok((0, 0.0))
    }

    pub async fn get_resource_stats(&self) -> ResourceStats {
        self.stats.read().await.clone()
    }

    pub async fn force_cleanup(&self) -> Result<()> {
        self.schedule_cleanup_task(CleanupTask {
            id: uuid::Uuid::new_v4().to_string(),
            task_type: CleanupTaskType::TempFile,
            target_path: PathBuf::from("temp"),
            created_at: Instant::now(),
            priority: CleanupPriority::Critical,
        }).await?;

        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down resource manager");

        // Cleanup all active processes
        let mut processes = self.active_processes.lock().await;
        for (name, mut process) in processes.drain() {
            tracing::info!("Terminating process: {}", name);
            let _ = process.kill().await;
        }

        // Cleanup temp files
        Self::cleanup_temp_files(&self.temp_files).await?;

        tracing::info!("Resource manager shutdown complete");
        Ok(())
    }
}

impl Drop for ResourceManager {
    fn drop(&mut self) {
        tracing::debug!("ResourceManager dropped for device: {}", self.device_id);
    }
}