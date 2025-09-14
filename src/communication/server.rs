use iceoryx2::prelude::*;
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::communication::common::ChunkPayload;
use iceoryx2::active_request::ActiveRequest;
use iceoryx2::service::builder::request_response::RequestResponseOpenError;
use std::time::Duration;
const CYCLE_TIME: Duration = Duration::from_secs(1);

use crate::communication::common;

fn generate_fake_process_list() -> String {
    let mut content = String::new();
    content.push_str("PID\tCPU\tMEM\tCOMMAND\n");
    for i in 0..10000 {
        let line = format!(
            "{}\t{:.2}\t{:.1}M\t/usr/bin/fake-process-long-name-{}\n",
            1000 + i,
            i as f32 * 0.1,
            i as f32 * 2.5,
            i
        );
        content.push_str(&line);
    }
    content
}
pub fn server_running<Service>(
    node: &Node<Service>,
    service_name: &str,
) -> Result<bool, Box<dyn std::error::Error>>
where
    Service: iceoryx2::service::Service,
{
    let service_check = node
        .service_builder(&service_name.try_into()?)
        .request_response::<common::Command, common::MessageChunk>()
        .open();

    match service_check {
        Ok(_) => Ok(true), // Service exists, server is running
        Err(RequestResponseOpenError::DoesNotExist) => Ok(false), // Service does not exist, server is not running
        Err(e) => Err(e.into()),                                  // Propagate other errors
    }
}
pub fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::default();
    let node = NodeBuilder::new()
        .config(&config) // and provide it to the NodeBuilder
        .create::<ipc::Service>()?;

    if server_running(&node, common::IPC_NAME)? {
        eprintln!("Another instance of the server is already running.");
        std::process::exit(1); // Exit with an error code
    }

    let service_name = common::IPC_NAME.try_into()?;
    let service = node
        .service_builder(&service_name)
        .request_response::<common::Command, common::MessageChunk>()
        .open_or_create()?;

    let server = service.server_builder().create()?;

    println!("daemon started");
    while node.wait(Duration::from_millis(100)).is_ok() {
        while let Some(request) = server.receive()? {
            // match *request {
            //     common::Command::List => {
            //         println!("Received 'list' command. Generating and sending response...");
            //         let large_response = generate_fake_process_list();
            //         let response_bytes = large_response.as_bytes();
            //         let message_uuid = Uuid::new_v4().as_u128();
            //
            //         let mut chunks = response_bytes
            //             .chunks(common::CHUNK_PAYLOAD_CAPACITY)
            //             .peekable();
            //         let mut seq_num = 0;
            //
            //         while let Some(chunk_data) = chunks.next() {
            //             let is_last_chunk = chunks.peek().is_none();
            //             let mut payload_array = [0u8; common::CHUNK_PAYLOAD_CAPACITY];
            //             payload_array[..chunk_data.len()].copy_from_slice(chunk_data);
            //
            //             let chunk = common::MessageChunk {
            //                 uuid: message_uuid,
            //                 sequence_number: seq_num,
            //                 is_last: is_last_chunk,
            //                 used_payload_size: chunk_data.len() as u32,
            //                 payload: payload_array,
            //             };
            //
            //             println!(
            //                 "  > Sending chunk {} (last: {}) of size {}",
            //                 seq_num,
            //                 is_last_chunk,
            //                 chunk_data.len()
            //             );
            //             request.send_copy(chunk)?;
            //             seq_num += 1;
            //         }
            //     }
            //     _ => {}
            // }
            //

            match *request {
                common::Command::List => {
                    println!("Received 'list' command. Generating and sending response...");
                    let large_response = generate_fake_process_list();
                    send_response(&request, large_response, common::CHUNK_PAYLOAD_CAPACITY)?;
                }
                _ => {}
            }
        }
    }
    return Ok(());
}
pub fn send_response<Service, RequestPayload, RequestHeader, ResponsePayload, ResponseHeader>(
    request: &ActiveRequest<
        Service,
        RequestPayload,
        RequestHeader,
        ResponsePayload,
        ResponseHeader,
    >,
    response_data: impl AsRef<[u8]>,
    chunk_capacity: usize,
) -> Result<(), Box<dyn std::error::Error>>
where
    Service: iceoryx2::service::Service,
    RequestPayload: std::fmt::Debug + iceoryx2::prelude::ZeroCopySend + ?Sized,
    RequestHeader: std::fmt::Debug + iceoryx2::prelude::ZeroCopySend,
    ResponsePayload: ChunkPayload + std::fmt::Debug + iceoryx2::prelude::ZeroCopySend,
    ResponseHeader: std::fmt::Debug + iceoryx2::prelude::ZeroCopySend + Default,
{
    let response_bytes = response_data.as_ref();
    let message_uuid = uuid::Uuid::new_v4().as_u128();

    let mut chunks = response_bytes.chunks(chunk_capacity).peekable();
    let mut seq_num = 0;

    while let Some(chunk_data) = chunks.next() {
        let is_last_chunk = chunks.peek().is_none();
        let chunk = ResponsePayload::new(
            message_uuid,
            seq_num,
            is_last_chunk,
            chunk_data.len() as u32,
            chunk_data.to_vec(),
        );

        println!(
            "  > Sending chunk {} (last: {}) of size {}",
            seq_num,
            is_last_chunk,
            chunk_data.len()
        );
        request.send_copy(chunk)?; // Send the chunk
        seq_num += 1;
    }

    Ok(())
}
