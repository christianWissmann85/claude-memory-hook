use crate::config;
use crate::db;

pub fn run() -> anyhow::Result<()> {
    let project_dir = config::detect_project_dir()?;
    let db_path = config::db_path(&project_dir);

    if !db_path.exists() {
        println!("No memory database found at {}", db_path.display());
        println!("Run `claude-memory install` to set up automatic session logging.");
        return Ok(());
    }

    let conn = db::open(&db_path)?;

    let (session_count, total_input, total_output) = db::sessions::session_stats(&conn)?;
    let note_count = db::notes::note_count(&conn)?;

    // Get DB file size
    let file_size = std::fs::metadata(&db_path)?.len();

    // Get date range
    let date_range: Option<(String, String)> = conn
        .query_row(
            "SELECT MIN(started_at), MAX(started_at) FROM sessions",
            [],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .ok()
        .and_then(|(min, max)| Some((min?, max?)));

    println!("claude-memory status");
    println!("====================");
    println!("Project:    {}", project_dir.display());
    println!("Database:   {}", db_path.display());
    println!("DB size:    {}", format_bytes(file_size));
    println!();
    println!("Sessions:   {}", session_count);
    println!("Notes:      {}", note_count);
    println!(
        "Tokens:     {} input / {} output",
        format_number(total_input),
        format_number(total_output)
    );

    if let Some((min, max)) = date_range {
        println!("Date range: {} to {}", &min[..10.min(min.len())], &max[..10.min(max.len())]);
    }

    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn format_number(n: i64) -> String {
    if n < 1_000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    }
}
