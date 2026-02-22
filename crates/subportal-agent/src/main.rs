mod agent;
mod enrollment;
mod hub;
mod router;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "subportal-agent",
    about = "subportal agent -- bridges server-side tools to desktop clients"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Start the agent (default)
    Run,
    /// Generate an enrollment ticket (requires running agent)
    Ticket {
        /// Token TTL in seconds
        #[arg(long, default_value_t = subportal_iroh::consts::DEFAULT_TOKEN_TTL_SECS)]
        ttl: u64,
    },
    /// List enrolled clients
    Clients,
    /// Revoke an enrolled client
    Revoke {
        /// Client name or endpoint ID
        name_or_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Run) {
        Command::Run => agent::run().await,
        Command::Ticket { ttl } => enrollment::print_ticket(ttl).await,
        Command::Clients => enrollment::list_clients().await,
        Command::Revoke { name_or_id } => enrollment::revoke_client(&name_or_id).await,
    }
}
