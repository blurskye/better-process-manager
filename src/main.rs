use clap::Parser;

use crate::communication::common::Command;
mod communication;
mod config;
mod error;
mod logging;
mod process_manager;

#[derive(Parser, Debug)]
#[command(name = "bpm")]
#[command(about = "Better Process Manager - A PM2 alternative in Rust")]
struct Cli {
    #[command(subcommand)]
    cli_command: CliCommands,
}

#[derive(clap::Subcommand, Debug)]
enum CliCommands {
    /// Start the daemon process
    Daemon,
    /// List all managed processes
    List,
    /// Show status of a specific process
    Status { name: String },
    /// Start a process from config file or by name
    Start { payload: String },
    /// Stop a running process
    Stop { name: String },
    /// Enable a process (add to managed list)
    Enable { payload: String },
    /// Disable a process (remove from managed list but don't stop)
    Disable { payload: String },
    /// Delete a process (stop and remove)
    Delete { payload: String },
    /// View logs for a process
    Logs {
        name: String,
        /// Number of lines to show
        #[arg(short = 'n', long, default_value = "20")]
        lines: usize,
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
    },
    /// Restart a process
    Restart { name: String },
    /// Flush logs for a process
    Flush { name: Option<String> },
    /// Save current process list
    Save,
    /// Resurrect saved processes
    Resurrect,
    /// Generate startup script
    Startup,
    /// Open monitoring dashboard
    Monit,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.cli_command {
        CliCommands::Daemon => communication::server::run_server(),
        CliCommands::List => communication::client::run_client(Command::List),
        CliCommands::Status { name } => {
            communication::client::run_client(Command::new_status(&name))
        }
        CliCommands::Start { payload } => {
            communication::client::run_client(Command::new_start(&payload))
        }
        CliCommands::Stop { name } => communication::client::run_client(Command::new_stop(&name)),
        CliCommands::Enable { payload } => {
            communication::client::run_client(Command::new_enable(&payload))
        }
        CliCommands::Disable { payload } => {
            communication::client::run_client(Command::new_disable(&payload))
        }
        CliCommands::Delete { payload } => {
            communication::client::run_client(Command::new_delete(&payload))
        }
        CliCommands::Logs {
            name,
            lines,
            follow,
        } => {
            let payload = format!("{}:{}:{}", name, lines, follow);
            communication::client::run_client(Command::new_logs(&payload))
        }
        CliCommands::Restart { name } => {
            communication::client::run_client(Command::new_restart(&name))
        }
        CliCommands::Flush { name } => {
            let payload = name.unwrap_or_default();
            communication::client::run_client(Command::new_flush(&payload))
        }
        CliCommands::Save => communication::client::run_client(Command::Save),
        CliCommands::Resurrect => communication::client::run_client(Command::Resurrect),
        CliCommands::Startup => {
            // Generate startup script locally, no daemon needed
            match config::startup::generate_startup_script() {
                Ok(path) => {
                    println!("Startup script generated at: {}", path.display());
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        CliCommands::Monit => communication::client::run_monit(),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
