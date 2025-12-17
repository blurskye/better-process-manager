//! Process spawner utilities
//!
//! Async-based process spawning with log handling.

#![allow(dead_code)] // Spawner is for future use with async integration

use std::process::Stdio;
use std::sync::Arc;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

use std::time::{Duration, Instant};
pub struct LaunchCommand {
    program: String,
    directory: String,
    args: Vec<String>,

    log_path: String,
}

#[tokio::main]
pub async fn spawn(
    program: String,
    args: Vec<String>,
    log_path: String,
    restart: bool,
    always_restart: bool,
    directory: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let app_start_timestamp = Instant::now();
        // let directory = directory.unwrap_or("~/");

        let directory = directory.clone().unwrap_or_else(|| "~/".to_string());
        let mut child = Command::new(&program)
            .current_dir(directory)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let err_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .append(true)
            .open(format!("{}/err.txt", log_path))
            .await?;
        let out_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .append(true)
            .open(format!("{}/out.txt", log_path))
            .await?;

        let shared_err_file = Arc::new(Mutex::new(err_file));
        let shared_out_file = Arc::new(Mutex::new(out_file));

        let stdout = child.stdout.take().expect("child should have stdout");
        let stderr = child.stderr.take().expect("child should have stderr");

        let stdout_handle = tokio::spawn(write_logs(stdout, shared_out_file.clone()));
        let stderr_handle = tokio::spawn(write_logs(stderr, shared_err_file.clone()));

        let status = child.wait().await?;
        println!("\n[EXIT] Exited with: {}", status);

        stdout_handle.await??;
        stderr_handle.await??;
        if app_start_timestamp.elapsed() < Duration::from_secs(3) {
            break;
        }

        if !always_restart && !restart {
            break;
        } else if status.success() {
            break;
        } else if restart == false {
            break;
        }
    }

    Ok(())
}

async fn write_logs(
    stream: impl AsyncRead + Unpin,
    file: Arc<Mutex<tokio::fs::File>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let reader = BufReader::new(stream);
    let mut lines = reader.lines();

    while let Some(line) = lines.next_line().await? {
        // println!("{}", line);
        let mut file = file.lock().await;
        file.write_all(format!("{}\n", line).as_bytes()).await?;
        file.flush().await?;
    }

    Ok(())
}
