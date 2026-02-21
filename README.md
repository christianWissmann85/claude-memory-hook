# claude-memory

Automatic session logging and recall for [Claude Code](https://claude.ai/code). Captures session metadata via hooks and exposes MCP tools for searching past work — so Claude can remember what you worked on across projects and sessions.

> **Platform support:** Auto-capture currently works with **Claude Code only**, via its `SessionEnd` hook and JSONL transcript format. The MCP server is standard JSON-RPC over stdio and can be queried by any MCP-compatible client. Hooks for Aider, Cursor, and others are [planned](#roadmap).

## The Key Idea

Most memory tools require you — or the agent — to actively decide to save something. **claude-memory does nothing of the sort.** The hook fires silently at the end of every session, in the background, without any human or LLM intervention. You just work; the memory accumulates automatically.

## How It Works

1. A `SessionEnd` hook calls `claude-memory ingest` after every Claude Code session — **automatically, with no user action required**.
2. The transcript is parsed and stored in a per-project SQLite database (`.claude/memory.db`).
3. An MCP server (`claude-memory serve`) exposes search and recall tools that Claude can use during future sessions.

## Installation

```bash
# Install from crates.io (once published)
cargo install claude-memory

# Or build from source
cargo build --release && cp target/release/claude-memory ~/.local/bin/

# Run the one-time installer
claude-memory install
```

The `install` command:
- Adds a `SessionEnd` hook to `~/.claude/settings.json`
- Registers the MCP server in `.mcp.json` for the current project

Restart Claude Code to activate — everything else is automatic.

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

## Roadmap

The passive, automatic hook is what makes this useful. Expanding that to more tools is the priority:

| Framework | Status | Notes |
|-----------|--------|-------|
| Claude Code | ✅ Supported | `SessionEnd` hook + JSONL transcripts |
| GitHub Copilot Chat | ✅ Implemented | `claude-memory-vscode` companion extension — see below |
| Aider | 🔲 Planned | Parse `--log` / `.aider.chat.history.md` on session exit |
| Cursor | 🔲 Planned | VS Code extension hook or file watcher on chat history |
| Continue.dev | 🔲 Planned | Continue plugin hook on session close |
| Generic / Manual | 🔲 Planned | `claude-memory ingest --file <transcript>` for any tool |

See the open issues for details and to contribute.

## GitHub Copilot Chat (`claude-memory-vscode`)

The companion VS Code extension lives in [`claude-memory-vscode/`](claude-memory-vscode/).

### How it captures sessions

Because VS Code's stable chat API only exposes conversation history *inside* a chat participant's request handler, capture is **semi-passive**:

- **Manual trigger:** type `@memory save` (or just `@memory`) in any Copilot Chat session. The extension reads the full conversation history via `context.history` and pipes it to `claude-memory ingest --format copilot`.
- **Auto-save on window blur:** when VS Code loses focus, the last captured snapshot is automatically re-ingested (configurable via `claudeMemory.autoSaveOnWindowClose`).
- **Command palette:** `Claude Memory: Save Current Chat Session` — or bind a keyboard shortcut.

> The VS Code chat API does not expose a public `onConversationEnd` event for third-party extensions. The `@memory` participant is the closest fully-stable equivalent to Claude Code's `SessionEnd` hook. A file-watcher approach for fully-passive capture is tracked in [issue #2](https://github.com/christianWissmann85/claude-memory-hook/issues/2).

### Setup

```bash
cd claude-memory-vscode
npm install
npm run compile
# Then install the extension in VS Code:
code --install-extension . # or press F5 to run in Extension Development Host
```

### Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `claudeMemory.binaryPath` | `claude-memory` | Path to the binary (must be on `$PATH` or set to an absolute path) |
| `claudeMemory.autoSaveOnWindowClose` | `true` | Re-ingest the last snapshot when VS Code loses focus |

### `ingest --format copilot`

The extension sends this JSON to `claude-memory ingest --format copilot` via stdin:

```json
{
  "format": "copilot",
  "session_id": "<uuid>",
  "cwd": "/path/to/workspace",
  "captured_at": "2026-02-21T10:00:00Z",
  "model": "gpt-4o",
  "turns": [
    { "role": "user",      "content": "..." },
    { "role": "assistant", "content": "..." }
  ]
}
```

You can also ingest a saved file directly:

```bash
claude-memory ingest --format copilot --file session.json
```


Bug reports, feature requests, and PRs are welcome. If you add support for a new tool's hook, please include a sample transcript in `tests/fixtures/` so the parser can be tested.

## License

MIT
