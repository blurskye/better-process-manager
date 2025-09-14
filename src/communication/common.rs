// use iceoryx2::prelude::ZeroCopySend;
//
// pub(super) const MAX_PAYLOAD_SIZE: usize = 4096;
// pub(super) const CHUNK_METADATA_SIZE: usize = std::mem::size_of::<u128>()
//     + std::mem::size_of::<u32>()
//     + std::mem::size_of::<bool>()
//     + std::mem::size_of::<u32>();
// pub(super) const CHUNK_PAYLOAD_CAPACITY: usize = MAX_PAYLOAD_SIZE - CHUNK_METADATA_SIZE;
//
// #[derive(Debug, ZeroCopySend)]
// #[repr(C)]
// pub(super) enum Command {
//     List,
//     Status,
//     Start,
//     Enable,
//     Disable,
//     Delete,
//     Logs,
//     Restart,
// }
//
// #[derive(Debug, ZeroCopySend)]
// #[repr(C)]
// pub(super) struct MessageChunk {
//     pub(super) uuid: u128,
//     pub(super) sequence_number: u32,
//     pub(super) is_last: bool,
//     pub(super) used_payload_size: u32,
//     pub(super) payload: [u8; CHUNK_PAYLOAD_CAPACITY],
// }
//
// pub(super) const IPC_NAME: &str = "better_process_manager";
//
// impl Default for MessageChunk {
//     fn default() -> Self {
//         Self {
//             uuid: 0,
//             sequence_number: 0,
//             is_last: false,
//             used_payload_size: 0,
//             payload: [0u8; CHUNK_PAYLOAD_CAPACITY],
//         }
//     }
// }
//

use iceoryx2::prelude::ZeroCopySend;

pub(super) const MAX_PAYLOAD_SIZE: usize = 4096;
pub(super) const CHUNK_METADATA_SIZE: usize = std::mem::size_of::<u128>()
    + std::mem::size_of::<u32>()
    + std::mem::size_of::<bool>()
    + std::mem::size_of::<u32>();
pub(super) const CHUNK_PAYLOAD_CAPACITY: usize = MAX_PAYLOAD_SIZE - CHUNK_METADATA_SIZE;

#[derive(Debug, ZeroCopySend)]
#[repr(C)]
pub(super) enum Command {
    List,
    Status,
    Start,
    Enable,
    Disable,
    Delete,
    Logs,
    Restart,
}

#[derive(Debug, ZeroCopySend)]
#[repr(C)]
pub(super) struct MessageChunk {
    pub(super) uuid: u128,
    pub(super) sequence_number: u32,
    pub(super) is_last: bool,
    pub(super) used_payload_size: u32,
    pub(super) payload: [u8; CHUNK_PAYLOAD_CAPACITY],
}

pub(super) const IPC_NAME: &str = "better_process_manager";

impl Default for MessageChunk {
    fn default() -> Self {
        Self {
            uuid: 0,
            sequence_number: 0,
            is_last: false,
            used_payload_size: 0,
            payload: [0u8; CHUNK_PAYLOAD_CAPACITY],
        }
    }
}

pub trait ChunkPayload {
    fn new(
        uuid: u128,
        sequence_number: u32,
        is_last: bool,
        used_payload_size: u32,
        payload: Vec<u8>,
    ) -> Self;
}

impl ChunkPayload for MessageChunk {
    fn new(
        uuid: u128,
        sequence_number: u32,
        is_last: bool,
        used_payload_size: u32,
        payload: Vec<u8>,
    ) -> Self {
        let mut payload_array = [0u8; CHUNK_PAYLOAD_CAPACITY];
        payload_array[..payload.len()].copy_from_slice(&payload);

        Self {
            uuid,
            sequence_number,
            is_last,
            used_payload_size,
            payload: payload_array,
        }
    }
}
