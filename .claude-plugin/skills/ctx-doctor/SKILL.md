---
name: ctx-doctor
description: |
  Run context-mode diagnostics. Checks runtimes, hooks, FTS5,
  plugin registration, npm and marketplace versions.
  Trigger: /context-mode:ctx-doctor
user-invocable: true
---

# Context Mode Doctor

Run diagnostics and display results directly in the conversation.

## Instructions

1. Call the `ctx_doctor` MCP tool directly. It runs all checks server-side and returns a plain-text status report.
2. Display the results verbatim — they are already formatted with plain-text status prefixes: `[OK]` PASS, `[FAIL]` FAIL, `[WARN]` WARN. Renderer-safe (no markdown task-list syntax) for cross-client compatibility (e.g., Z.ai GLM).
3. **Fallback** (only if MCP tool call fails): Derive the **plugin root** from this skill's base directory (go up 2 levels — remove `/skills/ctx-doctor`), then run with Bash:
   ```
   "<PLUGIN_ROOT>/.claude-plugin/bin/context-mode" doctor
   ```
   On Windows use `context-mode.exe`. Re-display results verbatim with the same `[OK]`/`[FAIL]`/`[WARN]` prefixes.
