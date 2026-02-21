mod cli;
mod config;
mod db;
mod mcp;
mod transcript;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use cli::IngestFormat;

#[derive(Parser)]
#[command(name = "claude-memory", about = "Automatic session logging and recall for Claude Code")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ingest a session transcript (called by SessionEnd hook, or via --format)
    Ingest {
        /// Input format. Defaults to 'claude' (reads hook JSON from stdin).
        #[arg(short, long, default_value = "claude")]
        format: IngestFormat,
        /// Path to transcript file. If omitted, reads from stdin.
        #[arg(short = 'F', long)]
        file: Option<PathBuf>,
    },
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
        Commands::Ingest { format, file } => cli::ingest::run(format, file)?,
        Commands::Serve => mcp::server::run()?,
        Commands::Install => cli::install::run()?,
        Commands::Status => cli::status::run()?,
        Commands::Search { query, limit } => cli::search::run(&query, limit)?,
    }

    Ok(())
}
