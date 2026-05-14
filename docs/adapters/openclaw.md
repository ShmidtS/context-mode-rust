# OpenClaw Adapter

context-mode plugin for the [OpenClaw](https://github.com/openclaw) gateway, targeting **Pi Agent** sessions.

## Overview

**OpenClaw** is the gateway/platform that manages agent sessions, extensions, and tool routing. **Pi Agent** is OpenClaw's coding agent — it runs within OpenClaw and provides Read, Write, Edit, and Bash tools for software development tasks.

The context-mode adapter hooks into Pi Agent sessions specifically, intercepting tool calls to route data-heavy operations through the sandbox and tracking session events for compaction recovery.

### Supported Configurations

- **Pi Agent sessions** with coding tools (Read/Write/Edit/Bash) — fully supported.
- **Custom agents** with coding tools — may work but are untested. The adapter relies on tool names matching Pi Agent's conventions.

## Installation

### Quick install

```bash
cargo build --workspace --release
cargo install --path crates/cli
cargo install --path crates/server
```

This builds the `context-mode` and `context-mode-server` binaries used by OpenClaw.

### Prerequisites

- **Rust 1.85+** must be installed
- **OpenClaw must have been started once** — the setup needs `openclaw.json`, which OpenClaw creates on first launch
- **`OPENCLAW_STATE_DIR`** must point to your OpenClaw state directory (default: `/openclaw`)

### Manual install

For advanced users or custom setups, build from source and configure OpenClaw to launch `context-mode-server` as its MCP server.

See [`docs/platform-support.md`](../platform-support.md) for the shared binary setup.

### Troubleshooting

**"openclaw.json not found"**
OpenClaw creates this file on first launch. Start OpenClaw once (`openclaw gateway start`), then re-run the install script. This is the most common issue for users who install context-mode before ever starting OpenClaw.

**"OPENCLAW_STATE_DIR (/path) does not exist. Is OpenClaw installed?"**
The state directory doesn't exist at the expected path. Check where OpenClaw stores state — common locations are `~/.openclaw` or `/openclaw`. Set `OPENCLAW_STATE_DIR` to the correct path before configuring the MCP server.

**Plugin installed but not loading**
Clear the jiti cache (`rm -f /tmp/jiti/context-mode-*.cjs`) and restart the gateway. If the issue persists, verify the plugin appears in `openclaw plugins list`.

**Plugin loads but `ctx_*` tools are missing from the agent's tool list**
The plugin registers its hooks via `api.on(...)` / `api.registerCommand(...)`, but the agent-callable `ctx_*` tools live in the Rust MCP server. OpenClaw surfaces them by spawning the server as an MCP sidecar declared in `mcp.servers.context-mode`. If you configured OpenClaw manually, verify with `openclaw mcp list` and add it if missing:

```bash
openclaw mcp set context-mode \
  "{\"command\":\"context-mode-server\",\"args\":[]}"
openclaw gateway restart
```

After the restart, the agent's tool inventory should include `context-mode__ctx_execute`, `context-mode__ctx_search`, `context-mode__ctx_fetch_and_index`, and the rest of the `ctx_*` surface (OpenClaw prefixes MCP-sourced tools with the server name).

## Hook Registration

The adapter uses two different registration APIs, matching OpenClaw's internal architecture:

- **`api.on()`** for lifecycle and tool hooks: `session_start`, `before_tool_call`, `after_tool_call`, `before_compaction`, `after_compaction`, `before_prompt_build`, `before_model_resolve`. These are typed event emitters with structured payloads.
- **`api.registerHook()`** for command hooks: `command:new`, `command:reset`, `command:stop`. These use colon-delimited names and the generic hook registration system.

Using the wrong API (e.g., `api.registerHook("before_tool_call", ...)`) registers silently but the hook never fires. This distinction is critical.

## Session Continuity

| Hook | Method | Status |
|---|---|---|
| `after_tool_call` | `api.on()` | Working |
| `before_compaction` | `api.on()` | Working |
| `session_start` | `api.on()` | Working |
| `command:new` | `api.registerHook()` | Working |
| `command:reset` | `api.registerHook()` | Working |
| `command:stop` | `api.registerHook()` | Working |

### Graceful Degradation

If compaction hooks fail to fire (e.g., on older OpenClaw versions), the adapter falls back to **DB snapshot reconstruction** — rebuilding session state from the events already persisted in SQLite by `after_tool_call`. This produces a less precise snapshot than the PreCompact path but preserves critical state (active files, tasks, errors).

## Previously Known Upstream Issues

Both issues below have been resolved in upstream OpenClaw:

- **[#4967](https://github.com/openclaw/openclaw/issues/4967)** — Compaction hooks not firing. Closed as duplicate of [#3728](https://github.com/openclaw/openclaw/issues/3728); fix merged.
- **[#5513](https://github.com/openclaw/openclaw/issues/5513)** — `api.on()` hooks not invoked for tool lifecycle events. Fixed in [PR #9761](https://github.com/openclaw/openclaw/pull/9761).

## Minimum Version

**Required: OpenClaw >2026.1.29**

This is the first release that includes the `api.on()` fix from [PR #9761](https://github.com/openclaw/openclaw/pull/9761), which shipped on 2026-01-29.

**What breaks on older versions:** Lifecycle hooks registered via `api.on()` — including `before_compaction`, `after_compaction`, `session_start`, and tool interception hooks — may silently fail to fire.

**Graceful degradation:** If compaction hooks don't fire, the adapter falls back to DB snapshot reconstruction, rebuilding session state from events already persisted by `after_tool_call`. This produces a less precise snapshot than the PreCompact path but preserves critical state (active files, tasks, errors). The adapter will not crash on older versions, but compaction recovery quality will be reduced.

## Workspace Routing

The adapter resolves project paths from Pi Agent session metadata, ensuring session databases and routing instructions are scoped per-workspace.

## Key Files

| File | Purpose |
|---|---|
| `crates/cli/` | CLI dispatcher and setup commands |
| `crates/server/` | MCP server implementation |
| `configs/openclaw/AGENTS.md` | OpenClaw routing instructions |
