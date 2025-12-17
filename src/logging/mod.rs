//! Logging Module  
//!
//! Implements log management including rotation, streaming, and formatting.

#![allow(dead_code)] // These utilities are for future use

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Configuration for log rotation
#[derive(Debug, Clone)]
pub struct LogRotationConfig {
    /// Maximum size in bytes before rotating
    pub max_size: u64,
    /// Maximum number of rotated files to keep
    pub max_files: u32,
    /// Whether to compress rotated files
    pub compress: bool,
}

impl Default for LogRotationConfig {
    fn default() -> Self {
        Self {
            max_size: 10 * 1024 * 1024, // 10MB
            max_files: 5,
            compress: false,
        }
    }
}

/// Log manager for a process
pub struct LogManager {
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    rotation_config: LogRotationConfig,
}

impl LogManager {
    /// Create a new log manager
    pub fn new(stdout_path: PathBuf, stderr_path: PathBuf) -> Self {
        Self {
            stdout_path,
            stderr_path,
            rotation_config: LogRotationConfig::default(),
        }
    }

    /// Set rotation configuration
    pub fn with_rotation(mut self, config: LogRotationConfig) -> Self {
        self.rotation_config = config;
        self
    }

    /// Get the last N lines from stdout
    pub fn tail_stdout(&self, lines: usize) -> io::Result<Vec<String>> {
        tail_file(&self.stdout_path, lines)
    }

    /// Get the last N lines from stderr
    pub fn tail_stderr(&self, lines: usize) -> io::Result<Vec<String>> {
        tail_file(&self.stderr_path, lines)
    }

    /// Get combined logs (interleaved by timestamp if available)
    pub fn get_combined_logs(&self, lines: usize) -> io::Result<String> {
        let stdout_lines = self.tail_stdout(lines)?;
        let stderr_lines = self.tail_stderr(lines)?;

        let mut output = String::new();
        output.push_str("=== stdout ===\n");
        for line in &stdout_lines {
            output.push_str(line);
            output.push('\n');
        }
        output.push_str("\n=== stderr ===\n");
        for line in &stderr_lines {
            output.push_str(line);
            output.push('\n');
        }

        Ok(output)
    }

    /// Check if rotation is needed and perform it
    pub fn rotate_if_needed(&self) -> io::Result<()> {
        self.maybe_rotate(&self.stdout_path)?;
        self.maybe_rotate(&self.stderr_path)?;
        Ok(())
    }

    /// Rotate a specific log file if needed
    fn maybe_rotate(&self, path: &Path) -> io::Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let metadata = fs::metadata(path)?;
        if metadata.len() < self.rotation_config.max_size {
            return Ok(());
        }

        // Rotate files
        for i in (1..self.rotation_config.max_files).rev() {
            let old_path = format!("{}.{}", path.display(), i);
            let new_path = format!("{}.{}", path.display(), i + 1);
            if Path::new(&old_path).exists() {
                fs::rename(&old_path, &new_path)?;
            }
        }

        // Move current file to .1
        let new_path = format!("{}.1", path.display());
        fs::rename(path, &new_path)?;

        // Create new empty log file
        File::create(path)?;

        // Delete oldest if we have too many
        let oldest = format!("{}.{}", path.display(), self.rotation_config.max_files + 1);
        if Path::new(&oldest).exists() {
            fs::remove_file(&oldest)?;
        }

        Ok(())
    }

    /// Flush logs (truncate both stdout and stderr)
    pub fn flush(&self) -> io::Result<()> {
        if self.stdout_path.exists() {
            OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&self.stdout_path)?;
        }
        if self.stderr_path.exists() {
            OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&self.stderr_path)?;
        }
        Ok(())
    }

    /// Get log directory
    pub fn log_dir(&self) -> Option<&Path> {
        self.stdout_path.parent()
    }
}

/// Read the last N lines from a file
fn tail_file(path: &Path, lines: usize) -> io::Result<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let all_lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();
    let start = all_lines.len().saturating_sub(lines);

    Ok(all_lines[start..].to_vec())
}

/// Stream new lines from a file (for follow mode)
pub struct LogStreamer {
    file: File,
    path: PathBuf,
    position: u64,
}

impl LogStreamer {
    /// Create a new log streamer, starting from the end of the file
    pub fn new(path: PathBuf) -> io::Result<Self> {
        let mut file = File::open(&path)?;
        let position = file.seek(SeekFrom::End(0))?;

        Ok(Self {
            file,
            path,
            position,
        })
    }

    /// Create a new log streamer, starting from N lines before the end
    pub fn with_tail(path: PathBuf, lines: usize) -> io::Result<Self> {
        let file = File::open(&path)?;
        let reader = BufReader::new(&file);

        let all_lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();
        let start_line = all_lines.len().saturating_sub(lines);

        // Calculate position for start_line
        let mut position = 0u64;
        for (i, line) in all_lines.iter().enumerate() {
            if i >= start_line {
                break;
            }
            position += line.len() as u64 + 1; // +1 for newline
        }

        let mut file = File::open(&path)?;
        file.seek(SeekFrom::Start(position))?;

        Ok(Self {
            file,
            path,
            position,
        })
    }

    /// Read any new lines since last read
    pub fn read_new(&mut self) -> io::Result<Vec<String>> {
        // Check if file was rotated (size smaller than our position)
        let metadata = fs::metadata(&self.path)?;
        if metadata.len() < self.position {
            // File was rotated, restart from beginning
            self.file = File::open(&self.path)?;
            self.position = 0;
        }

        // Seek to our saved position
        self.file.seek(SeekFrom::Start(self.position))?;

        let reader = BufReader::new(&self.file);
        let lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();

        // Update position
        if !lines.is_empty() {
            self.file.seek(SeekFrom::End(0))?;
            self.position = self.file.stream_position()?;
        }

        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_tail_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.log");

        let mut file = File::create(&file_path).unwrap();
        for i in 1..=100 {
            writeln!(file, "Line {}", i).unwrap();
        }
        drop(file);

        let lines = tail_file(&file_path, 10).unwrap();
        assert_eq!(lines.len(), 10);
        assert_eq!(lines[0], "Line 91");
        assert_eq!(lines[9], "Line 100");
    }

    #[test]
    fn test_log_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let stdout_path = temp_dir.path().join("out.log");
        let stderr_path = temp_dir.path().join("error.log");

        // Create a small log file
        let mut file = File::create(&stdout_path).unwrap();
        for _ in 0..1000 {
            writeln!(file, "Some log content that takes up space").unwrap();
        }
        drop(file);

        let manager =
            LogManager::new(stdout_path.clone(), stderr_path).with_rotation(LogRotationConfig {
                max_size: 1000, // Very small for testing
                max_files: 3,
                compress: false,
            });

        manager.rotate_if_needed().unwrap();

        // Should have rotated
        assert!(Path::new(&format!("{}.1", stdout_path.display())).exists());
    }
}
