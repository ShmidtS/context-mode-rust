# context-mode

Raw tool output floods context window. Use context-mode MCP tools to keep raw data in sandbox.

## Think in Code

Analyze/count/filter/compare/search/parse/transform data: **write code** via `ctx_execute(language, code)`, `console.log()` only the answer. Do NOT read raw data into context. One script replaces ten tool calls.

## Build & Test

| Command | Purpose |
|---------|---------|
| `cargo build --workspace` | Build all crates |
| `cargo build --workspace --release` | Optimized release build |
| `cargo test --workspace` | Run all tests |
| `cargo clippy --workspace -- -D warnings` | Lint |
| `cargo fmt -- --check` | Format check |

## Vault Graph

- `ctx_vault_index({ vaultPath })` — builds graph of notes, wiki-links, tags, frontmatter.
- `ctx_vault_graph({ mode, nodePath|tag, limit })` — modes: `neighbors` (BFS), `backlinks` (reverse), `tag-cluster`.

Prefer `ctx_search` for code queries; vault graph for markdown document relationships.

For tool selection, decision trees, and critical rules see [context-mode skill](skills/context-mode/SKILL.md).
