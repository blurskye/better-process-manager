use iceoryx2::prelude::*;

use crate::communication::common;
use std::collections::BTreeMap;
use std::time::Duration;

pub fn run_client(command: common::Command) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::default();
    let node = NodeBuilder::new()
        .config(&config)
        .create::<ipc::Service>()?;

    let service_name = common::IPC_NAME;

    println!("Requesting 'list' command from server...");
    match request_server(&node, service_name, command, Duration::from_secs(5)) {
        Ok(response) => {
            // println!("\n--- Server Response ---\n{}", response);
            println!("{}", response);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }

    Ok(())
}

fn request_server<Service>(
    node: &Node<Service>,
    service_name: &str,
    command: common::Command,
    timeout: Duration,
) -> Result<String, Box<dyn std::error::Error>>
where
    Service: iceoryx2::service::Service,
{
    let service = node
        .service_builder(&service_name.try_into()?)
        .request_response::<common::Command, common::MessageChunk>()
        .open_or_create()?;

    let client = service.client_builder().create()?;

    // println!("Sending command to server...");
    let pending_response = client.send_copy(command)?;

    let mut received_chunks = BTreeMap::new();
    let mut message_complete = false;

    let start_time = std::time::Instant::now();

    while !message_complete && start_time.elapsed() < timeout {
        if let Some(response) = pending_response.receive()? {
            let chunk = response.payload();
            // println!(
            //     "  < Received chunk {} (last: {})",
            //     chunk.sequence_number, chunk.is_last
            // );

            let payload_data = chunk.payload[..chunk.used_payload_size as usize].to_vec();
            received_chunks.insert(chunk.sequence_number, payload_data);

            if chunk.is_last {
                message_complete = true;
            }
        } else {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    if !message_complete {
        return Err("Error: Timed out waiting for complete response from server.".into());
    }

    let mut full_message_bytes = Vec::new();
    for (_, chunk_data) in received_chunks {
        full_message_bytes.extend(chunk_data);
    }

    let final_output = String::from_utf8(full_message_bytes)?;
    Ok(final_output)
}
