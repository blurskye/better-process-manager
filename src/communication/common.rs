use iceoryx2::prelude::ZeroCopySend;

pub(super) const MAX_PAYLOAD_SIZE: usize = 4096;
pub(super) const CHUNK_METADATA_SIZE: usize = std::mem::size_of::<u128>()
    + std::mem::size_of::<u32>()
    + std::mem::size_of::<bool>()
    + std::mem::size_of::<u32>();
pub(super) const CHUNK_PAYLOAD_CAPACITY: usize = MAX_PAYLOAD_SIZE - CHUNK_METADATA_SIZE;

#[derive(Debug, ZeroCopySend)]
#[repr(C)]
pub enum Command {
    Daemon,
    List,
    Status([u8; CHUNK_PAYLOAD_CAPACITY]),
    Start([u8; CHUNK_PAYLOAD_CAPACITY]),
    Enable([u8; CHUNK_PAYLOAD_CAPACITY]),
    Disable([u8; CHUNK_PAYLOAD_CAPACITY]),
    Delete([u8; CHUNK_PAYLOAD_CAPACITY]),
    Logs([u8; CHUNK_PAYLOAD_CAPACITY]),
    Restart([u8; CHUNK_PAYLOAD_CAPACITY]),
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
    pub fn new_daemon() -> Self {
        Self::Daemon
    }
    pub fn new_list() -> Self {
        Self::List
    }

    pub fn new_status(input: &str) -> Self {
        Self::Status(Self::encode_payload(input))
    }

    pub fn new_start(input: &str) -> Self {
        Self::Start(Self::encode_payload(input))
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
}

#[derive(Debug, ZeroCopySend)]
#[repr(C)]
pub(super) struct MessageChunk {
    pub(super) sequence_number: u32,
    pub(super) is_last: bool,
    pub(super) used_payload_size: u32,
    pub(super) payload: [u8; CHUNK_PAYLOAD_CAPACITY],
}

pub(super) const IPC_NAME: &str = "better_process_manager";

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
