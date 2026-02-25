use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct NoteRow {
    pub id: String,
    pub session_id: Option<String>,
    pub content: String,
    pub tags: String,
    pub created_at: String,
}

/// Insert a new note.
pub fn insert_note(
    conn: &Connection,
    content: &str,
    tags: &[String],
    session_id: Option<&str>,
) -> anyhow::Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let tags_json = serde_json::to_string(tags)?;

    conn.execute(
        "INSERT INTO notes (id, session_id, content, tags) VALUES (?, ?, ?, ?)",
        params![id, session_id, content, tags_json],
    )?;

    Ok(id)
}

/// Full-text search notes.
pub fn search_notes(
    conn: &Connection,
    query: Option<&str>,
    tag: Option<&str>,
    limit: usize,
) -> anyhow::Result<Vec<NoteRow>> {
    // If we have an FTS query, use the FTS5 table
    if let Some(q) = query {
        let sanitized = super::sanitize_fts_query(q);

        let mut stmt = conn.prepare(
            "SELECT n.id, n.session_id, n.content, n.tags, n.created_at
             FROM notes_fts
             JOIN notes n ON notes_fts.rowid = n.rowid
             WHERE notes_fts MATCH ?
             ORDER BY rank
             LIMIT ?",
        )?;

        let rows = stmt
            .query_map(params![sanitized, limit as i64], |row| {
                Ok(NoteRow {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    content: row.get(2)?,
                    tags: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        return Ok(rows);
    }

    // If we only have a tag filter, use LIKE on the tags JSON array
    if let Some(t) = tag {
        let pattern = format!("%\"{}%", t);
        let mut stmt = conn.prepare(
            "SELECT id, session_id, content, tags, created_at
             FROM notes WHERE tags LIKE ?
             ORDER BY created_at DESC LIMIT ?",
        )?;

        let rows = stmt
            .query_map(params![pattern, limit as i64], |row| {
                Ok(NoteRow {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    content: row.get(2)?,
                    tags: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        return Ok(rows);
    }

    // No filter — return recent notes
    let mut stmt = conn.prepare(
        "SELECT id, session_id, content, tags, created_at
         FROM notes ORDER BY created_at DESC LIMIT ?",
    )?;

    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok(NoteRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                content: row.get(2)?,
                tags: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// Get note count.
pub fn note_count(conn: &Connection) -> anyhow::Result<i64> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))?;
    Ok(count)
}
