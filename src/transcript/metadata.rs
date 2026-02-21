use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Metadata extracted from a Claude Code session transcript.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub session_id: String,
    pub project_dir: String,
    pub git_branch: Option<String>,
    pub model: Option<String>,

    pub first_timestamp: Option<String>,
    pub last_timestamp: Option<String>,
    pub duration_seconds: Option<i64>,

    pub user_prompts: Vec<String>,
    pub files_modified: HashSet<String>,
    pub files_read: HashSet<String>,
    pub commands_run: Vec<String>,
    pub git_commits: Vec<String>,
    pub tool_counts: HashMap<String, u32>,

    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

impl SessionMetadata {
    /// Compute duration from first/last timestamps.
    pub fn compute_duration(&mut self) {
        if let (Some(first), Some(last)) = (&self.first_timestamp, &self.last_timestamp) {
            if let (Ok(start), Ok(end)) = (
                chrono::DateTime::parse_from_rfc3339(first),
                chrono::DateTime::parse_from_rfc3339(last),
            ) {
                self.duration_seconds = Some((end - start).num_seconds());
            }
        }
    }
}
