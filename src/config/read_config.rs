use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use serde::de::Deserializer;
use std::time::Duration;
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)] //auto cohersion into what matches signature
pub enum AppConfig {
    // Single app will look like { "name": "web-server", "script": "node server.js", ... }
    SingleApp(Box<App>),

    // Multi-app will look like { "my-project": [{ "name": "web-server", ... }, { "name": "worker", ... }] }
    MultiApp(Box<HashMap<String, Vec<App>>>),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct App {
    pub name: String,
    pub script: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub log: LogConfig,
    #[serde(default)]
    pub restart: RestartConfig,
    #[serde(default)]
    pub healthcheck: Option<HealthCheck>,
    #[serde(default)]
    pub schedule: Option<String>, // this will use cron syntax
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LogConfig {
    #[serde(default = "default_log_out")]
    pub out: String,
    #[serde(default = "default_log_error")]
    pub error: String,
    #[serde(default)]
    pub combined: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RestartConfig {
    #[serde(default = "default_restart_policy")]
    pub policy: RestartPolicy,
    #[serde(default = "default_max_restarts")]
    pub max_restarts: i32, // we use -1 for unlimited
    #[serde(deserialize_with = "parse_duration")]
    pub restart_delay: Duration,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    Always,
    OnFailure,
    Never,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HealthCheck {
    #[serde(rename = "type")]
    pub check_type: HealthCheckType,
    #[serde(default = "default_health_interval")]
    pub interval: String,
    #[serde(default = "default_health_timeout")]
    pub timeout: String,
    #[serde(default = "default_health_retries")]
    pub retries: u32,
    #[serde(default)]
    pub start_period: Option<String>,

    // HTTP specific
    #[serde(default)]
    pub url: Option<String>,

    // Command specific
    #[serde(default)]
    pub command: Option<String>,

    // TCP specific
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum HealthCheckType {
    Http,
    Tcp,
    Command,
}

fn default_log_out() -> String {
    "stdout".to_string()
}
fn default_log_error() -> String {
    "stderr".to_string()
}
fn default_restart_policy() -> RestartPolicy {
    RestartPolicy::OnFailure
}
fn default_max_restarts() -> i32 {
    -1
}
fn default_restart_delay() -> Duration {
    Duration::from_secs(5)
}
fn default_health_interval() -> String {
    "30s".to_string()
}
fn default_health_timeout() -> String {
    "5s".to_string()
}
fn default_health_retries() -> u32 {
    3
}
fn parse_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let duration = if s.ends_with("s") {
        let secs = s
            .trim_end_matches("s")
            .parse::<u64>()
            .map_err(serde::de::Error::custom)?;
        Duration::from_secs(secs)
    } else if s.ends_with("min") || s.ends_with("m") {
        let mins = s
            .trim_end_matches("min")
            .trim_end_matches("m")
            .parse::<u64>()
            .map_err(serde::de::Error::custom)?;
        Duration::from_secs(mins * 60)
    } else if s.ends_with("hr") || s.ends_with("h") {
        let hrs = s
            .trim_end_matches("hr")
            .trim_end_matches("h")
            .parse::<u64>()
            .map_err(serde::de::Error::custom)?;
        Duration::from_secs(hrs * 3600)
    } else {
        return Err(serde::de::Error::custom("Invalid duration format"));
    };
    Ok(duration)
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            out: default_log_out(),
            error: default_log_error(),
            combined: false,
        }
    }
}

impl Default for RestartConfig {
    fn default() -> Self {
        Self {
            policy: default_restart_policy(),
            max_restarts: default_max_restarts(),
            restart_delay: default_restart_delay(),
        }
    }
}

impl AppConfig {
    pub fn from_file(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: AppConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    // Get all apps from this config, regardless of format
    pub fn get_apps(&self) -> (Option<String>, Vec<App>) {
        match self {
            AppConfig::SingleApp(app) => (None, vec![*app.clone()]),
            AppConfig::MultiApp(projects) => {
                let mut apps = Vec::new();
                for (_, project_apps) in (*projects).iter() {
                    for app in project_apps {
                        apps.push(app.clone());
                    }
                }
                (Some(projects.keys().next().unwrap().to_owned()), apps)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_app() {
        let json = r#"{
            "name": "test-app",
            "script": "node",
            "args": ["app.js"]
        }"#;

        let config: AppConfig = serde_json::from_str(json).unwrap();
        let (_, apps) = config.get_apps();

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "test-app");
        assert_eq!(apps[0].script, "node");
        assert_eq!(apps[0].args, vec!["app.js"]);
    }

    #[test]
    fn test_parse_multi_app() {
        let json = r#"{
            "my-project": [
                {"name": "app1", "script": "node", "args": ["a.js"]},
                {"name": "app2", "script": "python", "args": ["b.py"]}
            ]
        }"#;

        let config: AppConfig = serde_json::from_str(json).unwrap();
        let (project_name, apps) = config.get_apps();

        assert_eq!(project_name, Some("my-project".to_string()));
        assert_eq!(apps.len(), 2);
    }

    #[test]
    fn test_parse_healthcheck() {
        let json = r#"{
            "name": "test-app",
            "script": "node",
            "args": ["app.js"],
            "healthcheck": {
                "type": "tcp",
                "host": "127.0.0.1",
                "port": 3000,
                "interval": "30s",
                "timeout": "5s",
                "retries": 3
            }
        }"#;

        let config: AppConfig = serde_json::from_str(json).unwrap();
        let (_, apps) = config.get_apps();

        let hc = apps[0].healthcheck.as_ref().unwrap();
        assert!(matches!(hc.check_type, HealthCheckType::Tcp));
        assert_eq!(hc.port, Some(3000));
        assert_eq!(hc.retries, 3);
    }

    #[test]
    fn test_parse_restart_policy() {
        let json = r#"{
            "name": "test",
            "script": "node",
            "args": [],
            "restart": {
                "policy": "on-failure",
                "max_restarts": 5,
                "restart_delay": "10s"
            }
        }"#;

        let config: AppConfig = serde_json::from_str(json).unwrap();
        let (_, apps) = config.get_apps();

        assert!(matches!(apps[0].restart.policy, RestartPolicy::OnFailure));
        assert_eq!(apps[0].restart.max_restarts, 5);
    }

    #[test]
    fn test_default_values() {
        let json = r#"{"name": "minimal", "script": "echo", "args": []}"#;

        let config: AppConfig = serde_json::from_str(json).unwrap();
        let (_, apps) = config.get_apps();

        // Check defaults
        assert_eq!(apps[0].log.out, "stdout");
        assert_eq!(apps[0].log.error, "stderr");
        assert!(matches!(apps[0].restart.policy, RestartPolicy::OnFailure));
        assert_eq!(apps[0].restart.max_restarts, -1);
    }
}
