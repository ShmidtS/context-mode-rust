---
name: ctx-upgrade
description: |
  Update context-mode from GitHub and fix hooks/settings.
  Pulls latest, builds, installs, updates npm global, configures hooks.
  Trigger: /context-mode:ctx-upgrade
user-invocable: true
---

# Context Mode Upgrade

Pull latest from GitHub and reinstall the plugin.

## Instructions

1. Call the `ctx_upgrade` MCP tool directly. It returns the current server version.
2. To perform a full upgrade (pull latest, rebuild, reinstall), run with Bash:
   ```
   cd <PLUGIN_ROOT> && git pull && cargo build --release && cargo install --path crates/mcp
   ```
   Replace `<PLUGIN_ROOT>` with the plugin root directory.
3. Display results as a markdown checklist:
   ```
   ## context-mode upgrade
   - [x] Pulled latest from GitHub
   - [x] Built and installed v1.0.39
   - [x] Hooks configured
   - [x] Doctor: all checks PASS
   ```
   Use `[x]` for success, `[ ]` for failure. Show actual version numbers.
4. Tell the user to **restart their session** to pick up the new version.
5. **Fallback** (only if MCP tool call fails): Derive the **plugin root** from this skill's base directory (go up 2 levels — remove `/skills/ctx-upgrade`), then run with Bash:
   ```
   cd "<PLUGIN_ROOT>" && git pull && cargo build --release && cargo install --path crates/mcp
   ```
