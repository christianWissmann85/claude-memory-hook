use rusqlite::Connection;

/// Current schema version. Bump this and add a migration function when changing the schema.
const CURRENT_VERSION: i64 = 1;

/// Create all tables, FTS5 indexes, and triggers if they don't exist.
/// Runs migrations if the schema is outdated.
pub fn ensure_schema(conn: &Connection) -> anyhow::Result<()> {
    // Core tables (idempotent)
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            project_dir TEXT NOT NULL,
            git_branch TEXT,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            duration_seconds INTEGER,
            model TEXT,
            user_prompts TEXT NOT NULL DEFAULT '[]',
            files_modified TEXT NOT NULL DEFAULT '[]',
            files_read TEXT NOT NULL DEFAULT '[]',
            commands_run TEXT NOT NULL DEFAULT '[]',
            git_commits TEXT NOT NULL DEFAULT '[]',
            tools_used TEXT NOT NULL DEFAULT '{}',
            input_tokens INTEGER DEFAULT 0,
            output_tokens INTEGER DEFAULT 0,
            summary TEXT,
            ingested_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS notes (
            id TEXT PRIMARY KEY,
            session_id TEXT,
            content TEXT NOT NULL,
            tags TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (session_id) REFERENCES sessions(id)
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_started_at ON sessions(started_at);
        CREATE INDEX IF NOT EXISTS idx_sessions_project_dir ON sessions(project_dir);
        CREATE INDEX IF NOT EXISTS idx_notes_session_id ON notes(session_id);
        CREATE INDEX IF NOT EXISTS idx_notes_created_at ON notes(created_at);
        ",
    )?;

    let version = get_schema_version(conn)?;

    if version < CURRENT_VERSION {
        run_migrations(conn, version)?;
    }

    Ok(())
}

/// Get the current schema version (0 if table is empty or freshly created).
fn get_schema_version(conn: &Connection) -> anyhow::Result<i64> {
    let version: Option<i64> = conn
        .query_row(
            "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    Ok(version.unwrap_or(0))
}

/// Set the schema version.
fn set_schema_version(conn: &Connection, version: i64) -> anyhow::Result<()> {
    conn.execute("DELETE FROM schema_version", [])?;
    conn.execute("INSERT INTO schema_version (version) VALUES (?)", [version])?;
    Ok(())
}

/// Run all pending migrations from `from_version` to `CURRENT_VERSION`.
fn run_migrations(conn: &Connection, from_version: i64) -> anyhow::Result<()> {
    if from_version < 1 {
        migrate_v0_to_v1(conn)?;
    }

    set_schema_version(conn, CURRENT_VERSION)?;
    Ok(())
}

/// Migration v0 → v1:
/// - Drop old FTS5 tables and triggers (no porter stemming, missing files_read)
/// - Recreate with `tokenize='porter unicode61'` and `files_read` column
/// - Rebuild index from existing data
fn migrate_v0_to_v1(conn: &Connection) -> anyhow::Result<()> {
    // Drop old sessions FTS infrastructure
    conn.execute_batch(
        "
        DROP TRIGGER IF EXISTS sessions_ai;
        DROP TRIGGER IF EXISTS sessions_ad;
        DROP TRIGGER IF EXISTS sessions_au;
        DROP TABLE IF EXISTS sessions_fts;
        ",
    )?;

    // Drop old notes FTS infrastructure
    conn.execute_batch(
        "
        DROP TRIGGER IF EXISTS notes_ai;
        DROP TRIGGER IF EXISTS notes_ad;
        DROP TRIGGER IF EXISTS notes_au;
        DROP TABLE IF EXISTS notes_fts;
        ",
    )?;

    // Recreate sessions FTS with porter stemming + files_read
    conn.execute_batch(
        "
        CREATE VIRTUAL TABLE sessions_fts USING fts5(
            user_prompts, files_modified, files_read, commands_run, git_commits, summary,
            content=sessions, content_rowid=rowid,
            tokenize='porter unicode61'
        );

        CREATE TRIGGER sessions_ai AFTER INSERT ON sessions BEGIN
            INSERT INTO sessions_fts(rowid, user_prompts, files_modified, files_read, commands_run, git_commits, summary)
            VALUES (new.rowid, new.user_prompts, new.files_modified, new.files_read, new.commands_run, new.git_commits, new.summary);
        END;

        CREATE TRIGGER sessions_ad AFTER DELETE ON sessions BEGIN
            INSERT INTO sessions_fts(sessions_fts, rowid, user_prompts, files_modified, files_read, commands_run, git_commits, summary)
            VALUES ('delete', old.rowid, old.user_prompts, old.files_modified, old.files_read, old.commands_run, old.git_commits, old.summary);
        END;

        CREATE TRIGGER sessions_au AFTER UPDATE ON sessions BEGIN
            INSERT INTO sessions_fts(sessions_fts, rowid, user_prompts, files_modified, files_read, commands_run, git_commits, summary)
            VALUES ('delete', old.rowid, old.user_prompts, old.files_modified, old.files_read, old.commands_run, old.git_commits, old.summary);
            INSERT INTO sessions_fts(rowid, user_prompts, files_modified, files_read, commands_run, git_commits, summary)
            VALUES (new.rowid, new.user_prompts, new.files_modified, new.files_read, new.commands_run, new.git_commits, new.summary);
        END;
        ",
    )?;

    // Recreate notes FTS with porter stemming
    conn.execute_batch(
        "
        CREATE VIRTUAL TABLE notes_fts USING fts5(
            content, tags,
            content=notes, content_rowid=rowid,
            tokenize='porter unicode61'
        );

        CREATE TRIGGER notes_ai AFTER INSERT ON notes BEGIN
            INSERT INTO notes_fts(rowid, content, tags)
            VALUES (new.rowid, new.content, new.tags);
        END;

        CREATE TRIGGER notes_ad AFTER DELETE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, content, tags)
            VALUES ('delete', old.rowid, old.content, old.tags);
        END;

        CREATE TRIGGER notes_au AFTER UPDATE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, content, tags)
            VALUES ('delete', old.rowid, old.content, old.tags);
            INSERT INTO notes_fts(rowid, content, tags)
            VALUES (new.rowid, new.content, new.tags);
        END;
        ",
    )?;

    // Rebuild FTS indexes from existing data
    conn.execute_batch(
        "
        INSERT INTO sessions_fts(sessions_fts) VALUES('rebuild');
        INSERT INTO notes_fts(notes_fts) VALUES('rebuild');
        ",
    )?;

    Ok(())
}
