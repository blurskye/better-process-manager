//! Process Registry - Tracks all managed processes
//!
//! This module provides the central registry for all processes managed by BPM.
//! It handles process lifecycle, state tracking, and metrics collection.

use crate::config::read_config::{App, HealthCheck, HealthCheckType as ConfigHealthCheckType};
use crate::process_manager::health::{HealthCheckConfig, HealthCheckType, HealthStatus};
use crate::process_manager::process::combined_usage;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use sysinfo::{Pid, System};

/// Process lifecycle states
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessState {
    Starting,
    Running,
    Stopping,
    Stopped,
    Errored,
    Restarting,
}

impl std::fmt::Display for ProcessState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessState::Starting => write!(f, "starting"),
            ProcessState::Running => write!(f, "running"),
            ProcessState::Stopping => write!(f, "stopping"),
            ProcessState::Stopped => write!(f, "stopped"),
            ProcessState::Errored => write!(f, "errored"),
            ProcessState::Restarting => write!(f, "restarting"),
        }
    }
}

/// Information about a managed process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    /// Unique name of the process
    pub name: String,
    /// Process ID (if running)
    pub pid: Option<u32>,
    /// Current state
    pub state: ProcessState,
    /// Path to the config file
    pub config_path: PathBuf,
    /// Script/command being run
    pub script: String,
    /// Arguments passed to the script
    pub args: Vec<String>,
    /// Working directory
    pub cwd: Option<PathBuf>,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Number of restarts since last start
    pub restart_count: u32,
    /// Time when the process was started
    pub started_at: Option<DateTime<Utc>>,
    /// Last known CPU usage (percentage)
    pub cpu_usage: f32,
    /// Last known memory usage (bytes)
    pub memory_usage: u64,
    /// Log file paths
    pub stdout_log: PathBuf,
    pub stderr_log: PathBuf,
    /// Whether auto-restart is enabled
    pub auto_restart: bool,
    /// Maximum memory before restart (0 = disabled)
    pub max_memory: u64,
    /// Health check configuration (optional)
    #[serde(skip)]
    pub healthcheck: Option<HealthCheckConfig>,
    /// Current health status
    #[serde(skip)]
    pub health_status: HealthStatus,
    /// Last health check time
    pub last_health_check: Option<DateTime<Utc>>,
    /// Consecutive health check failures
    pub health_failures: u32,
    /// Watch directories for auto-restart on file changes
    pub watch_dirs: Vec<PathBuf>,
    /// Watch patterns (e.g., "*.js", "*.py")
    pub watch_patterns: Vec<String>,
}

impl ProcessInfo {
    /// Create a new ProcessInfo from an App config
    pub fn from_app(app: &App, config_path: PathBuf) -> Self {
        let default_log_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("bpm")
            .join("logs")
            .join(&app.name);

        // Determine log paths - use custom if specified, otherwise default
        let stdout_log = Self::resolve_log_path(&app.log.out, &default_log_dir, "out.log");
        let stderr_log = Self::resolve_log_path(&app.log.error, &default_log_dir, "error.log");

        // Convert health check config if present
        let healthcheck = app
            .healthcheck
            .as_ref()
            .map(|hc| Self::convert_healthcheck(hc));

        // Get watch directories from cwd if specified
        let watch_dirs = app.cwd.clone().map(|d| vec![d]).unwrap_or_default();

        Self {
            name: app.name.clone(),
            pid: None,
            state: ProcessState::Stopped,
            config_path,
            script: app.script.clone(),
            args: app.args.clone(),
            cwd: app.cwd.clone(),
            env: app.env.clone(),
            restart_count: 0,
            started_at: None,
            cpu_usage: 0.0,
            memory_usage: 0,
            stdout_log,
            stderr_log,
            auto_restart: matches!(
                app.restart.policy,
                crate::config::read_config::RestartPolicy::Always
                    | crate::config::read_config::RestartPolicy::OnFailure
            ),
            max_memory: 0,
            healthcheck,
            health_status: HealthStatus::Unknown,
            last_health_check: None,
            health_failures: 0,
            watch_dirs,
            watch_patterns: vec![],
        }
    }

    /// Convert config HealthCheck to internal HealthCheckConfig
    fn convert_healthcheck(hc: &HealthCheck) -> HealthCheckConfig {
        let check_type = match hc.check_type {
            ConfigHealthCheckType::Http => HealthCheckType::Http {
                url: hc
                    .url
                    .clone()
                    .unwrap_or_else(|| "http://localhost:8080".to_string()),
                expected_status: None,
            },
            ConfigHealthCheckType::Tcp => HealthCheckType::Tcp {
                host: hc.host.clone().unwrap_or_else(|| "127.0.0.1".to_string()),
                port: hc.port.unwrap_or(8080),
            },
            ConfigHealthCheckType::Command => HealthCheckType::Command {
                cmd: hc.command.clone().unwrap_or_default(),
                args: vec![],
            },
        };

        HealthCheckConfig {
            check_type,
            interval: Self::parse_duration_str(&hc.interval),
            timeout: Self::parse_duration_str(&hc.timeout),
            retries: hc.retries,
            start_period: hc
                .start_period
                .as_ref()
                .map(|s| Self::parse_duration_str(s))
                .unwrap_or(Duration::from_secs(10)),
        }
    }

    /// Resolve log path - use custom path if absolute, otherwise use default directory
    fn resolve_log_path(config_path: &str, default_dir: &PathBuf, default_name: &str) -> PathBuf {
        // "stdout" and "stderr" are special values meaning use default
        if config_path == "stdout" || config_path == "stderr" {
            return default_dir.join(default_name);
        }

        let path = PathBuf::from(config_path);
        if path.is_absolute() {
            // Absolute path - use as-is
            path
        } else {
            // Relative path - use default directory
            default_dir.join(default_name)
        }
    }

    /// Parse duration string like "30s", "5m", "1h"
    fn parse_duration_str(s: &str) -> Duration {
        if s.ends_with('s') {
            s.trim_end_matches('s')
                .parse::<u64>()
                .map(Duration::from_secs)
                .unwrap_or(Duration::from_secs(30))
        } else if s.ends_with('m') {
            s.trim_end_matches('m')
                .parse::<u64>()
                .map(|m| Duration::from_secs(m * 60))
                .unwrap_or(Duration::from_secs(30))
        } else if s.ends_with('h') {
            s.trim_end_matches('h')
                .parse::<u64>()
                .map(|h| Duration::from_secs(h * 3600))
                .unwrap_or(Duration::from_secs(30))
        } else {
            Duration::from_secs(30)
        }
    }

    /// Get the uptime as a human-readable string
    pub fn uptime(&self) -> String {
        match self.started_at {
            Some(started) => {
                let duration = Utc::now().signed_duration_since(started);
                let secs = duration.num_seconds();
                if secs < 60 {
                    format!("{}s", secs)
                } else if secs < 3600 {
                    format!("{}m", secs / 60)
                } else if secs < 86400 {
                    format!("{}h", secs / 3600)
                } else {
                    format!("{}d", secs / 86400)
                }
            }
            None => "-".to_string(),
        }
    }

    /// Format memory usage as human-readable string
    pub fn memory_display(&self) -> String {
        let bytes = self.memory_usage;
        if bytes < 1024 {
            format!("{}B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1}KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1}GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

/// Thread-safe process registry
#[derive(Debug, Clone)]
pub struct ProcessRegistry {
    inner: Arc<RwLock<RegistryInner>>,
}

#[derive(Debug)]
struct RegistryInner {
    processes: HashMap<String, ProcessInfo>,
    system: System,
}

impl Default for ProcessRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RegistryInner {
                processes: HashMap::new(),
                system: System::new_all(),
            })),
        }
    }

    /// Register a new process
    pub fn register(&self, info: ProcessInfo) -> Result<(), String> {
        let mut inner = self.inner.write().map_err(|e| e.to_string())?;
        if inner.processes.contains_key(&info.name) {
            return Err(format!("Process '{}' already exists", info.name));
        }
        inner.processes.insert(info.name.clone(), info);
        Ok(())
    }

    /// Get a process by name
    pub fn get(&self, name: &str) -> Option<ProcessInfo> {
        let inner = self.inner.read().ok()?;
        inner.processes.get(name).cloned()
    }

    /// Update a process's state
    pub fn update_state(&self, name: &str, state: ProcessState) -> Result<(), String> {
        let mut inner = self.inner.write().map_err(|e| e.to_string())?;
        if let Some(process) = inner.processes.get_mut(name) {
            process.state = state;
            Ok(())
        } else {
            Err(format!("Process '{}' not found", name))
        }
    }

    /// Update a process's PID
    pub fn update_pid(&self, name: &str, pid: Option<u32>) -> Result<(), String> {
        let mut inner = self.inner.write().map_err(|e| e.to_string())?;
        if let Some(process) = inner.processes.get_mut(name) {
            process.pid = pid;
            if pid.is_some() {
                process.started_at = Some(Utc::now());
                process.state = ProcessState::Running;
            }
            Ok(())
        } else {
            Err(format!("Process '{}' not found", name))
        }
    }

    /// Increment restart count
    pub fn increment_restart_count(&self, name: &str) -> Result<u32, String> {
        let mut inner = self.inner.write().map_err(|e| e.to_string())?;
        if let Some(process) = inner.processes.get_mut(name) {
            process.restart_count += 1;
            Ok(process.restart_count)
        } else {
            Err(format!("Process '{}' not found", name))
        }
    }

    /// Remove a process from the registry
    pub fn remove(&self, name: &str) -> Option<ProcessInfo> {
        let mut inner = self.inner.write().ok()?;
        inner.processes.remove(name)
    }

    /// Get all processes
    pub fn list(&self) -> Vec<ProcessInfo> {
        let inner = self.inner.read().ok();
        match inner {
            Some(guard) => guard.processes.values().cloned().collect(),
            None => Vec::new(),
        }
    }

    /// Refresh metrics for all running processes
    pub fn refresh_metrics(&self) {
        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        inner.system.refresh_all();

        // Collect PIDs first
        let pids_to_check: Vec<(String, u32)> = inner
            .processes
            .iter()
            .filter_map(|(name, p)| p.pid.map(|pid| (name.clone(), pid)))
            .collect();

        // Collect metrics - use combined_usage to get process tree metrics
        let metrics: Vec<(String, Option<(f32, u64)>)> = pids_to_check
            .iter()
            .map(|(name, pid)| {
                // Try to get combined metrics for process tree, fall back to single process
                let metrics = combined_usage(*pid).ok().or_else(|| {
                    let sys_pid = Pid::from_u32(*pid);
                    inner
                        .system
                        .process(sys_pid)
                        .map(|p| (p.cpu_usage(), p.memory()))
                });
                (name.clone(), metrics)
            })
            .collect();

        // Now update processes
        for (name, opt_metrics) in metrics {
            if let Some(process) = inner.processes.get_mut(&name) {
                if let Some((cpu, mem)) = opt_metrics {
                    process.cpu_usage = cpu;
                    process.memory_usage = mem;
                } else {
                    // Process has died
                    if process.state == ProcessState::Running {
                        process.state = ProcessState::Errored;
                        process.pid = None;
                    }
                }
            }
        }
    }

    /// Check if any processes have died and need restart
    pub fn check_dead_processes(&self) -> Vec<String> {
        let mut dead = Vec::new();
        let inner = match self.inner.read() {
            Ok(guard) => guard,
            Err(_) => return dead,
        };

        for process in inner.processes.values() {
            if process.state == ProcessState::Errored && process.auto_restart {
                dead.push(process.name.clone());
            }
        }

        dead
    }

    /// Get all running processes
    pub fn get_running_processes(&self) -> Vec<ProcessInfo> {
        let inner = match self.inner.read() {
            Ok(guard) => guard,
            Err(_) => return vec![],
        };

        inner
            .processes
            .values()
            .filter(|p| p.state == ProcessState::Running)
            .cloned()
            .collect()
    }

    /// Update health status for a process
    pub fn update_health_status(&self, name: &str, status: HealthStatus) -> Result<(), String> {
        let mut inner = self.inner.write().map_err(|e| e.to_string())?;
        if let Some(process) = inner.processes.get_mut(name) {
            process.health_status = status;
            process.last_health_check = Some(Utc::now());
            Ok(())
        } else {
            Err(format!("Process '{}' not found", name))
        }
    }

    /// Increment health failure count and return new count
    pub fn increment_health_failures(&self, name: &str) -> u32 {
        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(_) => return 0,
        };

        if let Some(process) = inner.processes.get_mut(name) {
            process.health_failures += 1;
            process.health_failures
        } else {
            0
        }
    }

    /// Reset health failure count
    pub fn reset_health_failures(&self, name: &str) -> Result<(), String> {
        let mut inner = self.inner.write().map_err(|e| e.to_string())?;
        if let Some(process) = inner.processes.get_mut(name) {
            process.health_failures = 0;
            Ok(())
        } else {
            Err(format!("Process '{}' not found", name))
        }
    }

    /// Format process list as a table string
    pub fn format_table(&self) -> String {
        let processes = self.list();

        if processes.is_empty() {
            return "No processes running".to_string();
        }

        let mut output = String::new();
        output.push_str(&format!(
            "{:<4} {:<20} {:<10} {:<8} {:<8} {:<10} {:<8}\n",
            "ID", "NAME", "STATUS", "↺", "CPU", "MEM", "UPTIME"
        ));
        output.push_str(&"-".repeat(76));
        output.push('\n');

        for (idx, process) in processes.iter().enumerate() {
            let status_color = match process.state {
                ProcessState::Running => "🟢",
                ProcessState::Stopped => "⚪",
                ProcessState::Errored => "🔴",
                ProcessState::Starting | ProcessState::Restarting => "🟡",
                ProcessState::Stopping => "🟠",
            };

            output.push_str(&format!(
                "{:<4} {:<20} {} {:<7} {:<8} {:<8} {:<10} {:<8}\n",
                idx,
                truncate(&process.name, 20),
                status_color,
                process.state,
                process.restart_count,
                format!("{:.1}%", process.cpu_usage),
                process.memory_display(),
                process.uptime(),
            ));
        }

        output
    }

    /// Save registry state to disk
    pub fn save_state(&self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let inner = self.inner.read().map_err(|e| e.to_string())?;
        let processes: Vec<&ProcessInfo> = inner.processes.values().collect();
        let json = serde_json::to_string_pretty(&processes)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load registry state from disk
    pub fn load_state(&self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        if !path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(path)?;
        let processes: Vec<ProcessInfo> = serde_json::from_str(&content)?;

        let mut inner = self.inner.write().map_err(|e| e.to_string())?;
        for process in processes {
            inner.processes.insert(process.name.clone(), process);
        }

        Ok(())
    }
}

/// Truncate a string to a maximum length
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}…", &s[..max_len - 1])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_process(name: &str) -> ProcessInfo {
        ProcessInfo {
            name: name.to_string(),
            pid: None,
            state: ProcessState::Stopped,
            config_path: PathBuf::from("/tmp/test.json"),
            script: "echo".to_string(),
            args: vec!["hello".to_string()],
            cwd: None,
            env: HashMap::new(),
            restart_count: 0,
            started_at: None,
            cpu_usage: 0.0,
            memory_usage: 0,
            stdout_log: PathBuf::from("/tmp/out.log"),
            stderr_log: PathBuf::from("/tmp/err.log"),
            auto_restart: true,
            max_memory: 0,
            healthcheck: None,
            health_status: HealthStatus::Unknown,
            last_health_check: None,
            health_failures: 0,
            watch_dirs: vec![],
            watch_patterns: vec![],
        }
    }

    #[test]
    fn test_process_registry() {
        let registry = ProcessRegistry::new();
        let info = create_test_process("test");

        assert!(registry.register(info.clone()).is_ok());
        assert!(registry.get("test").is_some());
        assert!(registry.update_state("test", ProcessState::Running).is_ok());

        let updated = registry.get("test").unwrap();
        assert_eq!(updated.state, ProcessState::Running);
    }

    #[test]
    fn test_registry_duplicate_register() {
        let registry = ProcessRegistry::new();
        let info = create_test_process("dup-test");

        assert!(registry.register(info.clone()).is_ok());
        assert!(registry.register(info).is_err()); // Should fail on duplicate
    }

    #[test]
    fn test_registry_update_pid() {
        let registry = ProcessRegistry::new();
        let info = create_test_process("pid-test");

        registry.register(info).unwrap();
        assert!(registry.update_pid("pid-test", Some(12345)).is_ok());

        let process = registry.get("pid-test").unwrap();
        assert_eq!(process.pid, Some(12345));
        assert_eq!(process.state, ProcessState::Running);
        assert!(process.started_at.is_some());
    }

    #[test]
    fn test_registry_restart_count() {
        let registry = ProcessRegistry::new();
        let info = create_test_process("restart-test");

        registry.register(info).unwrap();

        let count = registry.increment_restart_count("restart-test").unwrap();
        assert_eq!(count, 1);

        let count = registry.increment_restart_count("restart-test").unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_registry_health_tracking() {
        let registry = ProcessRegistry::new();
        let info = create_test_process("health-test");

        registry.register(info).unwrap();

        // Test health failure tracking
        assert_eq!(registry.increment_health_failures("health-test"), 1);
        assert_eq!(registry.increment_health_failures("health-test"), 2);
        assert_eq!(registry.increment_health_failures("health-test"), 3);

        // Test reset
        assert!(registry.reset_health_failures("health-test").is_ok());
        let process = registry.get("health-test").unwrap();
        assert_eq!(process.health_failures, 0);
    }

    #[test]
    fn test_registry_get_running() {
        let registry = ProcessRegistry::new();

        let mut running = create_test_process("running");
        running.state = ProcessState::Running;

        let stopped = create_test_process("stopped");

        registry.register(running).unwrap();
        registry.register(stopped).unwrap();

        let running_procs = registry.get_running_processes();
        assert_eq!(running_procs.len(), 1);
        assert_eq!(running_procs[0].name, "running");
    }

    #[test]
    fn test_resolve_log_path_default() {
        let default_dir = PathBuf::from("/var/log/bpm/test");

        // "stdout" and "stderr" use defaults
        let path = ProcessInfo::resolve_log_path("stdout", &default_dir, "out.log");
        assert_eq!(path, PathBuf::from("/var/log/bpm/test/out.log"));

        let path = ProcessInfo::resolve_log_path("stderr", &default_dir, "error.log");
        assert_eq!(path, PathBuf::from("/var/log/bpm/test/error.log"));
    }

    #[test]
    fn test_resolve_log_path_absolute() {
        let default_dir = PathBuf::from("/var/log/bpm/test");

        // Absolute path used as-is
        let path = ProcessInfo::resolve_log_path("/custom/path/app.log", &default_dir, "out.log");
        assert_eq!(path, PathBuf::from("/custom/path/app.log"));
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(
            ProcessInfo::parse_duration_str("30s"),
            Duration::from_secs(30)
        );
        assert_eq!(
            ProcessInfo::parse_duration_str("5m"),
            Duration::from_secs(300)
        );
        assert_eq!(
            ProcessInfo::parse_duration_str("1h"),
            Duration::from_secs(3600)
        );
        assert_eq!(
            ProcessInfo::parse_duration_str("invalid"),
            Duration::from_secs(30)
        ); // Default
    }
}
