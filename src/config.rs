use std::path::{Path, PathBuf};

/// A discovered project with a memory database.
pub struct DiscoveredProject {
    pub project_dir: PathBuf,
    pub db_path: PathBuf,
}

/// Scan for all projects with memory databases.
///
/// Searches at two depths under `$HOME`:
///   `$HOME/*/.claude/memory.db`   (depth 1)
///   `$HOME/*/*/.claude/memory.db` (depth 2)
pub fn discover_project_dbs() -> Vec<DiscoveredProject> {
    let home = match std::env::var("HOME") {
        Ok(h) => PathBuf::from(h),
        Err(_) => return Vec::new(),
    };

    let mut results = Vec::new();

    // Depth 1: $HOME/<project>/.claude/memory.db
    scan_depth(&home, &mut results);

    // Depth 2: $HOME/<dir>/<project>/.claude/memory.db
    if let Ok(entries) = std::fs::read_dir(&home) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_depth(&path, &mut results);
            }
        }
    }

    results.sort_by(|a, b| a.project_dir.cmp(&b.project_dir));
    results
}

/// Scan immediate children of `parent` for `.claude/memory.db`.
fn scan_depth(parent: &Path, results: &mut Vec<DiscoveredProject>) {
    let entries = match std::fs::read_dir(parent) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let project_dir = entry.path();
        if !project_dir.is_dir() {
            continue;
        }
        let db = project_dir.join(".claude").join("memory.db");
        if db.is_file() {
            results.push(DiscoveredProject {
                project_dir,
                db_path: db,
            });
        }
    }
}

/// Find the project root by walking up from `start` looking for `.git/`.
/// Falls back to `start` itself if no `.git/` found.
pub fn find_project_root(start: &Path) -> PathBuf {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join(".git").exists() {
            return dir;
        }
        if !dir.pop() {
            return start.to_path_buf();
        }
    }
}

/// Resolve the database path for a project: `<project>/.claude/memory.db`
pub fn db_path(project_dir: &Path) -> PathBuf {
    project_dir.join(".claude").join("memory.db")
}

/// Detect the project directory from the environment.
///
/// Priority:
/// 1. `CLAUDE_MEMORY_PROJECT` env var
/// 2. Current working directory → walk up to find `.git/`
pub fn detect_project_dir() -> anyhow::Result<PathBuf> {
    if let Ok(project) = std::env::var("CLAUDE_MEMORY_PROJECT") {
        let path = PathBuf::from(project);
        anyhow::ensure!(path.is_dir(), "CLAUDE_MEMORY_PROJECT is not a directory: {}", path.display());
        return Ok(path);
    }

    let cwd = std::env::current_dir()?;
    Ok(find_project_root(&cwd))
}
