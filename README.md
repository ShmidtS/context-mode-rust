# Context Mode

## Features

1. **Context Saving** — Sandbox tools keep raw data out of context. 315 KB becomes 5.4 KB (98%).
2. **Session Continuity** — Tracks every edit, git op, task, error in SQLite. Rebuilds state on compact/resume.
3. **Think in Code** — Program the analysis, not compute it. One script replaces ten tool calls.
4. **Output Compression** — Terse output. ~65-75% token reduction.

## Install

**Claude Code** (marketplace):

```bash
/plugin marketplace add ShmidtS/context-mode-rust
```

```bash
/plugin install context-mode-rust@context-mode-rust
```

**All platforms** (from source):

Requires Rust 1.85+.

```bash
git clone https://github.com/ShmidtS/context-mode-rust.git
cd context-mode-rust
cargo build --workspace --release
# binaries are in target/release/context-mode and target/release/context-mode-server
cargo install --path crates/cli    # optional: adds `context-mode` to PATH
cargo install --path crates/server # optional: adds `context-mode-server` to PATH
```

Then register the MCP server in your AI client. For **Claude Code**:

```bash
/mcp add context-mode-rust context-mode-server
```

Or manually add to `~/.claude.json` (replace the path with the absolute path to the binary):

```json
{
  "mcpServers": {
    "context-mode-rust": {
      "command": "context-mode-server"
    }
  }
}
```

For other platforms (Cursor, VS Code Copilot, Gemini CLI, Codex, OpenCode, Zed, and 8 more), see [platform-specific setup](docs/platform-support.md).

## Tools

| Tool | Purpose |
|------|---------|
| `ctx_batch_execute` | Run commands + search queries in one call |
| `ctx_execute` | Run code in 11 languages (sandbox) |
| `ctx_execute_file` | Process files in sandbox |
| `ctx_search` | Query indexed content (BM25) |
| `ctx_index` | Chunk markdown into FTS5 |
| `ctx_fetch_and_index` | Fetch URL, index, 24h cache |
| `ctx_doctor` | Diagnose installation |

Utility commands: `ctx stats`, `ctx upgrade`, `ctx purge`, `ctx insight`.

## How It Works

- **Sandbox** — Isolated subprocess per `ctx_execute`. 11 runtimes (JS, TS, Python, Shell, Ruby, Go, Rust, PHP, Perl, R, Elixir).
- **Knowledge Base** — FTS5 BM25 with Porter stemming + trigram matching via RRF. 14-day auto-cleanup.
- **Session Continuity** — Per-project SQLite DB. Compacts and resumes without repeating context.

## Security

Enforces your existing permission rules in the sandbox. Block `sudo` -> also blocked inside `ctx_execute`. Add rules to `.claude/settings.json`:

```json
{
  "permissions": {
    "deny": ["Bash(sudo *)", "Bash(rm -rf /*)", "Read(.env)", "Read(*/.env*)"],
    "allow": ["Bash(git:*)", "Bash(npm:*)"]
  }
}
```

`ctx_fetch_and_index` blocks dangerous URL targets by default. Tool input redacts keys matching `authorization`, `token`, `secret`, `password`, `api_key`.

## Benchmarks

Over a full session: 315 KB raw -> 5.4 KB context. 94% more context available for problem solving.

[Full 21-scenario benchmark ->](BENCHMARK.md)

## Privacy

Not a cloud service. MCP protocol layer — raw data stays in sandboxed subprocess. No telemetry, no cloud sync, no account. SQLite databases live locally and die when you're done.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development workflow and TDD guidelines.

```bash
git clone https://github.com/ShmidtS/context-mode-rust.git context-mode-rust
cd context-mode-rust && cargo test --workspace
```

## License

Licensed under [Elastic License 2.0](LICENSE) (source-available).
