use crate::config;
use crate::db;

pub fn run(query: &str, limit: usize) -> anyhow::Result<()> {
    let project_dir = config::detect_project_dir()?;
    let db_path = config::db_path(&project_dir);

    if !db_path.exists() {
        println!("No memory database found. Run `claude-memory install` first.");
        return Ok(());
    }

    let conn = db::open(&db_path)?;
    let results = db::sessions::search_sessions(&conn, query, limit)?;

    if results.is_empty() {
        println!("No sessions found matching: {}", query);
        return Ok(());
    }

    println!("Found {} session(s) matching: {}\n", results.len(), query);

    for session in &results {
        let date = &session.started_at[..10.min(session.started_at.len())];
        let duration = session
            .duration_seconds
            .map(format_duration)
            .unwrap_or_else(|| "?".to_string());
        let branch = session
            .git_branch
            .as_deref()
            .unwrap_or("?");

        println!("--- {} | {} | branch: {} ---", date, duration, branch);
        println!("  ID: {}", session.id);

        // Show first user prompt (truncated)
        if let Ok(prompts) = serde_json::from_str::<Vec<String>>(&session.user_prompts) {
            if let Some(first) = prompts.first() {
                let display = if first.len() > 120 {
                    format!("{}...", &first[..120])
                } else {
                    first.clone()
                };
                println!("  First prompt: {}", display);
            }
        }

        // Show files modified
        if let Ok(files) = serde_json::from_str::<Vec<String>>(&session.files_modified) {
            if !files.is_empty() {
                let display: Vec<&str> = files.iter().map(|f| {
                    f.rsplit('/').next().unwrap_or(f)
                }).take(5).collect();
                println!("  Files: {}", display.join(", "));
            }
        }

        println!();
    }

    Ok(())
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
