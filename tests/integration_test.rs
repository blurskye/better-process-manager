use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

/// Helper to get the binary path
fn get_bpm_binary() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    path.pop(); // Remove 'deps'
    path.push("bpm");
    path
}

/// Helper to start daemon in background
fn start_daemon() -> std::process::Child {
    let bpm = get_bpm_binary();
    Command::new(bpm)
        .arg("daemon")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start daemon")
}

/// Helper to run bpm command
fn run_bpm(args: &[&str]) -> String {
    let bpm = get_bpm_binary();
    let output = Command::new(bpm)
        .args(args)
        .output()
        .expect("Failed to run bpm");

    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
#[ignore] // Run with: cargo test --test integration_test -- --ignored
fn test_full_lifecycle() {
    // Start daemon
    let mut daemon = start_daemon();
    thread::sleep(Duration::from_secs(2));

    // Create test config
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.json");
    let config = r#"{
        "name": "test-process",
        "script": "sleep",
        "args": ["10"],
        "cwd": "/tmp"
    }"#;
    fs::write(&config_path, config).unwrap();

    // Start process
    let output = run_bpm(&["start", config_path.to_str().unwrap()]);
    assert!(output.contains("Started"));

    thread::sleep(Duration::from_secs(1));

    // List processes
    let output = run_bpm(&["list"]);
    assert!(output.contains("test-process"));
    assert!(output.contains("running"));

    // Stop by ID
    let output = run_bpm(&["stop", "0"]);
    assert!(output.contains("Stopped"));

    thread::sleep(Duration::from_secs(1));

    // Verify stopped
    let output = run_bpm(&["list"]);
    assert!(output.contains("stopped"));

    // Cleanup
    daemon.kill().ok();
}

#[test]
#[ignore]
fn test_stop_by_name() {
    let mut daemon = start_daemon();
    thread::sleep(Duration::from_secs(2));

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.json");
    let config = r#"{
        "name": "named-process",
        "script": "sleep",
        "args": ["10"]
    }"#;
    fs::write(&config_path, config).unwrap();

    run_bpm(&["start", config_path.to_str().unwrap()]);
    thread::sleep(Duration::from_secs(1));

    // Stop by name
    let output = run_bpm(&["stop", "named-process"]);
    assert!(output.contains("Stopped"));

    daemon.kill().ok();
}

#[test]
#[ignore]
fn test_logs_by_id() {
    let mut daemon = start_daemon();
    thread::sleep(Duration::from_secs(2));

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.json");
    let config = r#"{
        "name": "echo-process",
        "script": "echo",
        "args": ["hello world"]
    }"#;
    fs::write(&config_path, config).unwrap();

    run_bpm(&["start", config_path.to_str().unwrap()]);
    thread::sleep(Duration::from_secs(2));

    // Get logs by ID
    let output = run_bpm(&["logs", "0"]);
    assert!(output.contains("echo-process"));

    daemon.kill().ok();
}
