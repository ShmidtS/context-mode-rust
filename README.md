# context-mode-rust

Rust migration of the context-mode MCP plugin.

## Structure

- `crates/core` — Core types and data structures
- `crates/utils` — Utility functions (truncate, cache, lifecycle)
- `crates/cli` — CLI binary (`context-mode`)
- `crates/server` — HTTP server binary (`context-mode-server`)

## Build

```bash
cargo build
cargo test
cargo run --bin context-mode -- --help
cargo run --bin context-mode-server
```

## Status

Work in progress — foundational crates migrated from TypeScript.
