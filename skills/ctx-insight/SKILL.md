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
2. Display the tool's output verbatim — it contains the dashboard URL (default http://127.0.0.1:3030).
3. If status is "not running", tell the user to run the insight server binary manually: `context-mode-server --insight`.
