use clap::Parser;
mod communication;
mod error;
mod process_manager;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    cli_command: CliCommands,
}

#[derive(clap::Subcommand, Debug)]
enum CliCommands {
    Daemon,
    List,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.cli_command {
        CliCommands::Daemon => communication::server::run_server(),
        CliCommands::List => communication::client::run_client(),
    };

    if let Err(e) = result {
        eprintln!("An error occurred: {}", e);
        std::process::exit(1);
    }
}
