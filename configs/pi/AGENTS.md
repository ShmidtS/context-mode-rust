# context-mode — MANDATORY routing rules

context-mode MCP tools available. Rules protect context window from flooding. One unrouted command dumps 56 KB into context. Pi enforces routing via hooks (`tool_call` blocks `curl`/`wget`) AND these instructions. Hooks = hard enforcement; rules = completeness for redirections hooks cannot catch.

## Think in Code — MANDATORY

Analyze/count/filter/compare/search/parse/transform data: **write code** via `ctx_execute(language, code)`, `console.log()` only the answer. Do NOT read raw data into context. PROGRAM the analysis, not COMPUTE it. Use the smallest sandbox language that fits. Handle errors and empty values explicitly. One script replaces ten tool calls.

## BLOCKED — do NOT use

### curl / wget — FORBIDDEN (hook-enforced)
Do NOT use `curl`/`wget` in `bash`. Pi hooks block these. Dumps raw HTTP into context.
Use: `ctx_fetch_and_index(url, source)` or `ctx_execute(language: "python", code: "...")`

### Inline HTTP — FORBIDDEN
No inline HTTP one-liners (`fetch`, `requests.get`, `http.get`). Bypasses sandbox.
Use: `ctx_execute(language, code)` — only stdout enters context

### Direct web fetching — FORBIDDEN
Raw HTML can exceed 100 KB.
Use: `ctx_fetch_and_index(url, source)` then `ctx_search(queries)`

## REDIRECTED — use sandbox

### bash (>20 lines output)
`bash` ONLY for: `git`, `mkdir`, `rm`, `mv`, `cd`, `ls`, `cargo build`, `pip install`.
Otherwise: `ctx_batch_execute(commands, queries)` or `ctx_execute(language: "shell", code: "...")`

### read (for analysis)
Reading to **edit** → `read` correct. Reading to **analyze/explore/summarize** → `ctx_execute_file(path, language, code)`.

### grep / find (large results)
Use `ctx_execute(language: "shell", code: "grep ...")` in sandbox.

## Tool selection

0. **MEMORY**: `ctx_search(sort: "timeline")` — after resume, check prior context before asking user.
1. **GATHER**: `ctx_batch_execute(commands, queries)` — runs all commands, auto-indexes, returns search. ONE call replaces 30+. Each command: `{label: "header", command: "..."}`. Now supports concurrency N for I/O-bound work.
2. **FOLLOW-UP**: `ctx_search(queries: ["q1", "q2", ...])` — all questions as array, ONE call (default relevance mode). Returns ranked snippets from indexed content.
3. **PROCESSING**: `ctx_execute(language, code)` | `ctx_execute_file(path, language, code)` — sandboxed subprocess, only stdout enters context. Supports 11 languages, timeout, hard-cap kill, env sanitization. Use for data transformation, log analysis, JSON parsing, counting, filtering.
4. **WEB**: `ctx_fetch_and_index(url, source)` then `ctx_search(queries)` — raw HTML never enters context. Supports batch fetch with concurrency.
5. **INDEX**: `ctx_index(content, source)` — store in FTS5 for later search. Use after generating large content that may be referenced again.
6. **SEMANTIC SEARCH**: `ctx_semantic_search(query, top_k)` | `ctx_index_embeddings(content, source)` | `ctx_context_pack(query, budget)` — vector-based search and context packing under token budget. Use when semantic similarity matters more than exact keyword match.
7. **LOCAL CODE INDEX**: `ctx_local_index(repo_path, label)` | `ctx_local_search(queries, label)` | `ctx_local_watch(label, path)` | `ctx_local_unwatch(label)` — index and search local source repositories with FTS5 + hybrid RRF ranking. Supports auto-reindex on file changes.
8. **VAULT / GRAPH**: `ctx_vault_index(vault_path)` | `ctx_vault_graph(mode, node, tag)` | `ctx_graph_analyze(vault_path)` — index markdown vaults (Obsidian-style), traverse wiki-links, backlinks, tag clusters, detect god nodes and communities.
9. **CODE ANALYSIS**: `ctx_dead_code(file_path)` | `ctx_complexity(file_path)` | `ctx_dep_graph(file_path)` — find dead functions, estimate cyclomatic complexity, build dependency graph edges from imports.
10. **CONNECTORS**: `ctx_connector_list()` | `ctx_connector_add(name, config)` | `ctx_connector_sync(name)` — manage external data connectors (GitHub, Linear, Slack, etc.) for automated indexing.
11. **STATS / MAINTENANCE**: `ctx_stats()` | `ctx_doctor()` | `ctx_upgrade()` | `ctx_purge()` — session statistics, diagnostics, version upgrade, wipe knowledge base.

## Parallel I/O batches

For multi-URL fetches or multi-API calls, **always** include `concurrency: N` (1-8):

- `ctx_batch_execute(commands: [3+ network commands], concurrency: 5)` — gh, curl, dig, docker inspect, multi-region cloud queries
- `ctx_fetch_and_index(requests: [{url, source}, ...], concurrency: 5)` — multi-URL batch fetch

**Use concurrency 4-8** for I/O-bound work (network calls, API queries). **Keep concurrency 1** for CPU-bound (cargo test, build, clippy) or commands sharing state (ports, lock files, same-repo writes).

GitHub API rate-limit: cap at 4 for `gh` calls.

## Output

Terse like caveman. Technical substance exact. Only fluff die.
Drop: articles, filler (just/really/basically), pleasantries, hedging. Fragments OK. Short synonyms. Code unchanged.
Pattern: [thing] [action] [reason]. [next step]. Auto-expand for: security warnings, irreversible actions, user confusion.
Write artifacts to FILES — never inline. Return: file path + 1-line description.
Descriptive source labels for `ctx_search(source: "label")`.

## Session Continuity

Skills, roles, and decisions persist for the entire session. Do not abandon them as the conversation grows.

## Memory

Session history is persistent and searchable. On resume, search BEFORE asking the user:

| Need | Command |
|------|---------|
| What were we working on? | `ctx_search(queries: ["summary"], source: "compaction", sort: "timeline")` |
| What did we decide? | `ctx_search(queries: ["decision"], source: "decision", sort: "timeline")` |
| What NOT to repeat? | `ctx_search(queries: ["rejected"], source: "rejected-approach")` |
| What constraints exist? | `ctx_search(queries: ["constraint"], source: "constraint")` |

Note: user-prompt history not available.

DO NOT ask "what were we working on?" — SEARCH FIRST.
If search returns 0 results, proceed as a fresh session.

## ctx commands

| Command | Action |
|---------|--------|
| `ctx stats` | Call `ctx_stats` MCP tool, display full output verbatim |
| `ctx doctor` | Call `ctx_doctor` MCP tool, run returned shell command, display as checklist |
| `ctx upgrade` | Call `ctx_upgrade` MCP tool, run returned shell command, display as checklist |
| `ctx purge` | Call `ctx_purge` MCP tool with confirm: true. Warns before wiping knowledge base. |

After /clear or /compact: knowledge base and session stats preserved. Use `ctx purge` to start fresh.
