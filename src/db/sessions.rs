use rusqlite::{params, Connection};
use serde::Serialize;

use crate::transcript::metadata::SessionMetadata;

#[derive(Debug, Serialize)]
pub struct SessionRow {
    pub id: String,
    pub project_dir: String,
    pub git_branch: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_seconds: Option<i64>,
    pub model: Option<String>,
    pub user_prompts: String,
    pub files_modified: String,
    pub files_read: String,
    pub commands_run: String,
    pub git_commits: String,
    pub tools_used: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub summary: Option<String>,
}

/// Check if a session has already been ingested.
pub fn session_exists(conn: &Connection, session_id: &str) -> anyhow::Result<bool> {
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM sessions WHERE id = ?",
        params![session_id],
        |row| row.get(0),
    )?;
    Ok(exists)
}

/// Insert a session from parsed metadata.
pub fn insert_session(conn: &Connection, meta: &SessionMetadata) -> anyhow::Result<()> {
    let user_prompts = serde_json::to_string(&meta.user_prompts)?;
    let files_modified: Vec<&String> = meta.files_modified.iter().collect();
    let files_modified_json = serde_json::to_string(&files_modified)?;
    let files_read: Vec<&String> = meta.files_read.iter().collect();
    let files_read_json = serde_json::to_string(&files_read)?;
    let commands_run = serde_json::to_string(&meta.commands_run)?;
    let git_commits = serde_json::to_string(&meta.git_commits)?;
    let tools_used = serde_json::to_string(&meta.tool_counts)?;

    conn.execute(
        "INSERT INTO sessions (id, project_dir, git_branch, started_at, ended_at,
         duration_seconds, model, user_prompts, files_modified, files_read,
         commands_run, git_commits, tools_used, input_tokens, output_tokens)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            meta.session_id,
            meta.project_dir,
            meta.git_branch,
            meta.first_timestamp.as_deref().unwrap_or("unknown"),
            meta.last_timestamp,
            meta.duration_seconds,
            meta.model,
            user_prompts,
            files_modified_json,
            files_read_json,
            commands_run,
            git_commits,
            tools_used,
            meta.total_input_tokens as i64,
            meta.total_output_tokens as i64,
        ],
    )?;

    Ok(())
}

/// Full-text search across sessions using FTS5.
///
/// Returns `(results, is_fallback)` where `is_fallback` is true if the results
/// came from an OR query after the original AND query returned nothing.
pub fn search_sessions(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> anyhow::Result<(Vec<SessionRow>, bool)> {
    let sanitized = super::sanitize_fts_query(query);

    let rows = fts_match(conn, &sanitized, limit)?;

    if !rows.is_empty() {
        return Ok((rows, false));
    }

    // AND returned nothing — try OR fallback for multi-word queries
    if let Some(or_query) = super::build_or_fallback(&sanitized) {
        let fallback_rows = fts_match(conn, &or_query, limit)?;
        if !fallback_rows.is_empty() {
            return Ok((fallback_rows, true));
        }
    }

    Ok((Vec::new(), false))
}

/// Execute an FTS5 MATCH query against sessions_fts.
fn fts_match(
    conn: &Connection,
    match_expr: &str,
    limit: usize,
) -> anyhow::Result<Vec<SessionRow>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.project_dir, s.git_branch, s.started_at, s.ended_at,
                s.duration_seconds, s.model, s.user_prompts, s.files_modified,
                s.files_read, s.commands_run, s.git_commits, s.tools_used,
                s.input_tokens, s.output_tokens, s.summary
         FROM sessions_fts
         JOIN sessions s ON sessions_fts.rowid = s.rowid
         WHERE sessions_fts MATCH ?
         ORDER BY rank
         LIMIT ?",
    )?;

    let rows = stmt
        .query_map(params![match_expr, limit as i64], |row| {
            Ok(SessionRow {
                id: row.get(0)?,
                project_dir: row.get(1)?,
                git_branch: row.get(2)?,
                started_at: row.get(3)?,
                ended_at: row.get(4)?,
                duration_seconds: row.get(5)?,
                model: row.get(6)?,
                user_prompts: row.get(7)?,
                files_modified: row.get(8)?,
                files_read: row.get(9)?,
                commands_run: row.get(10)?,
                git_commits: row.get(11)?,
                tools_used: row.get(12)?,
                input_tokens: row.get(13)?,
                output_tokens: row.get(14)?,
                summary: row.get(15)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// List sessions ordered by date, optionally filtered.
pub fn list_sessions(
    conn: &Connection,
    limit: usize,
    date_from: Option<&str>,
    date_to: Option<&str>,
) -> anyhow::Result<Vec<SessionRow>> {
    let mut sql = String::from(
        "SELECT id, project_dir, git_branch, started_at, ended_at,
                duration_seconds, model, user_prompts, files_modified,
                files_read, commands_run, git_commits, tools_used,
                input_tokens, output_tokens, summary
         FROM sessions WHERE 1=1",
    );

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(from) = date_from {
        sql.push_str(" AND started_at >= ?");
        param_values.push(Box::new(from.to_string()));
    }
    if let Some(to) = date_to {
        sql.push_str(" AND started_at <= ?");
        param_values.push(Box::new(to.to_string()));
    }

    sql.push_str(" ORDER BY started_at DESC LIMIT ?");
    param_values.push(Box::new(limit as i64));

    let params: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params.as_slice(), |row| {
            Ok(SessionRow {
                id: row.get(0)?,
                project_dir: row.get(1)?,
                git_branch: row.get(2)?,
                started_at: row.get(3)?,
                ended_at: row.get(4)?,
                duration_seconds: row.get(5)?,
                model: row.get(6)?,
                user_prompts: row.get(7)?,
                files_modified: row.get(8)?,
                files_read: row.get(9)?,
                commands_run: row.get(10)?,
                git_commits: row.get(11)?,
                tools_used: row.get(12)?,
                input_tokens: row.get(13)?,
                output_tokens: row.get(14)?,
                summary: row.get(15)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// Get a single session by ID.
pub fn get_session(conn: &Connection, session_id: &str) -> anyhow::Result<Option<SessionRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_dir, git_branch, started_at, ended_at,
                duration_seconds, model, user_prompts, files_modified,
                files_read, commands_run, git_commits, tools_used,
                input_tokens, output_tokens, summary
         FROM sessions WHERE id = ?",
    )?;

    let mut rows = stmt.query_map(params![session_id], |row| {
        Ok(SessionRow {
            id: row.get(0)?,
            project_dir: row.get(1)?,
            git_branch: row.get(2)?,
            started_at: row.get(3)?,
            ended_at: row.get(4)?,
            duration_seconds: row.get(5)?,
            model: row.get(6)?,
            user_prompts: row.get(7)?,
            files_modified: row.get(8)?,
            files_read: row.get(9)?,
            commands_run: row.get(10)?,
            git_commits: row.get(11)?,
            tools_used: row.get(12)?,
            input_tokens: row.get(13)?,
            output_tokens: row.get(14)?,
            summary: row.get(15)?,
        })
    })?;

    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Lightweight project-level summary for cross-project listing.
#[allow(dead_code)]
pub struct ProjectSummary {
    pub session_count: i64,
    pub note_count: i64,
    pub first_session: Option<String>,
    pub last_session: Option<String>,
    pub last_branch: Option<String>,
}

/// Get a lightweight summary from a database connection.
/// Designed for cross-project discovery — runs fast on any memory.db.
pub fn project_summary(conn: &Connection) -> anyhow::Result<ProjectSummary> {
    let session_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?;

    let note_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))?;

    let (first_session, last_session) = conn
        .query_row(
            "SELECT MIN(started_at), MAX(started_at) FROM sessions",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((None, None));

    let last_branch: Option<String> = conn
        .query_row(
            "SELECT git_branch FROM sessions ORDER BY started_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(None);

    Ok(ProjectSummary {
        session_count,
        note_count,
        first_session,
        last_session,
        last_branch,
    })
}

/// Get total session count and DB stats.
pub fn session_stats(conn: &Connection) -> anyhow::Result<(i64, i64, i64)> {
    let count: i64 =
        conn.query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?;
    let total_input: i64 = conn.query_row(
        "SELECT COALESCE(SUM(input_tokens), 0) FROM sessions",
        [],
        |row| row.get(0),
    )?;
    let total_output: i64 = conn.query_row(
        "SELECT COALESCE(SUM(output_tokens), 0) FROM sessions",
        [],
        |row| row.get(0),
    )?;
    Ok((count, total_input, total_output))
}
