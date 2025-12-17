//! Process utilities for collecting process tree info
//!
//! Provides utilities for getting descendant processes and combined usage stats.

use once_cell::sync::Lazy;
use std::collections::VecDeque;
use std::error::Error;
use std::sync::Mutex;
use sysinfo::{Pid, System};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("provided pid was invalid")]
    InvalidPid(u32),
}

static SYSTEM: Lazy<Mutex<System>> = Lazy::new(|| Mutex::new(System::new_all()));

pub fn collect_descendants(root_pid: u32) -> Result<Vec<Pid>, Box<dyn Error>> {
    let root_pid = Pid::from_u32(root_pid);
    let sys = SYSTEM.lock().unwrap();
    if sys.process(root_pid).is_none() {
        return Err(Box::new(ProcessError::InvalidPid(root_pid.as_u32())));
    }
    let mut result = Vec::new();
    let mut queue = VecDeque::new();
    queue.push_back(root_pid);
    while let Some(pid) = queue.pop_front() {
        result.push(pid);
        result.extend(
            sys.processes()
                .iter()
                .filter(|x| x.1.parent() == Some(pid))
                .map(|x| {
                    queue.push_back(*x.0);
                    *x.0
                }),
        );
    }
    Ok(result)
}

pub fn combined_usage(root_pid: u32) -> Result<(f32, u64), Box<dyn Error>> {
    let mut total_cpu = 0.0;
    let mut mem_total = 0;

    let all_pids = collect_descendants(root_pid)?;
    let sys = SYSTEM.lock().unwrap();
    all_pids.iter().for_each(|x| {
        if let Some(process) = sys.process(*x) {
            total_cpu += process.cpu_usage();
            mem_total += process.memory();
        }
    });

    Ok((total_cpu, mem_total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_descendants_invalid_pid() {
        // Use a PID that definitely doesn't exist
        let result = collect_descendants(999999);
        assert!(result.is_err());
    }

    #[test]
    fn test_collect_descendants_self() {
        // Get current process PID
        let pid = std::process::id();
        let result = collect_descendants(pid);
        
        // Should succeed and include at least the current process
        assert!(result.is_ok());
        let descendants = result.unwrap();
        assert!(!descendants.is_empty());
        assert!(descendants.contains(&Pid::from_u32(pid)));
    }

    #[test]
    fn test_combined_usage_invalid_pid() {
        let result = combined_usage(999999);
        assert!(result.is_err());
    }

    #[test]
    fn test_combined_usage_self() {
        let pid = std::process::id();
        let result = combined_usage(pid);
        
        // Should succeed and return some usage stats
        assert!(result.is_ok());
        let (cpu, mem) = result.unwrap();
        // CPU might be 0 but memory should be > 0
        assert!(cpu >= 0.0);
        assert!(mem > 0);
    }
}
