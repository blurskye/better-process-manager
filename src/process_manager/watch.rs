//! Watch Mode Module
//!
//! Implements file watching to automatically reload processes when source files change.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

/// File watcher that monitors directories for changes
#[derive(Debug)]
pub struct FileWatcher {
    /// Map of paths to their last modified times
    file_times: Arc<RwLock<HashMap<PathBuf, SystemTime>>>,
    /// Directories being watched
    watch_dirs: Vec<PathBuf>,
    /// File patterns to watch (e.g., "*.js", "*.py")
    patterns: Vec<String>,
    /// Directories to ignore
    ignore_dirs: Vec<String>,
}

impl FileWatcher {
    /// Create a new file watcher
    pub fn new(watch_dirs: Vec<PathBuf>, patterns: Vec<String>) -> Self {
        Self {
            file_times: Arc::new(RwLock::new(HashMap::new())),
            watch_dirs,
            patterns,
            ignore_dirs: vec![
                "node_modules".to_string(),
                ".git".to_string(),
                "target".to_string(),
                "__pycache__".to_string(),
                ".venv".to_string(),
                "venv".to_string(),
            ],
        }
    }

    /// Add a directory to ignore
    pub fn ignore(&mut self, dir: String) {
        self.ignore_dirs.push(dir);
    }

    /// Initialize the watcher by recording all current file times
    pub fn init(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut file_times = self.file_times.write().map_err(|e| e.to_string())?;

        for dir in &self.watch_dirs {
            self.scan_directory(dir, &mut file_times)?;
        }

        Ok(())
    }

    /// Scan a directory recursively and record file modification times
    fn scan_directory(
        &self,
        dir: &Path,
        file_times: &mut HashMap<PathBuf, SystemTime>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !dir.exists() || !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Skip ignored directories
            if path.is_dir() {
                if self.ignore_dirs.iter().any(|d| file_name == d) {
                    continue;
                }
                self.scan_directory(&path, file_times)?;
            } else if self.matches_pattern(&path) {
                if let Ok(metadata) = path.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        file_times.insert(path, modified);
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a path matches any of our watch patterns
    fn matches_pattern(&self, path: &Path) -> bool {
        if self.patterns.is_empty() {
            return true; // Watch all files if no patterns specified
        }

        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        for pattern in &self.patterns {
            if pattern.starts_with("*.") {
                let pattern_ext = pattern.trim_start_matches("*.");
                if extension == pattern_ext {
                    return true;
                }
            } else if file_name == pattern {
                return true;
            } else if file_name.contains(pattern.trim_start_matches('*').trim_end_matches('*')) {
                return true;
            }
        }

        false
    }

    /// Check for changes since last scan
    /// Returns list of changed files
    pub fn check_changes(&self) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
        let mut current_times: HashMap<PathBuf, SystemTime> = HashMap::new();

        for dir in &self.watch_dirs {
            self.scan_directory(dir, &mut current_times)?;
        }

        let mut changed = Vec::new();
        let file_times = self.file_times.read().map_err(|e| e.to_string())?;

        // Check for new or modified files
        for (path, current_time) in &current_times {
            match file_times.get(path) {
                Some(old_time) if current_time != old_time => {
                    changed.push(path.clone());
                }
                None => {
                    changed.push(path.clone());
                }
                _ => {}
            }
        }

        // Check for deleted files
        for path in file_times.keys() {
            if !current_times.contains_key(path) {
                changed.push(path.clone());
            }
        }

        // Update stored times
        drop(file_times);
        let mut file_times = self.file_times.write().map_err(|e| e.to_string())?;
        *file_times = current_times;

        Ok(changed)
    }
}

/// Watch configuration
#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub enabled: bool,
    pub directories: Vec<PathBuf>,
    pub patterns: Vec<String>,
    pub ignore: Vec<String>,
    pub delay: Duration,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            directories: vec![PathBuf::from(".")],
            patterns: vec![],
            ignore: vec!["node_modules".to_string(), ".git".to_string()],
            delay: Duration::from_millis(1000),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_file_watcher() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.js");

        // Create initial file
        let mut file = File::create(&test_file).unwrap();
        writeln!(file, "console.log('hello');").unwrap();
        drop(file);

        let watcher = FileWatcher::new(
            vec![temp_dir.path().to_path_buf()],
            vec!["*.js".to_string()],
        );
        watcher.init().unwrap();

        // No changes initially
        let changes = watcher.check_changes().unwrap();
        assert!(changes.is_empty());

        // Modify file
        std::thread::sleep(Duration::from_millis(100));
        let mut file = File::create(&test_file).unwrap();
        writeln!(file, "console.log('world');").unwrap();
        drop(file);

        // Detect change
        let changes = watcher.check_changes().unwrap();
        assert!(!changes.is_empty());
    }
}
