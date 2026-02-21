mod cli;
mod config;
mod db;
mod mcp;
mod transcript;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "claude-memory", about = "Automatic session logging and recall for Claude Code")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ingest a session transcript (called automatically by the SessionEnd hook)
    Ingest,
    /// Start MCP server for recall during sessions
    Serve,
    /// Install hooks and MCP configuration
    Install,
    /// Show database statistics for current project
    Status,
    /// Search past sessions from the command line
    Search {
        /// Search query (FTS5 syntax)
        query: String,
        /// Maximum results
        #[arg(short, long, default_value = "5")]
        limit: usize,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Ingest => cli::ingest::run()?,
        Commands::Serve => mcp::server::run()?,
        Commands::Install => cli::install::run()?,
        Commands::Status => cli::status::run()?,
        Commands::Search { query, limit } => cli::search::run(&query, limit)?,
    }

    Ok(())
}
