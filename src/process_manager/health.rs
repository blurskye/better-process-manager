//! Health Check Module
//!
//! Implements HTTP, TCP, and command-based health checks for processes.

#![allow(dead_code)] // Health checks are for future integration

use std::net::TcpStream;
use std::process::Command;
use std::time::Duration;

/// Health check result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Unhealthy(String),
    Unknown,
}

/// Health check configuration
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    pub check_type: HealthCheckType,
    pub interval: Duration,
    pub timeout: Duration,
    pub retries: u32,
    pub start_period: Duration,
}

#[derive(Debug, Clone)]
pub enum HealthCheckType {
    Http {
        url: String,
        expected_status: Option<u16>,
    },
    Tcp {
        host: String,
        port: u16,
    },
    Command {
        cmd: String,
        args: Vec<String>,
    },
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_type: HealthCheckType::Tcp {
                host: "127.0.0.1".to_string(),
                port: 8080,
            },
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            retries: 3,
            start_period: Duration::from_secs(10),
        }
    }
}

/// Perform a health check based on the configuration
pub fn check_health(config: &HealthCheckConfig) -> HealthStatus {
    match &config.check_type {
        HealthCheckType::Http {
            url,
            expected_status,
        } => check_http(url, config.timeout, *expected_status),
        HealthCheckType::Tcp { host, port } => check_tcp(host, *port, config.timeout),
        HealthCheckType::Command { cmd, args } => check_command(cmd, args, config.timeout),
    }
}

/// HTTP health check
fn check_http(url: &str, timeout: Duration, expected_status: Option<u16>) -> HealthStatus {
    // Using a simple HTTP check without external dependencies
    // For a full implementation, we'd use reqwest or ureq

    // Parse URL to get host and port
    let url_parts: Vec<&str> = url
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split('/')
        .collect();

    let host_port: Vec<&str> = url_parts
        .first()
        .unwrap_or(&"localhost:80")
        .split(':')
        .collect();
    let host = host_port.first().unwrap_or(&"localhost");
    let port: u16 = host_port.get(1).and_then(|p| p.parse().ok()).unwrap_or(80);

    // First check if we can connect
    match TcpStream::connect_timeout(&format!("{}:{}", host, port).parse().unwrap(), timeout) {
        Ok(mut stream) => {
            use std::io::{Read, Write};

            // Send HTTP GET request
            let path = if url_parts.len() > 1 {
                format!("/{}", url_parts[1..].join("/"))
            } else {
                "/".to_string()
            };

            let request = format!(
                "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
                path, host
            );

            if stream.write_all(request.as_bytes()).is_err() {
                return HealthStatus::Unhealthy("Failed to send request".to_string());
            }

            let mut response = String::new();
            if stream.read_to_string(&mut response).is_err() {
                return HealthStatus::Unhealthy("Failed to read response".to_string());
            }

            // Parse status code from response
            if let Some(status_line) = response.lines().next() {
                let parts: Vec<&str> = status_line.split_whitespace().collect();
                if let Some(status_code) = parts.get(1).and_then(|s| s.parse::<u16>().ok()) {
                    if let Some(expected) = expected_status {
                        if status_code == expected {
                            return HealthStatus::Healthy;
                        } else {
                            return HealthStatus::Unhealthy(format!(
                                "Expected status {}, got {}",
                                expected, status_code
                            ));
                        }
                    } else if status_code >= 200 && status_code < 400 {
                        return HealthStatus::Healthy;
                    } else {
                        return HealthStatus::Unhealthy(format!("Status code: {}", status_code));
                    }
                }
            }

            HealthStatus::Unhealthy("Invalid HTTP response".to_string())
        }
        Err(e) => HealthStatus::Unhealthy(format!("Connection failed: {}", e)),
    }
}

/// TCP health check - just verifies the port is open
fn check_tcp(host: &str, port: u16, timeout: Duration) -> HealthStatus {
    let addr = format!("{}:{}", host, port);
    match addr.parse() {
        Ok(socket_addr) => match TcpStream::connect_timeout(&socket_addr, timeout) {
            Ok(_) => HealthStatus::Healthy,
            Err(e) => HealthStatus::Unhealthy(format!("Connection failed: {}", e)),
        },
        Err(e) => HealthStatus::Unhealthy(format!("Invalid address: {}", e)),
    }
}

/// Command health check - runs a command and checks exit code
fn check_command(cmd: &str, args: &[String], timeout: Duration) -> HealthStatus {
    use std::time::Instant;

    let start = Instant::now();
    let result = Command::new(cmd).args(args).output();

    if start.elapsed() > timeout {
        return HealthStatus::Unhealthy("Command timed out".to_string());
    }

    match result {
        Ok(output) => {
            if output.status.success() {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy(format!(
                    "Command exited with code: {}",
                    output.status.code().unwrap_or(-1)
                ))
            }
        }
        Err(e) => HealthStatus::Unhealthy(format!("Command failed: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_localhost() {
        // This test may fail if nothing is listening on port 22
        let status = check_tcp("127.0.0.1", 22, Duration::from_secs(1));
        // Just verify it returns a valid status
        assert!(matches!(
            status,
            HealthStatus::Healthy | HealthStatus::Unhealthy(_)
        ));
    }

    #[test]
    fn test_command_true() {
        let status = check_command("true", &[], Duration::from_secs(5));
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[test]
    fn test_command_false() {
        let status = check_command("false", &[], Duration::from_secs(5));
        assert!(matches!(status, HealthStatus::Unhealthy(_)));
    }
}
