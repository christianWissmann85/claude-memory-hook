pub mod notes;
pub mod schema;
pub mod sessions;

use std::path::Path;

use rusqlite::Connection;

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
