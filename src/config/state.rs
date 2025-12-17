use super::read_config::{AppConfig, AppReference};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BpmConfig {
    pub enabled: HashMap<String, AppReference>, // Apps user enabled
    pub disabled: HashMap<String, AppReference>, // Apps user disabled
    pub deleted: HashMap<String, AppReference>, // Apps user deleted (for cleanup)
    pub last_updated: SystemTime,
}

impl Default for BpmConfig {
    fn default() -> Self {
        Self {
            enabled: HashMap::new(),
            disabled: HashMap::new(),
            deleted: HashMap::new(),
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

    pub fn enable_apps_from_config(
        &mut self,
        config_path: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let config = AppConfig::from_file(&config_path)?;
        let apps = config.get_apps();
        for app in apps.1 {
            let app_ref = AppReference {
                config_path: config_path.clone(),
                checksum: Self::calculate_checksum(&config_path),
            };

            // Remove from disabled/deleted if exists
            self.disabled.remove(&app.name);
            self.deleted.remove(&app.name);
            self.enabled.insert(app.name.clone(), app_ref);
        }

        self.last_updated = SystemTime::now();
        Ok(())
    }

    pub fn disable_app(&mut self, name: &str) {
        if let Some(app_ref) = self.enabled.remove(name) {
            let disabled_ref = app_ref;
            self.disabled.insert(name.to_string(), disabled_ref);
            self.last_updated = SystemTime::now();
        }
    }

    pub fn delete_app(&mut self, name: &str) {
        let app_ref = self
            .enabled
            .remove(name)
            .or_else(|| self.disabled.remove(name));

        if let Some(app_ref) = app_ref {
            self.deleted.insert(name.to_string(), app_ref);
            self.last_updated = SystemTime::now();
        }
    }

    fn calculate_checksum(path: &PathBuf) -> Option<String> {
        std::fs::read_to_string(path).ok().map(|content| {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            content.hash(&mut hasher);
            format!("{:x}", hasher.finish())
        })
    }
}

pub struct BpmState {
    pub enabled: HashMap<String, AppReference>, // Apps user enabled
    pub disabled: HashMap<String, AppReference>, // Apps user disabled
    pub runing: HashMap<String, AppReference>,
}
