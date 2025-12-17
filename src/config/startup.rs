//! Startup script generation for systemd
//!
//! Generates systemd user service files for automatic startup.

#![allow(dead_code)] // remove_startup_script for future unstartup command

use std::fs;
use std::path::PathBuf;
pub fn generate_startup_script() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let systemd_user_dir = home.join(".config/systemd/user");

    // Create the systemd user directory if it doesn't exist
    fs::create_dir_all(&systemd_user_dir)?;

    let service_path = systemd_user_dir.join("bpm.service");

    // Get the path to our own executable
    let exe_path = std::env::current_exe()?;

    let service_content = format!(
        r#"[Unit]
Description=Better Process Manager Daemon
After=default.target

[Service]
Type=simple
ExecStart={exe} daemon
Restart=on-failure
RestartSec=5
Environment=RUST_BACKTRACE=1

[Install]
WantedBy=default.target
"#,
        exe = exe_path.display()
    );

    fs::write(&service_path, service_content)?;

    println!("Service file created at: {}", service_path.display());
    println!();
    println!("To enable and start the service, run:");
    println!("  systemctl --user daemon-reload");
    println!("  systemctl --user enable bpm");
    println!("  systemctl --user start bpm");
    println!();
    println!("To check status:");
    println!("  systemctl --user status bpm");

    Ok(service_path)
}

/// Remove the startup script
pub fn remove_startup_script() -> Result<(), Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let service_path = home.join(".config/systemd/user/bpm.service");

    if service_path.exists() {
        fs::remove_file(&service_path)?;
        println!("Service file removed: {}", service_path.display());
        println!();
        println!("To fully disable, run:");
        println!("  systemctl --user daemon-reload");
    } else {
        println!("No startup script found at: {}", service_path.display());
    }

    Ok(())
}
