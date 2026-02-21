use std::io::{BufRead, BufReader};
use std::path::Path;

use serde_json::Value;

use super::metadata::SessionMetadata;

const MAX_COMMANDS: usize = 50;
const MAX_COMMAND_LEN: usize = 200;

/// Parse a Claude Code transcript JSONL file, extracting session metadata.
/// Streams line-by-line to handle large files efficiently.
pub fn parse_transcript(path: &Path) -> anyhow::Result<SessionMetadata> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    let mut meta = SessionMetadata::default();
    let mut seen_commands: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let value: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue, // Skip corrupt lines
        };

        // Update timestamp tracking
        if let Some(ts) = value.get("timestamp").and_then(|t| t.as_str()) {
            if meta.first_timestamp.is_none() {
                meta.first_timestamp = Some(ts.to_string());
            }
            meta.last_timestamp = Some(ts.to_string());
        }

        // Extract session metadata from any message
        if meta.session_id.is_empty() {
            if let Some(sid) = value.get("sessionId").and_then(|s| s.as_str()) {
                meta.session_id = sid.to_string();
            }
        }
        if meta.project_dir.is_empty() {
            if let Some(cwd) = value.get("cwd").and_then(|s| s.as_str()) {
                meta.project_dir = cwd.to_string();
            }
        }
        if meta.git_branch.is_none() {
            if let Some(branch) = value.get("gitBranch").and_then(|s| s.as_str()) {
                meta.git_branch = Some(branch.to_string());
            }
        }

        match value.get("type").and_then(|t| t.as_str()) {
            Some("user") => extract_user_message(&value, &mut meta),
            Some("assistant") => {
                extract_assistant_message(&value, &mut meta, &mut seen_commands);
            }
            _ => {} // Skip progress, file-history-snapshot, system, etc.
        }
    }

    meta.compute_duration();
    Ok(meta)
}

/// Extract data from a user message.
fn extract_user_message(value: &Value, meta: &mut SessionMetadata) {
    let content = match value.get("message").and_then(|m| m.get("content")) {
        Some(c) => c,
        None => return,
    };

    // String content = actual user prompt
    if let Some(text) = content.as_str() {
        // Skip meta/system messages (commands, local-command-stdout, etc.)
        if !text.starts_with('<') && !text.is_empty() {
            meta.user_prompts.push(truncate(text, 2000));
        }
    }

    // Array content may contain tool_result entries (skip those)
    // but also user messages with text blocks
    if let Some(arr) = content.as_array() {
        for item in arr {
            // Skip tool results
            if item.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                continue;
            }
            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                if !text.starts_with('<') && !text.is_empty() {
                    meta.user_prompts.push(truncate(text, 2000));
                }
            }
        }
    }
}

/// Extract data from an assistant message.
fn extract_assistant_message(
    value: &Value,
    meta: &mut SessionMetadata,
    seen_commands: &mut std::collections::HashSet<String>,
) {
    let message = match value.get("message") {
        Some(m) => m,
        None => return,
    };

    // Extract model
    if meta.model.is_none() {
        if let Some(model) = message.get("model").and_then(|m| m.as_str()) {
            meta.model = Some(model.to_string());
        }
    }

    // Extract token usage
    if let Some(usage) = message.get("usage") {
        if let Some(input) = usage.get("input_tokens").and_then(|t| t.as_u64()) {
            meta.total_input_tokens += input;
        }
        if let Some(output) = usage.get("output_tokens").and_then(|t| t.as_u64()) {
            meta.total_output_tokens += output;
        }
        // Include cache tokens
        if let Some(cache_create) =
            usage.get("cache_creation_input_tokens").and_then(|t| t.as_u64())
        {
            meta.total_input_tokens += cache_create;
        }
        if let Some(cache_read) =
            usage.get("cache_read_input_tokens").and_then(|t| t.as_u64())
        {
            meta.total_input_tokens += cache_read;
        }
    }

    // Extract tool uses from content array
    let content = match message.get("content").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => return,
    };

    for item in content {
        if item.get("type").and_then(|t| t.as_str()) != Some("tool_use") {
            continue;
        }

        let tool_name = match item.get("name").and_then(|n| n.as_str()) {
            Some(n) => n,
            None => continue,
        };

        // Count tool usage
        *meta.tool_counts.entry(tool_name.to_string()).or_insert(0) += 1;

        let input = item.get("input");

        match tool_name {
            "Write" | "Edit" => {
                if let Some(path) = input.and_then(|i| i.get("file_path")).and_then(|p| p.as_str())
                {
                    meta.files_modified.insert(path.to_string());
                }
            }
            "Read" => {
                if let Some(path) = input.and_then(|i| i.get("file_path")).and_then(|p| p.as_str())
                {
                    meta.files_read.insert(path.to_string());
                }
            }
            "Bash" => {
                if let Some(cmd) = input.and_then(|i| i.get("command")).and_then(|c| c.as_str()) {
                    let truncated = truncate(cmd, MAX_COMMAND_LEN);
                    if meta.commands_run.len() < MAX_COMMANDS
                        && seen_commands.insert(truncated.clone())
                    {
                        meta.commands_run.push(truncated.clone());
                    }

                    // Extract git commits
                    if cmd.contains("git commit") {
                        if let Some(msg) = extract_commit_message(cmd) {
                            meta.git_commits.push(msg);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Try to extract a commit message from a git commit command.
fn extract_commit_message(cmd: &str) -> Option<String> {
    // Look for -m "..." or -m '...' patterns
    if let Some(idx) = cmd.find("-m ") {
        let rest = &cmd[idx + 3..];
        let rest = rest.trim();
        if rest.starts_with('"') || rest.starts_with('\'') {
            let quote = rest.chars().next()?;
            let end = rest[1..].find(quote)?;
            return Some(rest[1..=end].to_string());
        }
        // -m "$(cat <<'EOF' ... pattern — grab a reasonable chunk
        return Some(truncate(rest, 100));
    }
    None
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_fixture(lines: &[&str]) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        for line in lines {
            writeln!(file, "{}", line).unwrap();
        }
        file
    }

    #[test]
    fn test_parse_user_prompt() {
        let fixture = write_fixture(&[
            r#"{"type":"user","sessionId":"test-123","cwd":"/home/test","gitBranch":"main","message":{"role":"user","content":"Hello, let's fix the bug"},"timestamp":"2026-02-21T10:00:00Z"}"#,
        ]);

        let meta = parse_transcript(fixture.path()).unwrap();
        assert_eq!(meta.session_id, "test-123");
        assert_eq!(meta.project_dir, "/home/test");
        assert_eq!(meta.git_branch, Some("main".to_string()));
        assert_eq!(meta.user_prompts, vec!["Hello, let's fix the bug"]);
    }

    #[test]
    fn test_parse_tool_use() {
        let fixture = write_fixture(&[
            r#"{"type":"assistant","sessionId":"test-123","cwd":"/home/test","message":{"model":"claude-opus-4-6","role":"assistant","content":[{"type":"tool_use","name":"Write","input":{"file_path":"/home/test/foo.rs"}},{"type":"tool_use","name":"Bash","input":{"command":"cargo test"}}],"usage":{"input_tokens":100,"output_tokens":50}},"timestamp":"2026-02-21T10:01:00Z"}"#,
        ]);

        let meta = parse_transcript(fixture.path()).unwrap();
        assert_eq!(meta.model, Some("claude-opus-4-6".to_string()));
        assert!(meta.files_modified.contains("/home/test/foo.rs"));
        assert_eq!(meta.commands_run, vec!["cargo test"]);
        assert_eq!(meta.total_input_tokens, 100);
        assert_eq!(meta.total_output_tokens, 50);
        assert_eq!(meta.tool_counts.get("Write"), Some(&1));
        assert_eq!(meta.tool_counts.get("Bash"), Some(&1));
    }

    #[test]
    fn test_skip_meta_messages() {
        let fixture = write_fixture(&[
            r#"{"type":"user","sessionId":"test-123","cwd":"/home/test","message":{"role":"user","content":"<local-command-caveat>skip this</local-command-caveat>"},"timestamp":"2026-02-21T10:00:00Z"}"#,
        ]);

        let meta = parse_transcript(fixture.path()).unwrap();
        assert!(meta.user_prompts.is_empty());
    }

    #[test]
    fn test_corrupt_lines_skipped() {
        let fixture = write_fixture(&[
            "not valid json",
            r#"{"type":"user","sessionId":"test-123","cwd":"/home/test","message":{"role":"user","content":"valid message"},"timestamp":"2026-02-21T10:00:00Z"}"#,
        ]);

        let meta = parse_transcript(fixture.path()).unwrap();
        assert_eq!(meta.user_prompts, vec!["valid message"]);
    }

    #[test]
    fn test_git_commit_extraction() {
        let msg = extract_commit_message(r#"git commit -m "fix: resolve bug""#);
        assert_eq!(msg, Some("fix: resolve bug".to_string()));
    }

    #[test]
    fn test_duration_computation() {
        let fixture = write_fixture(&[
            r#"{"type":"user","sessionId":"test-123","cwd":"/home/test","message":{"role":"user","content":"start"},"timestamp":"2026-02-21T10:00:00Z"}"#,
            r#"{"type":"user","sessionId":"test-123","cwd":"/home/test","message":{"role":"user","content":"end"},"timestamp":"2026-02-21T10:30:00Z"}"#,
        ]);

        let meta = parse_transcript(fixture.path()).unwrap();
        assert_eq!(meta.duration_seconds, Some(1800));
    }
}
