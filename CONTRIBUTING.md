# Contributing to context-mode

Licensed under Elastic License 2.0.

## Prerequisites

- [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code)
- Rust 1.85+

## Local Development Setup

```bash
git clone https://github.com/ShmidtS/context-mode-rust.git context-mode-rust
cd context-mode-rust
cargo build --workspace --release
```

**Symlink the cache** so Claude Code loads your local clone instead of the marketplace version:

```bash
ls ~/.claude/plugins/cache/context-mode-rust/context-mode-rust/
# Replace 0.9.23 with your actual version
mv ~/.claude/plugins/cache/context-mode-rust/context-mode-rust/0.9.23 \
   ~/.claude/plugins/cache/context-mode-rust/context-mode-rust/0.9.23.bak
ln -s /path/to/your/clone/context-mode-rust \
   ~/.claude/plugins/cache/context-mode-rust/context-mode-rust/0.9.23
```

Override PreToolUse in `~/.claude/settings.json`:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash|Read|Grep|WebFetch|Agent|mcp__plugin_context-mode_context-mode__ctx_execute|mcp__plugin_context-mode_context-mode__ctx_execute_file|mcp__plugin_context-mode_context-mode__ctx_batch_execute",
        "hooks": [
          {
            "type": "command",
            "command": "context-mode hook claude-code pretooluse"
          }
        ]
      }
    ]
  }
}
```

> Do NOT add PostToolUse, PreCompact, SessionStart, or UserPromptSubmit to `settings.json` — they are already registered by the plugin. Adding them twice causes double invocations and SQLite locking errors.

Bump version in `Cargo.toml`, `.claude-plugin/plugin.json`, and `.claude-plugin/marketplace.json`, then run `cargo build --workspace --release`. Run `/context-mode:ctx-doctor` to verify.

## Development Workflow

| Command | Purpose |
|---------|---------|
| `cargo build --workspace` | Build all crates |
| `cargo build --workspace --release` | Build optimized binaries |
| `cargo test --workspace` | Run all tests |
| `cargo clippy --workspace -- -D warnings` | Lint |
| `cargo fmt -- --check` | Format check |

| Changed | Rebuild? |
|---------|----------|
| `crates/*` | Yes |
| `configs/*` | No |
| `docs/*` | No |

## TDD Workflow

Red -> Green -> Refactor.

1. Write a failing test in an existing test file. Do NOT create new test files.
2. Write the minimum code to make it pass.
3. Refactor while keeping tests green.

| Domain | Test Location |
|--------|---------------|
| Adapters | `crates/*/src/*` unit tests |
| Search & FTS5 | `crates/*/src/*` unit tests |
| Server & tools | `crates/server/src/*` unit tests |
| CLI | `crates/cli/src/*` unit tests |
| Session DB/extract/snapshot | `crates/*/src/*` unit tests |
| Executor | `crates/*/src/*` unit tests |
| Store | `crates/*/src/*` unit tests |
| Security | `crates/*/src/*` unit tests |

If your change doesn't fit an existing file, discuss with the maintainer first.

## Submitting a PR

1. Fork and create a feature branch from `next`
2. Follow the local development setup above
3. Write tests first (TDD)
4. Run `cargo test --workspace` and `cargo clippy --workspace -- -D warnings`
5. Test in a live Claude Code session
6. Open a PR using the template
