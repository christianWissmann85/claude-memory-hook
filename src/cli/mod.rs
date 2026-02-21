pub mod ingest;
pub mod install;
pub mod search;
pub mod status;

use clap::ValueEnum;

/// Transcript format to ingest.
#[derive(Debug, Clone, ValueEnum, Default)]
pub enum IngestFormat {
    /// Claude Code JSONL transcript (default, via SessionEnd hook on stdin)
    #[default]
    Claude,
    /// GitHub Copilot Chat JSON (from the claude-memory VS Code extension)
    Copilot,
}
