use crate::communication::common::ChunkPayload;
use crate::config::read_config::AppConfig;
use crate::process_manager::health::{check_health, HealthStatus};
use crate::process_manager::registry::{ProcessInfo, ProcessRegistry, ProcessState};
use crate::process_manager::watch::FileWatcher;
use chrono::Utc;
use iceoryx2::active_request::ActiveRequest;
use iceoryx2::prelude::*;
use iceoryx2::service::builder::request_response::RequestResponseOpenError;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::communication::common;

/// Global process registry for the daemon
static REGISTRY: std::sync::OnceLock<ProcessRegistry> = std::sync::OnceLock::new();

fn get_registry() -> &'static ProcessRegistry {
    REGISTRY.get_or_init(ProcessRegistry::new)
}

fn get_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("bpm")
}

fn get_state_file() -> PathBuf {
    get_data_dir().join("state.json")
}

pub fn server_running<Service>(
    node: &Node<Service>,
    service_name: &str,
) -> Result<bool, Box<dyn std::error::Error>>
where
    Service: iceoryx2::service::Service,
{
    let service_check = node
        .service_builder(&service_name.try_into()?)
        .request_response::<common::Command, common::MessageChunk>()
        .open();

    match service_check {
        Ok(_) => Ok(true),
        Err(RequestResponseOpenError::DoesNotExist) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::default();
    let node = NodeBuilder::new()
        .config(&config)
        .create::<ipc::Service>()?;

    if server_running(&node, common::IPC_NAME)? {
        eprintln!("Another instance of the daemon is already running.");
        std::process::exit(1);
    }

    // Load previous state
    let registry = get_registry();
    if let Err(e) = registry.load_state(&get_state_file()) {
        eprintln!("Warning: Could not load previous state: {}", e);
    }

    let service_name = common::IPC_NAME.try_into()?;
    let service = node
        .service_builder(&service_name)
        .request_response::<common::Command, common::MessageChunk>()
        .open_or_create()?;

    let server = service.server_builder().create()?;

    println!("BPM daemon started");
    println!("Data directory: {}", get_data_dir().display());

    // Spawn background monitoring thread
    let registry_clone = registry.clone();
    std::thread::spawn(move || {
        // Store file watchers for processes with watch enabled
        let mut file_watchers: HashMap<String, FileWatcher> = HashMap::new();

        loop {
            std::thread::sleep(Duration::from_secs(5));
            registry_clone.refresh_metrics();

            // Check for dead processes that need restart
            let dead = registry_clone.check_dead_processes();
            for name in dead {
                if let Some(process) = registry_clone.get(&name) {
                    println!("Process '{}' died, attempting restart...", name);
                    let _ = registry_clone.update_state(&name, ProcessState::Restarting);
                    let _ = registry_clone.increment_restart_count(&name);

                    // Actually restart the process
                    match start_process(&registry_clone, &process) {
                        Ok(_) => println!("Process '{}' restarted successfully", name),
                        Err(e) => eprintln!("Failed to restart '{}': {}", name, e),
                    }
                }
            }

            // Run health checks on running processes
            let running = registry_clone.get_running_processes();
            for process in running {
                if let Some(hc_config) = &process.healthcheck {
                    // Check if enough time has passed since last check
                    let should_check = match process.last_health_check {
                        Some(last) => {
                            let elapsed = Utc::now().signed_duration_since(last);
                            elapsed.num_seconds() >= hc_config.interval.as_secs() as i64
                        }
                        None => {
                            // Check if start period has passed
                            if let Some(started) = process.started_at {
                                let elapsed = Utc::now().signed_duration_since(started);
                                elapsed.num_seconds() >= hc_config.start_period.as_secs() as i64
                            } else {
                                false
                            }
                        }
                    };

                    if should_check {
                        let status = check_health(hc_config);
                        let _ = registry_clone.update_health_status(&process.name, status.clone());

                        match status {
                            HealthStatus::Healthy => {
                                // Reset failure count
                                let _ = registry_clone.reset_health_failures(&process.name);
                            }
                            HealthStatus::Unhealthy(reason) => {
                                let failures =
                                    registry_clone.increment_health_failures(&process.name);
                                println!(
                                    "Health check failed for '{}': {} (failure {})",
                                    process.name, reason, failures
                                );

                                // Restart if too many failures
                                if failures >= hc_config.retries {
                                    println!("Process '{}' unhealthy, restarting...", process.name);
                                    let _ = registry_clone
                                        .update_state(&process.name, ProcessState::Restarting);
                                    let _ = registry_clone.reset_health_failures(&process.name);
                                    if let Some(proc) = registry_clone.get(&process.name) {
                                        match start_process(&registry_clone, &proc) {
                                            Ok(_) => println!(
                                                "Process '{}' restarted due to health check",
                                                process.name
                                            ),
                                            Err(e) => eprintln!(
                                                "Failed to restart '{}': {}",
                                                process.name, e
                                            ),
                                        }
                                    }
                                }
                            }
                            HealthStatus::Unknown => {}
                        }
                    }
                }

                // Initialize file watcher if needed
                if !process.watch_dirs.is_empty() && !file_watchers.contains_key(&process.name) {
                    let watcher = FileWatcher::new(
                        process.watch_dirs.clone(),
                        process.watch_patterns.clone(),
                    );
                    if watcher.init().is_ok() {
                        file_watchers.insert(process.name.clone(), watcher);
                    }
                }
            }

            // Check file watchers for changes
            let mut to_restart = Vec::new();
            for (name, watcher) in &file_watchers {
                if let Ok(changes) = watcher.check_changes() {
                    if !changes.is_empty() {
                        println!("File changes detected for '{}': {:?}", name, changes);
                        to_restart.push(name.clone());
                    }
                }
            }

            // Restart processes with file changes
            for name in to_restart {
                if let Some(process) = registry_clone.get(&name) {
                    println!("Restarting '{}' due to file changes...", name);
                    let _ = registry_clone.update_state(&name, ProcessState::Restarting);
                    match start_process(&registry_clone, &process) {
                        Ok(_) => println!("Process '{}' restarted due to file changes", name),
                        Err(e) => eprintln!("Failed to restart '{}': {}", name, e),
                    }
                }
            }
        }
    });

    while node.wait(Duration::from_millis(100)).is_ok() {
        while let Some(request) = server.receive()? {
            let response = match &*request {
                common::Command::List => {
                    registry.refresh_metrics();
                    registry.format_table()
                }
                common::Command::Status(payload) => {
                    let name = common::Command::decode_payload(payload).unwrap_or("");
                    handle_status(registry, name)
                }
                common::Command::Start(payload) => {
                    let path = common::Command::decode_payload(payload).unwrap_or("");
                    handle_start(registry, path)
                }
                common::Command::Stop(payload) => {
                    let name = common::Command::decode_payload(payload).unwrap_or("");
                    handle_stop(registry, name)
                }
                common::Command::Restart(payload) => {
                    let name = common::Command::decode_payload(payload).unwrap_or("");
                    handle_restart(registry, name)
                }
                common::Command::Delete(payload) => {
                    let name = common::Command::decode_payload(payload).unwrap_or("");
                    handle_delete(registry, name)
                }
                common::Command::Enable(payload) => {
                    let path = common::Command::decode_payload(payload).unwrap_or("");
                    handle_enable(registry, path)
                }
                common::Command::Disable(payload) => {
                    let name = common::Command::decode_payload(payload).unwrap_or("");
                    handle_disable(registry, name)
                }
                common::Command::Logs(payload) => {
                    let args = common::Command::decode_payload(payload).unwrap_or("");
                    handle_logs(registry, args)
                }
                common::Command::Flush(payload) => {
                    let name = common::Command::decode_payload(payload).unwrap_or("");
                    handle_flush(registry, name)
                }
                common::Command::Save => handle_save(registry),
                common::Command::Resurrect => handle_resurrect(registry),
            };

            send_response(&request, response, common::CHUNK_PAYLOAD_CAPACITY)?;
        }
    }

    // Save state before exiting
    if let Err(e) = registry.save_state(&get_state_file()) {
        eprintln!("Warning: Could not save state: {}", e);
    }

    Ok(())
}

fn handle_status(registry: &ProcessRegistry, name: &str) -> String {
    match registry.get(name) {
        Some(process) => {
            serde_json::to_string_pretty(&process).unwrap_or_else(|_| format!("{:?}", process))
        }
        None => format!("Process '{}' not found", name),
    }
}

fn handle_start(registry: &ProcessRegistry, path: &str) -> String {
    let config_path = PathBuf::from(path);

    if !config_path.exists() {
        return format!("Config file not found: {}", path);
    }

    let config = match AppConfig::from_file(&config_path) {
        Ok(c) => c,
        Err(e) => return format!("Failed to parse config: {}", e),
    };

    let (_, apps) = config.get_apps();
    let mut results = Vec::new();

    for app in apps {
        let info = ProcessInfo::from_app(&app, config_path.clone());
        let name = info.name.clone();

        if let Err(e) = registry.register(info.clone()) {
            results.push(format!("Warning: {}", e));
            continue;
        }

        match start_process(registry, &info) {
            Ok(_) => results.push(format!("Started: {}", name)),
            Err(e) => results.push(format!("Failed to start {}: {}", name, e)),
        }
    }

    results.join("\n")
}

fn start_process(
    registry: &ProcessRegistry,
    info: &ProcessInfo,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::{Command, Stdio};

    let _ = registry.update_state(&info.name, ProcessState::Starting);

    // Create log directories
    if let Some(parent) = info.stdout_log.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let stdout_file = std::fs::File::create(&info.stdout_log)?;
    let stderr_file = std::fs::File::create(&info.stderr_log)?;

    let mut cmd = Command::new(&info.script);
    cmd.args(&info.args)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));

    if let Some(cwd) = &info.cwd {
        cmd.current_dir(cwd);
    }

    for (key, value) in &info.env {
        cmd.env(key, value);
    }

    let child = cmd.spawn()?;
    let pid = child.id();

    registry.update_pid(&info.name, Some(pid))?;

    Ok(())
}

fn handle_stop(registry: &ProcessRegistry, name: &str) -> String {
    match registry.get(name) {
        Some(process) => {
            if let Some(pid) = process.pid {
                let _ = registry.update_state(name, ProcessState::Stopping);

                // Send SIGTERM
                if let Err(e) = nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(pid as i32),
                    nix::sys::signal::Signal::SIGTERM,
                ) {
                    return format!("Failed to send SIGTERM: {}", e);
                }

                // Wait a bit, then check if process is still running
                std::thread::sleep(Duration::from_secs(2));

                // Check if still running, send SIGKILL if needed
                if let Some(updated) = registry.get(name) {
                    if updated.pid.is_some() {
                        let _ = nix::sys::signal::kill(
                            nix::unistd::Pid::from_raw(pid as i32),
                            nix::sys::signal::Signal::SIGKILL,
                        );
                    }
                }

                let _ = registry.update_state(name, ProcessState::Stopped);
                let _ = registry.update_pid(name, None);

                format!("Stopped: {}", name)
            } else {
                format!("Process '{}' is not running", name)
            }
        }
        None => format!("Process '{}' not found", name),
    }
}

fn handle_restart(registry: &ProcessRegistry, name: &str) -> String {
    let stop_result = handle_stop(registry, name);

    if let Some(process) = registry.get(name) {
        std::thread::sleep(Duration::from_millis(500));
        match start_process(registry, &process) {
            Ok(_) => format!("{}\nRestarted: {}", stop_result, name),
            Err(e) => format!("{}\nFailed to restart: {}", stop_result, e),
        }
    } else {
        stop_result
    }
}

fn handle_delete(registry: &ProcessRegistry, name: &str) -> String {
    let stop_result = handle_stop(registry, name);

    match registry.remove(name) {
        Some(_) => format!("{}\nDeleted: {}", stop_result, name),
        None => format!("Process '{}' not found", name),
    }
}

fn handle_enable(registry: &ProcessRegistry, path: &str) -> String {
    // Enable is same as start for now
    handle_start(registry, path)
}

fn handle_disable(registry: &ProcessRegistry, name: &str) -> String {
    if let Some(mut process) = registry.get(name) {
        process.auto_restart = false;
        format!("Auto-restart disabled for: {}", name)
    } else {
        format!("Process '{}' not found", name)
    }
}

fn handle_logs(registry: &ProcessRegistry, args: &str) -> String {
    let parts: Vec<&str> = args.split(':').collect();
    let name = parts.first().unwrap_or(&"");
    let lines: usize = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(20);
    // follow is ignored in this simple implementation

    if let Some(process) = registry.get(name) {
        let mut output = String::new();

        // Read stdout log
        if let Ok(content) = std::fs::read_to_string(&process.stdout_log) {
            let log_lines: Vec<&str> = content.lines().collect();
            let start = log_lines.len().saturating_sub(lines);
            output.push_str(&format!("=== {} stdout ===\n", name));
            for line in &log_lines[start..] {
                output.push_str(line);
                output.push('\n');
            }
        }

        // Read stderr log
        if let Ok(content) = std::fs::read_to_string(&process.stderr_log) {
            let log_lines: Vec<&str> = content.lines().collect();
            let start = log_lines.len().saturating_sub(lines);
            output.push_str(&format!("\n=== {} stderr ===\n", name));
            for line in &log_lines[start..] {
                output.push_str(line);
                output.push('\n');
            }
        }

        if output.is_empty() {
            format!("No logs found for: {}", name)
        } else {
            output
        }
    } else {
        format!("Process '{}' not found", name)
    }
}

fn handle_flush(registry: &ProcessRegistry, name: &str) -> String {
    if name.is_empty() {
        // Flush all logs
        let processes = registry.list();
        for process in processes {
            let _ = std::fs::write(&process.stdout_log, "");
            let _ = std::fs::write(&process.stderr_log, "");
        }
        "Flushed all logs".to_string()
    } else if let Some(process) = registry.get(name) {
        let _ = std::fs::write(&process.stdout_log, "");
        let _ = std::fs::write(&process.stderr_log, "");
        format!("Flushed logs for: {}", name)
    } else {
        format!("Process '{}' not found", name)
    }
}

fn handle_save(registry: &ProcessRegistry) -> String {
    match registry.save_state(&get_state_file()) {
        Ok(_) => format!("State saved to: {}", get_state_file().display()),
        Err(e) => format!("Failed to save state: {}", e),
    }
}

fn handle_resurrect(registry: &ProcessRegistry) -> String {
    // First load the state
    if let Err(e) = registry.load_state(&get_state_file()) {
        return format!("Failed to load state: {}", e);
    }

    // Then start all processes that were running
    let processes = registry.list();
    let mut results = Vec::new();

    for process in processes {
        if process.state == ProcessState::Running || process.state == ProcessState::Stopped {
            match start_process(registry, &process) {
                Ok(_) => results.push(format!("Resurrected: {}", process.name)),
                Err(e) => results.push(format!("Failed to resurrect {}: {}", process.name, e)),
            }
        }
    }

    if results.is_empty() {
        "No processes to resurrect".to_string()
    } else {
        results.join("\n")
    }
}

pub fn send_response<Service, RequestPayload, RequestHeader, ResponsePayload, ResponseHeader>(
    request: &ActiveRequest<
        Service,
        RequestPayload,
        RequestHeader,
        ResponsePayload,
        ResponseHeader,
    >,
    response_data: impl AsRef<[u8]>,
    chunk_capacity: usize,
) -> Result<(), Box<dyn std::error::Error>>
where
    Service: iceoryx2::service::Service,
    RequestPayload: std::fmt::Debug + iceoryx2::prelude::ZeroCopySend + ?Sized,
    RequestHeader: std::fmt::Debug + iceoryx2::prelude::ZeroCopySend,
    ResponsePayload: ChunkPayload + std::fmt::Debug + iceoryx2::prelude::ZeroCopySend,
    ResponseHeader: std::fmt::Debug + iceoryx2::prelude::ZeroCopySend + Default,
{
    let response_bytes = response_data.as_ref();

    let mut chunks = response_bytes.chunks(chunk_capacity).peekable();
    let mut seq_num = 0;

    while let Some(chunk_data) = chunks.next() {
        let is_last_chunk = chunks.peek().is_none();
        let chunk = ResponsePayload::new(
            seq_num,
            is_last_chunk,
            chunk_data.len() as u32,
            chunk_data.to_vec(),
        );

        request.send_copy(chunk)?;
        seq_num += 1;
    }

    Ok(())
}
