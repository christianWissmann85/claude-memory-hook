use std::path::PathBuf;

use serde_json::{json, Value};

pub fn run() -> anyhow::Result<()> {
    install_global_hook()?;
    install_project_mcp()?;
    println!("Installation complete! Restart Claude Code to activate.");
    Ok(())
}

/// Add SessionEnd hook to ~/.claude/settings.json
fn install_global_hook() -> anyhow::Result<()> {
    let settings_path = dirs_settings_path();

    let mut settings: Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content)?
    } else {
        json!({})
    };

    // Check if hook already installed
    if let Some(hooks) = settings.get("hooks") {
        if let Some(session_end) = hooks.get("SessionEnd") {
            if let Some(arr) = session_end.as_array() {
                for entry in arr {
                    if let Some(inner_hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
                        for h in inner_hooks {
                            if h.get("command")
                                .and_then(|c| c.as_str())
                                .is_some_and(|c| c.contains("claude-memory"))
                            {
                                println!("SessionEnd hook already installed in ~/.claude/settings.json");
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
    }

    // Add the hook
    let hook = json!({
        "hooks": [{
            "type": "command",
            "command": "claude-memory ingest",
            "timeout": 10
        }]
    });

    let hooks = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| json!({}));

    let session_end = hooks
        .as_object_mut()
        .unwrap()
        .entry("SessionEnd")
        .or_insert_with(|| json!([]));

    session_end.as_array_mut().unwrap().push(hook);

    // Ensure parent dir exists
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let formatted = serde_json::to_string_pretty(&settings)?;
    std::fs::write(&settings_path, formatted)?;

    println!("Added SessionEnd hook to {}", settings_path.display());
    Ok(())
}

/// Add MCP server to current project's .mcp.json
fn install_project_mcp() -> anyhow::Result<()> {
    let project_dir = crate::config::detect_project_dir()?;
    let mcp_path = project_dir.join(".mcp.json");

    let mut mcp: Value = if mcp_path.exists() {
        let content = std::fs::read_to_string(&mcp_path)?;
        serde_json::from_str(&content)?
    } else {
        json!({})
    };

    // Check if already configured
    if let Some(servers) = mcp.get("mcpServers") {
        if servers.get("claude-memory").is_some() {
            println!("MCP server already configured in {}", mcp_path.display());
            return Ok(());
        }
    }

    // Add the MCP server
    let servers = mcp
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert_with(|| json!({}));

    servers.as_object_mut().unwrap().insert(
        "claude-memory".to_string(),
        json!({
            "command": "claude-memory",
            "args": ["serve"]
        }),
    );

    let formatted = serde_json::to_string_pretty(&mcp)?;
    std::fs::write(&mcp_path, formatted)?;

    println!("Added claude-memory MCP server to {}", mcp_path.display());
    Ok(())
}

fn dirs_settings_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".claude").join("settings.json")
}
