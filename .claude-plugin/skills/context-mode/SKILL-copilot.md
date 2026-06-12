# Context Mode: VS Code Copilot Integration

For general context-mode concepts, tools, and rules, see [SKILL.md](./SKILL.md).

## Platform Overview

VS Code Copilot uses extension-based integration with hooks scoped to `.github/` directories. It extends `CopilotBaseAdapter` with VS Code-specific session management and extension detection.

## Hook Events

| Hook Event | Supported | Notes |
|------------|-----------|-------|
| preToolUse | Yes | Full support via extension hooks |
| postToolUse | Yes | Context injection after tool output |
| preCompact | Yes | Context preservation across compaction |
| sessionStart | Yes | Inject routing instructions at startup |

## Capabilities

- **canModifyArgs**: Yes -- PreToolUse can modify tool input
- **canModifyOutput**: Yes -- PostToolUse can modify tool output
- **canInjectSessionContext**: Yes -- Via sessionStart and preCompact

## Configuration

| Setting | Path |
|---------|------|
| MCP Config | `.vscode/mcp.json` (project) |
| Extension Dir | `~/.vscode/extensions/context-mode/` |
| Session DB | `.github/context-mode/sessions/{hash}.db` (project-scoped) |
| Session Events | `.github/context-mode/sessions/{hash}-events.md` |
| Config Dir | `.github/` (project-scoped) |

### Session Directory Detection

VS Code Copilot uses project-scoped session storage:
1. Checks for `.github/` directory in project root
2. Falls back to `~/.vscode/` if `.github/` not found

## MCP Registration

Configured in `.vscode/mcp.json`:

```json
{
  "servers": {
    "context-mode-rust": {
      "command": "context-mode-server",
      "args": []
    }
  }
}
```

## Session ID Resolution

Priority order:
1. `session_id` field from hook input
2. `VSCODE_PID` environment variable
3. `pid-{ppid}` fallback

## Project Directory

Resolved from: `CLAUDE_PROJECT_DIR` env (shared with Claude Code compatibility)

## Hook Scripts

Hook commands dispatched via the Rust CLI (`context-mode hook copilot <event>`):
- `pretooluse` -- VS Code-specific PreToolUse handler
- `posttooluse` -- VS Code-specific PostToolUse handler
- `precompact` -- VS Code-specific PreCompact handler
- `sessionstart` -- VS Code-specific SessionStart handler

## Wire Protocol

Uses the same JSON stdin/stdout format as the Copilot base adapter. The `CopilotBaseAdapter` provides shared wire-protocol parsing and response formatting.

## Extension Detection

Plugin registration checks `.vscode/mcp.json` for context-mode server entry. Version detection scans `~/.vscode/extensions/context-mode/package.json`.

## Limitations

- Project-scoped config means hooks must be configured per-project
- No plugin marketplace integration -- manual MCP config required
- Extension-based hooks have preview status -- some features may be incomplete
- JetBrains Copilot shares the same base adapter but uses different hook paths
