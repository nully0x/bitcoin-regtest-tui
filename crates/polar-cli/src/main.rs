use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser, Debug)]
#[command(name = "polar")]
#[command(author, version, about = "Lightning Network development environment in your terminal")]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Launch the interactive TUI
    Tui,
    /// List all networks
    List,
    /// Create a new network
    Create {
        /// Name of the network
        name: String,
    },
    /// Start a network
    Start {
        /// Name of the network
        name: String,
    },
    /// Stop a network
    Stop {
        /// Name of the network
        name: String,
    },
    /// Delete a network
    Delete {
        /// Name of the network
        name: String,
    },
}

fn setup_logging(verbosity: u8) {
    let filter = match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::new(filter))
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    setup_logging(cli.verbose);

    match cli.command {
        Some(Commands::Tui) | None => {
            tracing::info!("Launching TUI...");
            polar_tui::run().await?;
        }
        Some(Commands::List) => {
            // TODO: Implement network listing
            println!("No networks found. Use 'polar create <name>' to create one.");
        }
        Some(Commands::Create { name }) => {
            // TODO: Implement network creation
            println!("Created network: {name}");
        }
        Some(Commands::Start { name }) => {
            // TODO: Implement network start
            println!("Started network: {name}");
        }
        Some(Commands::Stop { name }) => {
            // TODO: Implement network stop
            println!("Stopped network: {name}");
        }
        Some(Commands::Delete { name }) => {
            // TODO: Implement network deletion
            println!("Deleted network: {name}");
        }
    }

    Ok(())
}
