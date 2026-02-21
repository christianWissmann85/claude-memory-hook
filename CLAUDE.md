# CLAUDE.md — claude-memory

## Overview

Automatic session logging and recall for Claude Code. Captures session metadata via hooks and provides MCP tools for searching past work.

## Quick Commands

```bash
cargo build --release              # Build
cargo test                         # Run tests
cargo clippy                       # Lint

# Manual testing
echo '{"session_id":"test","transcript_path":"/path/to/file.jsonl","cwd":"/project"}' | ./target/release/claude-memory ingest
./target/release/claude-memory status
./target/release/claude-memory search "query"
```

## Architecture

**Single binary, multiple modes:**

| Subcommand | Purpose |
|------------|---------|
| `ingest` | Called by SessionEnd hook, reads transcript, stores in DB |
| `serve` | MCP server (JSON-RPC over stdio) |
| `install` | Set up hooks in ~/.claude/settings.json + .mcp.json |
| `status` | Show DB stats |
| `search` | CLI search (debugging) |

## MCP Tools

| Tool | Description |
|------|-------------|
| `recall` | FTS5 search across sessions |
| `list_sessions` | Browse sessions chronologically |
| `get_session` | Full session details |
| `log_note` | Manual note-taking |
| `search_notes` | Search notes by content/tag |

## Database

- **Location:** `<project>/.claude/memory.db`
- **Engine:** SQLite with WAL mode + FTS5
- **Tables:** `sessions`, `notes`, `sessions_fts`, `notes_fts`

## Project Structure

```
src/
  main.rs           # clap subcommand dispatch
  config.rs         # Project dir detection, DB path
  cli/              # CLI subcommands (ingest, install, status, search)
  mcp/              # MCP server (server.rs) + tools (tools.rs)
  db/               # Database (schema, sessions, notes)
  transcript/       # JSONL parser + metadata extraction
```
