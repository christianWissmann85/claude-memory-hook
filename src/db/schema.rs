use rusqlite::Connection;

/// Create all tables, FTS5 indexes, and triggers if they don't exist.
pub fn ensure_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "
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

    // FTS5 tables (CREATE VIRTUAL TABLE doesn't support IF NOT EXISTS in all versions,
    // so we check manually)
    let has_sessions_fts: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='sessions_fts'",
        [],
        |row| row.get(0),
    )?;

    if !has_sessions_fts {
        conn.execute_batch(
            "
            CREATE VIRTUAL TABLE sessions_fts USING fts5(
                user_prompts, files_modified, commands_run, git_commits, summary,
                content=sessions, content_rowid=rowid
            );

            CREATE TRIGGER sessions_ai AFTER INSERT ON sessions BEGIN
                INSERT INTO sessions_fts(rowid, user_prompts, files_modified, commands_run, git_commits, summary)
                VALUES (new.rowid, new.user_prompts, new.files_modified, new.commands_run, new.git_commits, new.summary);
            END;

            CREATE TRIGGER sessions_ad AFTER DELETE ON sessions BEGIN
                INSERT INTO sessions_fts(sessions_fts, rowid, user_prompts, files_modified, commands_run, git_commits, summary)
                VALUES ('delete', old.rowid, old.user_prompts, old.files_modified, old.commands_run, old.git_commits, old.summary);
            END;

            CREATE TRIGGER sessions_au AFTER UPDATE ON sessions BEGIN
                INSERT INTO sessions_fts(sessions_fts, rowid, user_prompts, files_modified, commands_run, git_commits, summary)
                VALUES ('delete', old.rowid, old.user_prompts, old.files_modified, old.commands_run, old.git_commits, old.summary);
                INSERT INTO sessions_fts(rowid, user_prompts, files_modified, commands_run, git_commits, summary)
                VALUES (new.rowid, new.user_prompts, new.files_modified, new.commands_run, new.git_commits, new.summary);
            END;
            ",
        )?;
    }

    let has_notes_fts: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='notes_fts'",
        [],
        |row| row.get(0),
    )?;

    if !has_notes_fts {
        conn.execute_batch(
            "
            CREATE VIRTUAL TABLE notes_fts USING fts5(
                content, tags,
                content=notes, content_rowid=rowid
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
    }

    Ok(())
}
