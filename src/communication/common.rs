//! IPC Communication Types
//!
//! Defines commands and message chunks for daemon communication.

use iceoryx2::prelude::ZeroCopySend;

pub const MAX_PAYLOAD_SIZE: usize = 4096;
pub const CHUNK_METADATA_SIZE: usize = std::mem::size_of::<u128>()
    + std::mem::size_of::<u32>()
    + std::mem::size_of::<bool>()
    + std::mem::size_of::<u32>();
pub const CHUNK_PAYLOAD_CAPACITY: usize = MAX_PAYLOAD_SIZE - CHUNK_METADATA_SIZE;

/// Commands that can be sent to the daemon
#[derive(Debug, ZeroCopySend)]
#[repr(C)]
pub enum Command {
    List,
    Status([u8; CHUNK_PAYLOAD_CAPACITY]),
    Start([u8; CHUNK_PAYLOAD_CAPACITY]),
    Stop([u8; CHUNK_PAYLOAD_CAPACITY]),
    Enable([u8; CHUNK_PAYLOAD_CAPACITY]),
    Disable([u8; CHUNK_PAYLOAD_CAPACITY]),
    Delete([u8; CHUNK_PAYLOAD_CAPACITY]),
    Logs([u8; CHUNK_PAYLOAD_CAPACITY]),
    Restart([u8; CHUNK_PAYLOAD_CAPACITY]),
    Flush([u8; CHUNK_PAYLOAD_CAPACITY]),
    Save,
    Resurrect,
}

impl Command {
    pub fn encode_payload(input: &str) -> [u8; CHUNK_PAYLOAD_CAPACITY] {
        let mut buffer = [0u8; CHUNK_PAYLOAD_CAPACITY];
        let bytes = input.as_bytes();
        let len = bytes.len().min(CHUNK_PAYLOAD_CAPACITY);
        buffer[..len].copy_from_slice(&bytes[..len]);
        buffer
    }

    pub fn decode_payload(payload: &[u8]) -> Result<&str, std::str::Utf8Error> {
        let end = payload
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(payload.len());
        std::str::from_utf8(&payload[..end])
    }

    pub fn new_status(input: &str) -> Self {
        Self::Status(Self::encode_payload(input))
    }

    pub fn new_start(input: &str) -> Self {
        Self::Start(Self::encode_payload(input))
    }

    pub fn new_stop(input: &str) -> Self {
        Self::Stop(Self::encode_payload(input))
    }

    pub fn new_enable(input: &str) -> Self {
        Self::Enable(Self::encode_payload(input))
    }

    pub fn new_disable(input: &str) -> Self {
        Self::Disable(Self::encode_payload(input))
    }

    pub fn new_delete(input: &str) -> Self {
        Self::Delete(Self::encode_payload(input))
    }

    pub fn new_logs(input: &str) -> Self {
        Self::Logs(Self::encode_payload(input))
    }

    pub fn new_restart(input: &str) -> Self {
        Self::Restart(Self::encode_payload(input))
    }

    pub fn new_flush(input: &str) -> Self {
        Self::Flush(Self::encode_payload(input))
    }
}

/// Chunked message for large responses
#[derive(Debug, ZeroCopySend)]
#[repr(C)]
pub struct MessageChunk {
    pub sequence_number: u32,
    pub is_last: bool,
    pub used_payload_size: u32,
    pub payload: [u8; CHUNK_PAYLOAD_CAPACITY],
}

/// Get IPC service name with username suffix for multi-user support
pub fn get_ipc_name() -> String {
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string());
    format!("better_process_manager-{}", username)
}

impl Default for MessageChunk {
    fn default() -> Self {
        Self {
            sequence_number: 0,
            is_last: false,
            used_payload_size: 0,
            payload: [0u8; CHUNK_PAYLOAD_CAPACITY],
        }
    }
}

pub trait ChunkPayload {
    fn new(sequence_number: u32, is_last: bool, used_payload_size: u32, payload: Vec<u8>) -> Self;
}

impl ChunkPayload for MessageChunk {
    fn new(sequence_number: u32, is_last: bool, used_payload_size: u32, payload: Vec<u8>) -> Self {
        let mut payload_array = [0u8; CHUNK_PAYLOAD_CAPACITY];
        payload_array[..payload.len()].copy_from_slice(&payload);

        Self {
            sequence_number,
            is_last,
            used_payload_size,
            payload: payload_array,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_ipc_name() {
        let name = get_ipc_name();
        assert!(name.starts_with("better_process_manager-"));
        assert!(name.len() > "better_process_manager-".len());
    }

    #[test]
    fn test_encode_decode_payload() {
        let test_str = "test_process_name";
        let encoded = Command::encode_payload(test_str);
        let decoded = Command::decode_payload(&encoded).unwrap();
        assert_eq!(decoded, test_str);
    }

    #[test]
    fn test_encode_decode_empty() {
        let test_str = "";
        let encoded = Command::encode_payload(test_str);
        let decoded = Command::decode_payload(&encoded).unwrap();
        assert_eq!(decoded, test_str);
    }

    #[test]
    fn test_encode_decode_long_string() {
        let test_str = "a".repeat(CHUNK_PAYLOAD_CAPACITY - 1);
        let encoded = Command::encode_payload(&test_str);
        let decoded = Command::decode_payload(&encoded).unwrap();
        assert_eq!(decoded, test_str);
    }

    #[test]
    fn test_command_constructors() {
        let name = "test_app";
        
        let cmd = Command::new_status(name);
        if let Command::Status(payload) = cmd {
            assert_eq!(Command::decode_payload(&payload).unwrap(), name);
        } else {
            panic!("Expected Status command");
        }

        let cmd = Command::new_start(name);
        if let Command::Start(payload) = cmd {
            assert_eq!(Command::decode_payload(&payload).unwrap(), name);
        } else {
            panic!("Expected Start command");
        }

        let cmd = Command::new_stop(name);
        if let Command::Stop(payload) = cmd {
            assert_eq!(Command::decode_payload(&payload).unwrap(), name);
        } else {
            panic!("Expected Stop command");
        }
    }

    #[test]
    fn test_message_chunk_default() {
        let chunk = MessageChunk::default();
        assert_eq!(chunk.sequence_number, 0);
        assert!(!chunk.is_last);
        assert_eq!(chunk.used_payload_size, 0);
    }

    #[test]
    fn test_message_chunk_new() {
        let data = b"test data".to_vec();
        let chunk = MessageChunk::new(1, true, data.len() as u32, data.clone());
        
        assert_eq!(chunk.sequence_number, 1);
        assert!(chunk.is_last);
        assert_eq!(chunk.used_payload_size, data.len() as u32);
        assert_eq!(&chunk.payload[..data.len()], &data[..]);
    }
}
