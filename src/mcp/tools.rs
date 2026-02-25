use rusqlite::Connection;
use serde_json::{json, Value};

use crate::db::{notes, sessions};

/// Return all tool definitions for MCP tools/list.
pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "recall",
            "description": "Search past session memory for the current project. Returns matching sessions with context about what was discussed, files modified, and commands run. Use this to remember past work, find previous decisions, or recall how something was implemented.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query (supports FTS5 syntax: AND, OR, NOT, \"exact phrase\")"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum results (default: 5, max: 20)"
                    }
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "list_sessions",
            "description": "List recent sessions for the current project. Shows date, duration, branch, and key files modified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Max sessions to return (default: 10)"
                    },
                    "date_from": {
                        "type": "string",
                        "description": "Filter sessions after this date (ISO format, e.g. 2026-02-01)"
                    },
                    "date_to": {
                        "type": "string",
                        "description": "Filter sessions before this date (ISO format, e.g. 2026-02-21)"
                    }
                }
            }
        }),
        json!({
            "name": "get_session",
            "description": "Get full details of a past session including all user prompts, files modified/read, commands run, and git commits.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session ID (from recall or list_sessions results)"
                    }
                },
                "required": ["session_id"]
            }
        }),
        json!({
            "name": "log_note",
            "description": "Log a note for the current project. Use this to record decisions, rationale, architectural choices, or anything worth remembering across sessions.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "Note content (the decision, rationale, or information to remember)"
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Tags for categorization (e.g. [\"decision\", \"architecture\", \"bug\"])"
                    }
                },
                "required": ["content"]
            }
        }),
        json!({
            "name": "search_notes",
            "description": "Search notes by content or tag. Returns matching notes with timestamps and context.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "FTS5 search query for note content"
                    },
                    "tag": {
                        "type": "string",
                        "description": "Filter notes by tag"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max results (default: 10)"
                    }
                }
            }
        }),
        json!({
            "name": "list_projects",
            "description": "List all projects on this machine that have claude-memory databases. Shows session counts, date ranges, and recent branches for each project. Use this to discover past work across projects.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Maximum projects to return (default: 20)"
                    }
                }
            }
        }),
    ]
}

/// Dispatch a tool call to the appropriate handler.
pub fn dispatch(name: &str, args: &Value, conn: &Connection) -> anyhow::Result<String> {
    match name {
        "recall" => handle_recall(args, conn),
        "list_sessions" => handle_list_sessions(args, conn),
        "get_session" => handle_get_session(args, conn),
        "log_note" => handle_log_note(args, conn),
        "search_notes" => handle_search_notes(args, conn),
        "list_projects" => handle_list_projects(args),
        _ => Ok(format!("Unknown tool: {}", name)),
    }
}

fn handle_recall(args: &Value, conn: &Connection) -> anyhow::Result<String> {
    let query = args
        .get("query")
        .and_then(|q| q.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: query"))?;

    let limit = args
        .get("limit")
        .and_then(|l| l.as_u64())
        .unwrap_or(5)
        .min(20) as usize;

    let (results, is_fallback) = sessions::search_sessions(conn, query, limit)?;

    if results.is_empty() {
        return Ok(format!("No sessions found matching: \"{}\"", query));
    }

    let mut output = if is_fallback {
        format!(
            "# Found {} session(s) with partial matches for: \"{}\"\n\
             _(No exact match — showing sessions matching some of these terms)_\n\n",
            results.len(),
            query
        )
    } else {
        format!(
            "# Found {} session(s) matching: \"{}\"\n\n",
            results.len(),
            query
        )
    };

    for session in &results {
        output.push_str(&format_session_summary(session));
        output.push('\n');
    }

    Ok(output)
}

fn handle_list_sessions(args: &Value, conn: &Connection) -> anyhow::Result<String> {
    let limit = args
        .get("limit")
        .and_then(|l| l.as_u64())
        .unwrap_or(10)
        .min(50) as usize;

    let date_from = args.get("date_from").and_then(|d| d.as_str());
    let date_to = args.get("date_to").and_then(|d| d.as_str());

    let results = sessions::list_sessions(conn, limit, date_from, date_to)?;

    if results.is_empty() {
        return Ok("No sessions found.".to_string());
    }

    let mut output = format!("# {} Recent Session(s)\n\n", results.len());

    for session in &results {
        output.push_str(&format_session_summary(session));
        output.push('\n');
    }

    Ok(output)
}

fn handle_get_session(args: &Value, conn: &Connection) -> anyhow::Result<String> {
    let session_id = args
        .get("session_id")
        .and_then(|s| s.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: session_id"))?;

    let session = sessions::get_session(conn, session_id)?;

    match session {
        Some(s) => Ok(format_session_detail(&s)),
        None => Ok(format!("Session not found: {}", session_id)),
    }
}

fn handle_log_note(args: &Value, conn: &Connection) -> anyhow::Result<String> {
    let content = args
        .get("content")
        .and_then(|c| c.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: content"))?;

    let tags: Vec<String> = args
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let id = notes::insert_note(conn, content, &tags, None)?;

    let tag_display = if tags.is_empty() {
        String::new()
    } else {
        format!(" [{}]", tags.join(", "))
    };

    Ok(format!("Note saved{} (id: {})", tag_display, &id[..8]))
}

fn handle_search_notes(args: &Value, conn: &Connection) -> anyhow::Result<String> {
    let query = args.get("query").and_then(|q| q.as_str());
    let tag = args.get("tag").and_then(|t| t.as_str());
    let limit = args
        .get("limit")
        .and_then(|l| l.as_u64())
        .unwrap_or(10) as usize;

    let results = notes::search_notes(conn, query, tag, limit)?;

    if results.is_empty() {
        return Ok("No notes found.".to_string());
    }

    let mut output = format!("# {} Note(s)\n\n", results.len());

    for note in &results {
        let date = &note.created_at[..10.min(note.created_at.len())];
        let tags: Vec<String> = serde_json::from_str(&note.tags).unwrap_or_default();
        let tag_display = if tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", tags.join(", "))
        };

        output.push_str(&format!("## {}{}\n", date, tag_display));
        output.push_str(&note.content);
        output.push_str("\n\n");
    }

    Ok(output)
}

fn handle_list_projects(args: &Value) -> anyhow::Result<String> {
    let limit = args
        .get("limit")
        .and_then(|l| l.as_u64())
        .unwrap_or(20)
        .min(50) as usize;

    let current_project = crate::config::detect_project_dir().ok();
    let projects = crate::config::discover_project_dbs();

    if projects.is_empty() {
        return Ok("No projects with memory databases found.".to_string());
    }

    let mut entries: Vec<ProjectEntry> = Vec::new();

    for project in &projects {
        let conn = match crate::db::open_readonly(&project.db_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let summary = match sessions::project_summary(&conn) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let is_current = current_project
            .as_ref()
            .is_some_and(|cp| cp == &project.project_dir);

        let name = project
            .project_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| project.project_dir.display().to_string());

        entries.push(ProjectEntry {
            name,
            is_current,
            summary,
        });
    }

    // Current project first, then by last session date descending
    entries.sort_by(|a, b| {
        b.is_current.cmp(&a.is_current).then_with(|| {
            let a_date = a.summary.last_session.as_deref().unwrap_or("");
            let b_date = b.summary.last_session.as_deref().unwrap_or("");
            b_date.cmp(a_date)
        })
    });

    entries.truncate(limit);

    let mut output = format!("# {} Project(s) with Memory\n\n", entries.len());
    output.push_str("| Project | Sessions | Notes | Last Active | Branch |\n");
    output.push_str("|---------|----------|-------|-------------|--------|\n");

    for entry in &entries {
        let marker = if entry.is_current { " **(current)**" } else { "" };
        let last_active = entry
            .summary
            .last_session
            .as_ref()
            .map(|d| &d[..10.min(d.len())])
            .unwrap_or("-");
        let branch = entry.summary.last_branch.as_deref().unwrap_or("-");

        output.push_str(&format!(
            "| {}{} | {} | {} | {} | {} |\n",
            entry.name, marker, entry.summary.session_count, entry.summary.note_count, last_active, branch,
        ));
    }

    let total_sessions: i64 = entries.iter().map(|e| e.summary.session_count).sum();
    let total_notes: i64 = entries.iter().map(|e| e.summary.note_count).sum();
    output.push_str(&format!(
        "\n_Total: {} sessions, {} notes across {} projects_\n",
        total_sessions, total_notes, entries.len()
    ));

    Ok(output)
}

struct ProjectEntry {
    name: String,
    is_current: bool,
    summary: sessions::ProjectSummary,
}

// --- Formatting helpers ---

fn format_session_summary(session: &sessions::SessionRow) -> String {
    let date = &session.started_at[..10.min(session.started_at.len())];
    let duration = session
        .duration_seconds
        .map(format_duration)
        .unwrap_or_else(|| "?".to_string());
    let branch = session.git_branch.as_deref().unwrap_or("?");

    let mut out = format!("## {} | {} | branch: {}\n", date, duration, branch);
    out.push_str(&format!("**Session:** `{}`\n", session.id));

    if let Some(model) = &session.model {
        out.push_str(&format!("**Model:** {}\n", model));
    }

    // User prompts (show first 2, truncated)
    if let Ok(prompts) = serde_json::from_str::<Vec<String>>(&session.user_prompts) {
        if !prompts.is_empty() {
            out.push_str("**Prompts:**\n");
            for prompt in prompts.iter().take(2) {
                let display = if prompt.len() > 150 {
                    format!("{}...", &prompt[..150])
                } else {
                    prompt.clone()
                };
                out.push_str(&format!("- {}\n", display));
            }
            if prompts.len() > 2 {
                out.push_str(&format!("- _(+{} more)_\n", prompts.len() - 2));
            }
        }
    }

    // Files modified (show filenames only)
    if let Ok(files) = serde_json::from_str::<Vec<String>>(&session.files_modified) {
        if !files.is_empty() {
            let names: Vec<&str> = files
                .iter()
                .map(|f| f.rsplit('/').next().unwrap_or(f))
                .take(8)
                .collect();
            out.push_str(&format!("**Files modified:** {}", names.join(", ")));
            if files.len() > 8 {
                out.push_str(&format!(" (+{})", files.len() - 8));
            }
            out.push('\n');
        }
    }

    // Git commits
    if let Ok(commits) = serde_json::from_str::<Vec<String>>(&session.git_commits) {
        if !commits.is_empty() {
            out.push_str("**Commits:**\n");
            for commit in &commits {
                out.push_str(&format!("- {}\n", commit));
            }
        }
    }

    out
}

fn format_session_detail(session: &sessions::SessionRow) -> String {
    let mut out = format_session_summary(session);

    // Full file lists
    if let Ok(files) = serde_json::from_str::<Vec<String>>(&session.files_read) {
        if !files.is_empty() {
            out.push_str(&format!("\n**Files read ({}):**\n", files.len()));
            for f in files.iter().take(20) {
                out.push_str(&format!("- {}\n", f));
            }
        }
    }

    if let Ok(cmds) = serde_json::from_str::<Vec<String>>(&session.commands_run) {
        if !cmds.is_empty() {
            out.push_str(&format!("\n**Commands ({}):**\n", cmds.len()));
            for cmd in &cmds {
                out.push_str(&format!("- `{}`\n", cmd));
            }
        }
    }

    // Tool usage
    if let Ok(tools) =
        serde_json::from_str::<std::collections::HashMap<String, u32>>(&session.tools_used)
    {
        if !tools.is_empty() {
            let mut sorted: Vec<_> = tools.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));
            let display: Vec<String> = sorted.iter().map(|(k, v)| format!("{}:{}", k, v)).collect();
            out.push_str(&format!("\n**Tool usage:** {}\n", display.join(", ")));
        }
    }

    out.push_str(&format!(
        "\n**Tokens:** {} input / {} output\n",
        session.input_tokens, session.output_tokens
    ));

    out
}

fn format_duration(seconds: i64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    }
}
