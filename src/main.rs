use clap::Parser;

use crate::communication::common::Command;
mod communication;
mod config;
mod error;
mod process_manager;

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    cli_command: CliCommands,
}

#[derive(clap::Subcommand, Debug)]
enum CliCommands {
    Daemon,
    List,
    Status { payload: String },
    Start { payload: String },
    Enable { payload: String },
    Disable { payload: String },
    Delete { payload: String },
    Logs { payload: String },
    Restart { payload: String },
}

fn main() {
    let cli = Cli::parse();

    let command = match cli.cli_command {
        CliCommands::Daemon => communication::server::run_server(),
        CliCommands::List => Command::new_list(),
        CliCommands::Status { payload } => Command::new_status(&payload),
        CliCommands::Start { payload } => Command::new_start(&payload),
        CliCommands::Enable { payload } => Command::new_enable(&payload),
        CliCommands::Disable { payload } => Command::new_disable(&payload),
        CliCommands::Delete { payload } => Command::new_delete(&payload),
        CliCommands::Logs { payload } => Command::new_logs(&payload),
        CliCommands::Restart { payload } => Command::new_restart(&payload),
    };

    // Use the `command` as needed
}
// fn main() {
//     let cli = Cli::parse();
//
//     let result = match cli.cli_command {
//         CliCommands::Daemon => communication::server::run_server(),
//         CliCommands::List => {
//             communication::client::run_client(communication::common::Command::List)
//         }
//     };
//
//     if let Err(e) = result {
//         eprintln!("An error occurred: {}", e);
//         std::process::exit(1);
//     }
// }
