# claude-memory

Automatic session logging and recall for [Claude Code](https://claude.ai/code). Captures session metadata via hooks and exposes MCP tools for searching past work — so Claude can remember what you worked on across projects and sessions.

## How It Works

1. A `SessionEnd` hook calls `claude-memory ingest` after every Claude Code session.
2. The transcript is parsed and stored in a per-project SQLite database (`.claude/memory.db`).
3. An MCP server (`claude-memory serve`) exposes search and recall tools that Claude can use during future sessions.

## Installation

```bash
# Build from source
cargo build --release

# Add to PATH, then run the installer
claude-memory install
```

The `install` command:
- Adds a `SessionEnd` hook to `~/.claude/settings.json`
- Registers the MCP server in `.mcp.json` for the current project

Restart Claude Code to activate.

## CLI Reference

| Command | Description |
|---------|-------------|
| `claude-memory install` | Set up hooks and MCP configuration |
| `claude-memory ingest` | Ingest a session transcript (called automatically by the hook) |
| `claude-memory serve` | Start the MCP server (JSON-RPC over stdio) |
| `claude-memory status` | Show database statistics for the current project |
| `claude-memory search <query>` | Search past sessions from the command line |

**Search options:**

```bash
claude-memory search "query"          # default: top 5 results
claude-memory search "query" -l 20   # return up to 20 results
```

The query supports [FTS5 syntax](https://www.sqlite.org/fts5.html) (e.g., `"rust AND async"`, `"refactor*"`).

## MCP Tools

When running as an MCP server, the following tools are available to Claude:

| Tool | Description |
|------|-------------|
| `recall` | Full-text search across all ingested sessions |
| `list_sessions` | Browse sessions chronologically |
| `get_session` | Retrieve full details of a specific session |
| `log_note` | Manually save a note with optional tags |
| `search_notes` | Search notes by content or tag |

## Database

- **Location:** `<project-root>/.claude/memory.db`
- **Engine:** SQLite with WAL mode and FTS5
- **Tables:** `sessions`, `notes`, `sessions_fts`, `notes_fts`

## Development

```bash
cargo build            # Debug build
cargo build --release  # Release build
cargo test             # Run tests
cargo clippy           # Lint

# Manual ingest test
echo '{"session_id":"test","transcript_path":"/path/to/file.jsonl","cwd":"/project"}' \
  | ./target/release/claude-memory ingest
```

## Project Structure

```
src/
  main.rs           # clap subcommand dispatch
  config.rs         # Project dir detection, DB path
  cli/              # CLI subcommands (ingest, install, status, search)
  mcp/              # MCP server (server.rs) + tools (tools.rs)
  db/               # Database layer (schema, sessions, notes)
  transcript/       # JSONL parser + metadata extraction
```

## License

MIT
