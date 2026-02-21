/// Parser for the JSON transcript format produced by the
/// `claude-memory-vscode` GitHub Copilot Chat extension.
///
/// Expected shape:
/// ```json
/// {
///   "format": "copilot",
///   "session_id": "<uuid>",
///   "cwd": "/path/to/workspace",
///   "captured_at": "2026-02-21T10:00:00Z",
///   "model": "gpt-4o",
///   "turns": [
///     { "role": "user",      "content": "..." },
///     { "role": "assistant", "content": "..." }
///   ]
/// }
/// ```
use serde::Deserialize;

use super::metadata::SessionMetadata;

#[derive(Debug, Deserialize)]
struct CopilotTranscript {
    session_id: Option<String>,
    cwd: Option<String>,
    captured_at: Option<String>,
    model: Option<String>,
    turns: Vec<Turn>,
}

#[derive(Debug, Deserialize)]
struct Turn {
    role: String,
    content: String,
}

const MAX_PROMPT_LEN: usize = 2000;

pub fn parse_copilot_json(input: &str) -> anyhow::Result<SessionMetadata> {
    let transcript: CopilotTranscript = serde_json::from_str(input)?;

    let mut meta = SessionMetadata::default();

    meta.session_id = transcript
        .session_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    meta.project_dir = transcript.cwd.unwrap_or_default();
    meta.model = transcript.model;

    // Use captured_at as both first and last timestamp (single snapshot)
    if let Some(ts) = transcript.captured_at {
        meta.first_timestamp = Some(ts.clone());
        meta.last_timestamp = Some(ts);
    }

    for turn in &transcript.turns {
        if turn.role == "user" && !turn.content.trim().is_empty() {
            meta.user_prompts.push(truncate(&turn.content, MAX_PROMPT_LEN));
        }
    }

    Ok(meta)
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

    #[test]
    fn test_parse_copilot_json() {
        let json = r#"{
            "format": "copilot",
            "session_id": "abc-123",
            "cwd": "/home/chris/project",
            "captured_at": "2026-02-21T10:00:00Z",
            "model": "gpt-4o",
            "turns": [
                {"role": "user",      "content": "How do I fix the lifetime error?"},
                {"role": "assistant", "content": "You need to annotate the lifetime..."},
                {"role": "user",      "content": "Thanks, what about the borrow checker?"},
                {"role": "assistant", "content": "The borrow checker ensures..."}
            ]
        }"#;

        let meta = parse_copilot_json(json).unwrap();
        assert_eq!(meta.session_id, "abc-123");
        assert_eq!(meta.project_dir, "/home/chris/project");
        assert_eq!(meta.model, Some("gpt-4o".to_string()));
        assert_eq!(meta.user_prompts.len(), 2);
        assert_eq!(meta.user_prompts[0], "How do I fix the lifetime error?");
        assert_eq!(meta.user_prompts[1], "Thanks, what about the borrow checker?");
        assert_eq!(meta.first_timestamp, Some("2026-02-21T10:00:00Z".to_string()));
    }

    #[test]
    fn test_missing_session_id_generates_uuid() {
        let json = r#"{"turns": [{"role": "user", "content": "hello"}]}"#;
        let meta = parse_copilot_json(json).unwrap();
        assert!(!meta.session_id.is_empty());
    }

    #[test]
    fn test_empty_turns_produces_no_prompts() {
        let json = r#"{"session_id": "x", "turns": []}"#;
        let meta = parse_copilot_json(json).unwrap();
        assert!(meta.user_prompts.is_empty());
    }

    #[test]
    fn test_truncation() {
        let long_content = "a".repeat(3000);
        let json = format!(
            r#"{{"session_id": "x", "turns": [{{"role": "user", "content": "{}"}}]}}"#,
            long_content
        );
        let meta = parse_copilot_json(&json).unwrap();
        assert_eq!(meta.user_prompts[0].len(), MAX_PROMPT_LEN + 3); // +3 for "..."
    }
}
