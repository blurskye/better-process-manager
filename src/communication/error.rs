//! Communication Error Types

#![allow(dead_code)] // Error types for future use

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CommunicationError {
    #[error("unforeseen error occurred")]
    Unknown,
}
