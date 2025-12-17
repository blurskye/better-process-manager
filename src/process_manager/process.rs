//! Process utilities for collecting process tree info
//!
//! Provides utilities for getting descendant processes and combined usage stats.

#![allow(dead_code)] // These utilities are for future use

use once_cell::sync::Lazy;
use std::collections::VecDeque;
use std::error::Error;
use std::sync::Mutex;
use sysinfo::{Pid, System};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("Failed to lock the system mutex")]
    LockError,

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

    let all_pids = collect_descendants(root_pid).unwrap();
    let sys = SYSTEM.lock().unwrap();
    all_pids.iter().for_each(|x| {
        if let Some(process) = sys.process(*x) {
            total_cpu += process.cpu_usage();
            mem_total += process.memory();
        }
    });

    Ok((total_cpu, mem_total))
}
