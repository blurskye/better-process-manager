//! Config state management
//!
//! Tracks enabled/disabled applications.

#![allow(dead_code)] // For future use

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

/// Reference to an app's config file
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppReference {
    pub config_path: PathBuf,
    pub checksum: Option<String>,
}

/// Persistent BPM configuration state
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BpmConfig {
    pub enabled: HashMap<String, AppReference>,
    pub disabled: HashMap<String, AppReference>,
    pub last_updated: SystemTime,
}

impl Default for BpmConfig {
    fn default() -> Self {
        Self {
            enabled: HashMap::new(),
            disabled: HashMap::new(),
            last_updated: SystemTime::now(),
        }
    }
}

impl BpmConfig {
    pub fn load_or_create(state_path: &PathBuf) -> Self {
        if state_path.exists() {
            std::fs::read_to_string(state_path)
                .ok()
                .and_then(|content| serde_json::from_str(&content).ok())
                .unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self, state_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        if let Some(parent) = state_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(state_path, content)?;
        Ok(())
    }
}
