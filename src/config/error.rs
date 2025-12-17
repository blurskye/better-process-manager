//! Config Error Types

#![allow(dead_code)] // Error types for future use

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("unforeseen error occurred")]
    Unknown,
}
