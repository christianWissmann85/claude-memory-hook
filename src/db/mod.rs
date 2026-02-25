pub mod notes;
pub mod schema;
pub mod sessions;

use std::path::Path;

use rusqlite::Connection;

/// Sanitize a user query for safe FTS5 MATCH usage.
///
/// Replaces special characters (hyphens, colons, parens, etc.) with spaces
/// to prevent FTS5 from misinterpreting them as column filters or operators.
/// Preserves quoted phrases and FTS5 keywords (AND, OR, NOT, NEAR).
pub fn sanitize_fts_query(query: &str) -> String {
    let mut result = String::new();
    let mut in_quotes = false;

    for c in query.chars() {
        if c == '"' {
            in_quotes = !in_quotes;
            result.push(c);
        } else if in_quotes || c.is_alphanumeric() || c.is_whitespace() || c == '_' {
            result.push(c);
        } else {
            // Replace special characters with space to avoid FTS5 syntax errors
            result.push(' ');
        }
    }

    // Collapse multiple spaces
    result.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// Check whether a sanitized FTS5 query contains explicit boolean operators.
fn has_explicit_operators(query: &str) -> bool {
    query
        .split_whitespace()
        .any(|word| matches!(word, "AND" | "OR" | "NOT" | "NEAR"))
}

/// Build an OR-joined version of a multi-word query for fallback search.
/// Returns `None` if the query has fewer than 2 terms or already uses explicit operators.
pub fn build_or_fallback(sanitized_query: &str) -> Option<String> {
    if has_explicit_operators(sanitized_query) {
        return None;
    }

    let terms: Vec<&str> = sanitized_query.split_whitespace().collect();
    if terms.len() < 2 {
        return None;
    }

    Some(terms.join(" OR "))
}

/// Open an existing memory database in read-only mode.
/// Does not create directories or run migrations.
/// Used for cross-project discovery.
pub fn open_readonly(db_path: &Path) -> anyhow::Result<Connection> {
    let conn = Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    Ok(conn)
}

/// Open (or create) the memory database at the given path.
/// Enables WAL mode and creates schema if needed.
pub fn open(db_path: &Path) -> anyhow::Result<Connection> {
    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(db_path)?;

    // journal_mode returns a result row
    let mut stmt = conn.prepare("PRAGMA journal_mode=WAL")?;
    let _ = stmt.query_row([], |row| row.get::<_, String>(0));
    drop(stmt);

    // foreign_keys is a simple flag
    let mut stmt = conn.prepare("PRAGMA foreign_keys=ON")?;
    let _ = stmt.raw_execute();
    drop(stmt);

    schema::ensure_schema(&conn)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_hyphens() {
        assert_eq!(sanitize_fts_query("claude-memory install"), "claude memory install");
    }

    #[test]
    fn sanitize_strips_colons() {
        assert_eq!(sanitize_fts_query("column:term"), "column term");
    }

    #[test]
    fn sanitize_preserves_quoted_phrases() {
        assert_eq!(
            sanitize_fts_query("\"exact phrase\" AND something"),
            "\"exact phrase\" AND something"
        );
    }

    #[test]
    fn sanitize_preserves_fts5_operators() {
        assert_eq!(sanitize_fts_query("foo OR bar NOT baz"), "foo OR bar NOT baz");
    }

    #[test]
    fn sanitize_handles_parens_and_stars() {
        assert_eq!(sanitize_fts_query("(foo*) AND bar"), "foo AND bar");
    }

    #[test]
    fn sanitize_collapses_whitespace() {
        assert_eq!(sanitize_fts_query("foo  -  bar"), "foo bar");
    }

    #[test]
    fn sanitize_preserves_underscores() {
        assert_eq!(sanitize_fts_query("normal_depth"), "normal_depth");
    }

    #[test]
    fn sanitize_plain_query_unchanged() {
        assert_eq!(sanitize_fts_query("install MCP server"), "install MCP server");
    }

    #[test]
    fn or_fallback_multi_word() {
        assert_eq!(
            build_or_fallback("install MCP server"),
            Some("install OR MCP OR server".to_string())
        );
    }

    #[test]
    fn or_fallback_single_word_returns_none() {
        assert_eq!(build_or_fallback("install"), None);
    }

    #[test]
    fn or_fallback_with_explicit_and_returns_none() {
        assert_eq!(build_or_fallback("install AND server"), None);
    }

    #[test]
    fn or_fallback_with_explicit_or_returns_none() {
        assert_eq!(build_or_fallback("install OR server"), None);
    }

    #[test]
    fn or_fallback_with_not_returns_none() {
        assert_eq!(build_or_fallback("install NOT server"), None);
    }

    #[test]
    fn schema_version_starts_at_one() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("memory.db");
        let conn = open(&db_path).unwrap();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn migration_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("memory.db");
        // First open creates schema
        let conn = open(&db_path).unwrap();
        drop(conn);
        // Second open should not fail (migration already applied)
        let conn = open(&db_path).unwrap();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn fts5_uses_porter_stemming() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("memory.db");
        let conn = open(&db_path).unwrap();

        // Insert a session with "layers" in user_prompts
        conn.execute(
            "INSERT INTO sessions (id, project_dir, started_at, user_prompts)
             VALUES ('s1', '/test', '2025-01-01', '[\"added multiple layers\"]')",
            [],
        ).unwrap();

        // Search for "layer" (singular) — porter stemming should match "layers"
        let (results, is_fallback) = sessions::search_sessions(&conn, "layer", 5).unwrap();
        assert!(!is_fallback);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "s1");
    }

    #[test]
    fn or_fallback_triggers_when_and_fails() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("memory.db");
        let conn = open(&db_path).unwrap();

        // Session mentions "authentication" but not "database"
        conn.execute(
            "INSERT INTO sessions (id, project_dir, started_at, user_prompts)
             VALUES ('s1', '/test', '2025-01-01', '[\"implemented authentication flow\"]')",
            [],
        ).unwrap();

        // "authentication database" with AND → no match, fallback to OR → matches s1
        let (results, is_fallback) = sessions::search_sessions(&conn, "authentication database", 5).unwrap();
        assert!(is_fallback);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "s1");
    }

    #[test]
    fn and_match_preferred_over_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("memory.db");
        let conn = open(&db_path).unwrap();

        // Session that matches both terms
        conn.execute(
            "INSERT INTO sessions (id, project_dir, started_at, user_prompts)
             VALUES ('s1', '/test', '2025-01-01', '[\"fix authentication bug in database layer\"]')",
            [],
        ).unwrap();

        // AND should succeed — no fallback needed
        let (results, is_fallback) = sessions::search_sessions(&conn, "authentication database", 5).unwrap();
        assert!(!is_fallback);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn files_read_is_searchable() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("memory.db");
        let conn = open(&db_path).unwrap();

        conn.execute(
            "INSERT INTO sessions (id, project_dir, started_at, user_prompts, files_read)
             VALUES ('s1', '/test', '2025-01-01', '[]', '[\"src/config.rs\", \"src/main.rs\"]')",
            [],
        ).unwrap();

        let (results, _) = sessions::search_sessions(&conn, "config", 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "s1");
    }
}
