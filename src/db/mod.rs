pub mod notes;
pub mod schema;
pub mod sessions;

use std::path::Path;

use rusqlite::Connection;

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
