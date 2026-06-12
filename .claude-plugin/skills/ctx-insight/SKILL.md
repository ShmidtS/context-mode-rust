---
name: ctx-insight
description: |
  Open the context-mode Insight analytics dashboard in the browser.
  Shows personal metrics: session activity, tool usage, error rate,
  parallel work patterns, project focus, and actionable insights.
  First run installs dependencies (~30s). Subsequent runs open instantly.
  Trigger: /context-mode:ctx-insight
user-invocable: true
---

# Context Mode Insight

Open the personal analytics dashboard in the browser.

## Instructions

1. Call `ctx_insight` (no params).
2. Display the tool's output verbatim — it contains the dashboard URL (default http://127.0.0.1:3000).
3. The tool automatically starts the insight server if not already running.
4. If the tool reports "binary not found", reinstall the plugin or build with:
   ```
   cargo build --release --bin context-mode-insight
   ```
5. Open the returned URL in the browser to view the dashboard.
