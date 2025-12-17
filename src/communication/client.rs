use iceoryx2::prelude::*;

use crate::communication::common;
use std::collections::BTreeMap;
use std::time::Duration;

/// Auto-start daemon if not running and send command
pub fn run_client(command: common::Command) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::default();
    let node = NodeBuilder::new()
        .config(&config)
        .create::<ipc::Service>()?;

    let service_name = common::get_ipc_name();

    // Try to connect, auto-start daemon if needed
    if !crate::communication::server::server_running(&node, &service_name)? {
        eprintln!("Daemon not running. Start it with: bpm daemon");
        return Err("Daemon not running".into());
    }

    match request_server(&node, &service_name, command, Duration::from_secs(5)) {
        Ok(response) => {
            println!("{}", response);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

/// Run the monitoring dashboard (TUI)
#[allow(dead_code)] // TUI dashboard for future 'monit' command
pub fn run_monit() -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::{
        event::{self, Event, KeyCode},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    };
    use ratatui::prelude::*;
    use std::io::stdout;

    let config = Config::default();
    let node = NodeBuilder::new()
        .config(&config)
        .create::<ipc::Service>()?;

    let service_name = common::get_ipc_name();

    // Check if daemon is running
    if !crate::communication::server::server_running(&node, &service_name)? {
        eprintln!("Daemon not running. Start it with: bpm daemon");
        return Err("Daemon not running".into());
    }

    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    loop {
        // Get process list
        let response = request_server(
            &node,
            &service_name,
            common::Command::List,
            Duration::from_secs(2),
        )
        .unwrap_or_else(|_| "Failed to get process list".to_string());

        terminal.draw(|frame| {
            let area = frame.area();

            let block = ratatui::widgets::Block::default()
                .title(" BPM Monitor (q to quit) ")
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan));

            let paragraph = ratatui::widgets::Paragraph::new(response.clone())
                .block(block)
                .style(Style::default().fg(Color::White));

            frame.render_widget(paragraph, area);
        })?;

        // Handle input
        if event::poll(Duration::from_millis(1000))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    break;
                }
            }
        }
    }

    // Cleanup terminal
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

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

    let pending_response = client.send_copy(command)?;

    let mut received_chunks = BTreeMap::new();
    let mut message_complete = false;

    let start_time = std::time::Instant::now();

    while !message_complete && start_time.elapsed() < timeout {
        if let Some(response) = pending_response.receive()? {
            let chunk = response.payload();

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
