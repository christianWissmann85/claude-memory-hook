use std::path::{Path, PathBuf};

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
