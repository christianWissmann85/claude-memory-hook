use std::io::Read;
use std::path::PathBuf;

use serde::Deserialize;

use crate::config;
use crate::db;
use crate::transcript::parser;

/// Hook input from Claude Code's SessionEnd event.
#[derive(Debug, Deserialize)]
struct HookInput {
    session_id: Option<String>,
    transcript_path: Option<String>,
    cwd: Option<String>,
    #[allow(dead_code)]
    hook_event_name: Option<String>,
}

pub fn run() -> anyhow::Result<()> {
    // Read hook JSON from stdin
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    let hook: HookInput = serde_json::from_str(&input)?;

    let transcript_path = match &hook.transcript_path {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("claude-memory: no transcript_path in hook input, skipping");
            return Ok(());
        }
    };

    if !transcript_path.exists() {
        eprintln!(
            "claude-memory: transcript not found: {}, skipping",
            transcript_path.display()
        );
        return Ok(());
    }

    // Determine project directory
    let project_dir = match &hook.cwd {
        Some(cwd) => config::find_project_root(&PathBuf::from(cwd)),
        None => config::detect_project_dir()?,
    };

    let db_path = config::db_path(&project_dir);
    let conn = db::open(&db_path)?;

    // Check idempotency
    if let Some(ref sid) = hook.session_id {
        if db::sessions::session_exists(&conn, sid)? {
            return Ok(()); // Already ingested
        }
    }

    // Parse transcript
    let mut meta = parser::parse_transcript(&transcript_path)?;

    // Use hook session_id if transcript didn't have one
    if meta.session_id.is_empty() {
        if let Some(sid) = hook.session_id {
            meta.session_id = sid;
        } else {
            meta.session_id = uuid::Uuid::new_v4().to_string();
        }
    }

    // Use hook cwd if transcript didn't have one
    if meta.project_dir.is_empty() {
        meta.project_dir = project_dir.to_string_lossy().to_string();
    }

    // Skip empty sessions (no user prompts at all)
    if meta.user_prompts.is_empty() {
        return Ok(());
    }

    // Store in database
    db::sessions::insert_session(&conn, &meta)?;

    eprintln!(
        "claude-memory: ingested session {} ({} prompts, {} files modified)",
        &meta.session_id[..8.min(meta.session_id.len())],
        meta.user_prompts.len(),
        meta.files_modified.len()
    );

    Ok(())
}
