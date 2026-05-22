use crate::tool_handlers::format_tool_result;
use rmcp::{
    ServiceExt, handler::server::wrapper::Parameters, schemars, tool, tool_router, transport::stdio,
};
use serde_json::json;

#[derive(Clone)]
pub struct ContextModeServer;

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxExecuteParams {
    #[schemars(
        description = "Programming language for the code snippet (e.g., 'python', 'shell', 'javascript')"
    )]
    language: String,
    #[schemars(description = "Source code to execute in the sandbox")]
    code: String,
    #[schemars(description = "Maximum execution time in milliseconds (default: 30000)")]
    timeout: Option<u64>,
    #[schemars(description = "Run in background without waiting for completion")]
    background: Option<bool>,
    #[schemars(
        description = "Natural-language intent describing what the code should produce, used for output summarization"
    )]
    intent: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxExecuteFileParams {
    #[schemars(description = "Absolute path to the file to analyze")]
    path: String,
    #[schemars(
        description = "Programming language for the analysis code (e.g., 'python', 'shell', 'javascript')"
    )]
    language: String,
    #[schemars(description = "Source code that reads FILE_CONTENT and produces output")]
    code: String,
    #[schemars(description = "Maximum execution time in milliseconds (default: 30000)")]
    timeout: Option<u64>,
    #[schemars(description = "Natural-language intent describing what the code should produce")]
    intent: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxBatchExecuteParams {
    #[schemars(description = "List of labeled shell commands to execute in parallel")]
    commands: Vec<CtxBatchCommand>,
    #[schemars(description = "BM25 search queries against combined stdout of all commands")]
    queries: Option<Vec<String>>,
    #[schemars(description = "Max number of commands to run concurrently (default: unlimited)")]
    concurrency: Option<u64>,
    #[schemars(description = "Per-command timeout in milliseconds (default: 30000)")]
    timeout: Option<u64>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxBatchCommand {
    #[schemars(description = "Human-readable label for the command, used in output")]
    label: String,
    #[schemars(description = "Shell command string to execute")]
    command: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxIndexParams {
    #[schemars(description = "Raw text content to index directly")]
    content: Option<String>,
    #[schemars(description = "Absolute path to a file to read and index server-side")]
    path: Option<String>,
    #[schemars(description = "Source tag for attribution (e.g., 'Zod', 'React')")]
    source: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxFetchAndIndexParams {
    #[schemars(description = "URL to fetch and index")]
    url: String,
    #[schemars(description = "HTTP request timeout in milliseconds")]
    timeout: Option<u64>,
    #[schemars(description = "Source tag for attribution")]
    source: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxSearchParams {
    #[schemars(
        description = "BM25 search queries (OR semantics — more matching terms rank higher)"
    )]
    queries: Vec<String>,
    #[schemars(description = "Maximum number of results to return")]
    limit: Option<usize>,
    #[schemars(description = "Filter results by source tag")]
    source: Option<String>,
    #[schemars(description = "Filter results by MIME-like content type")]
    content_type: Option<String>,
    #[schemars(description = "Result ordering (default: relevance)")]
    sort: Option<String>,
    #[schemars(description = "Approximate max tokens to return")]
    token_budget: Option<usize>,
    #[schemars(description = "Minimum FTS5 match score threshold")]
    min_confidence: Option<f32>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxVaultIndexParams {
    #[schemars(description = "Absolute path to the markdown vault directory to index")]
    path: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxVaultGraphParams {
    #[schemars(
        description = "Traversal mode: neighbors, backlinks, tag-cluster, surprises, confidence-filter"
    )]
    mode: String,
    #[schemars(description = "Starting vault note path for graph traversal")]
    node_path: Option<String>,
    #[schemars(description = "Maximum BFS hops from the starting node")]
    max_hops: Option<usize>,
    #[schemars(description = "Filter results by frontmatter tag")]
    tag: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxSemanticSearchParams {
    #[schemars(description = "Natural-language query for semantic vector search")]
    query: String,
    #[schemars(description = "Maximum number of results to return")]
    limit: Option<usize>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxIndexEmbeddingsParams {
    #[schemars(description = "Raw text to embed and store in the vector index")]
    content: String,
    #[schemars(description = "Source tag for attribution")]
    source: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxContextPackParams {
    #[schemars(description = "Natural-language query describing the context needed")]
    query: String,
    #[schemars(description = "Approximate max tokens for the returned context pack")]
    token_budget: Option<usize>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxConnectorAddParams {
    #[schemars(description = "Unique connector name to add or reference")]
    name: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxLocalIndexParams {
    #[schemars(description = "Absolute path to the code repository to index")]
    path: String,
    #[schemars(description = "Custom repository identifier (default: derived from path)")]
    repo_id: Option<String>,
    #[schemars(description = "Force full reindex instead of incremental update")]
    fresh: Option<bool>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxLocalSearchParams {
    #[schemars(description = "FTS5 BM25 query against indexed code")]
    query: String,
    #[schemars(description = "Repository identifier to search within")]
    repo_id: Option<String>,
    #[schemars(description = "Maximum number of results to return")]
    limit: Option<usize>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxLocalStatusParams {
    #[schemars(description = "Indexing job identifier returned by ctx_local_index")]
    job_id: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxLocalReposParams {}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxLocalWatchParams {
    #[schemars(description = "Absolute path to the repository to watch for changes")]
    path: String,
    #[schemars(description = "Custom repository identifier")]
    repo_id: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxLocalUnwatchParams {
    #[schemars(description = "Repository identifier to stop watching")]
    repo_id: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxDeadCodeParams {
    #[schemars(description = "Absolute path to the source file to analyze")]
    path: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxComplexityParams {
    #[schemars(description = "Absolute path to the source file to analyze")]
    path: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxDepGraphParams {
    #[schemars(description = "List of source file paths to analyze")]
    paths: Option<Vec<String>>,
    #[schemars(description = "Single source file path to analyze")]
    path: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxGraphAnalyzeParams {
    #[schemars(description = "Absolute path to the indexed vault directory")]
    path: String,
    #[schemars(description = "Max number of high-centrality (god) nodes to report")]
    god_node_limit: Option<usize>,
    #[schemars(description = "Max number of unexpected cross-community links to report")]
    surprise_limit: Option<usize>,
    #[schemars(description = "Max number of note communities to report")]
    community_limit: Option<usize>,
    #[schemars(description = "Max number of follow-up questions to suggest")]
    question_limit: Option<usize>,
}

fn params_to_value<T: serde::Serialize>(params: T) -> serde_json::Value {
    serde_json::to_value(params).unwrap_or_default()
}

async fn call_tool<F, Fut>(tool_call: F) -> String
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<serde_json::Value>>,
{
    match tool_call().await {
        Ok(result) => format_tool_result(result),
        Err(e) => format!("Error: {e}"),
    }
}

#[tool_router]
impl ContextModeServer {
    #[tool(
        description = "Execute code in a sandboxed subprocess. MANDATORY for output >20 lines. Supports 11 languages."
    )]
    async fn ctx_execute(&self, Parameters(params): Parameters<CtxExecuteParams>) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::execute::ctx_execute(value)).await
    }

    #[tool(
        description = "Execute analysis code against a file without loading raw file contents into context."
    )]
    async fn ctx_execute_file(
        &self,
        Parameters(params): Parameters<CtxExecuteFileParams>,
    ) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::execute::ctx_execute_file(value)).await
    }

    #[tool(description = "Execute multiple commands and search indexed output.")]
    async fn ctx_batch_execute(
        &self,
        Parameters(params): Parameters<CtxBatchExecuteParams>,
    ) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::batch::ctx_batch_execute(value)).await
    }

    #[tool(description = "Index content or a file path into the context-mode store.")]
    async fn ctx_index(&self, Parameters(params): Parameters<CtxIndexParams>) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::search::ctx_index(value)).await
    }

    #[tool(description = "Search indexed context-mode content.")]
    async fn ctx_search(&self, Parameters(params): Parameters<CtxSearchParams>) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::search::ctx_search(value)).await
    }

    #[tool(description = "Fetch remote content and index it.")]
    async fn ctx_fetch_and_index(
        &self,
        Parameters(params): Parameters<CtxFetchAndIndexParams>,
    ) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::fetch_index::ctx_fetch_and_index(value)).await
    }

    #[tool(description = "Show context-mode session statistics.")]
    async fn ctx_stats(&self) -> String {
        call_tool(context_mode_tools::stats::ctx_stats).await
    }

    #[tool(description = "Run context-mode diagnostics.")]
    async fn ctx_doctor(&self) -> String {
        call_tool(context_mode_tools::stats::ctx_doctor).await
    }

    #[tool(description = "Return the current context-mode server version.")]
    async fn ctx_upgrade(&self) -> String {
        let result = json!({
            "content": [{
                "type": "text",
                "text": format!("context-mode v{}", env!("CARGO_PKG_VERSION"))
            }],
            "isError": false,
        });
        format_tool_result(result)
    }

    #[tool(description = "Purge the context-mode knowledge base and reset session stats.")]
    async fn ctx_purge(&self) -> String {
        call_tool(context_mode_tools::stats::ctx_purge).await
    }

    #[tool(description = "Start the context-mode insight dashboard and return its URL.")]
    async fn ctx_insight(&self) -> String {
        use std::process::Command;

        let ext = if cfg!(windows) { ".exe" } else { "" };
        let bin_name = format!("context-mode-insight{}", ext);

        let candidates = [
            std::env::current_dir()
                .unwrap_or_default()
                .join(".claude-plugin")
                .join("bin")
                .join(&bin_name),
            std::env::current_dir()
                .unwrap_or_default()
                .join("target")
                .join("release")
                .join(&bin_name),
            std::env::current_dir()
                .unwrap_or_default()
                .join("target")
                .join("debug")
                .join(&bin_name),
        ];

        let binary = candidates.iter().find(|p| p.exists()).cloned();

        if let Some(bin) = binary {
            // Check if already running
            let health = reqwest::get("http://127.0.0.1:3000/health").await;
            let already_running = health.map(|r| r.status().is_success()).unwrap_or(false);

            if !already_running {
                match Command::new(&bin)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    Ok(_) => {
                        // Wait briefly for server startup
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        let result = json!({
                            "content": [{
                                "type": "text",
                                "text": "Dashboard URL: http://127.0.0.1:3000\n\
                                         Status: running\n\
                                         Open the URL above in your browser to view the insight dashboard."
                            }],
                            "isError": false,
                        });
                        format_tool_result(result)
                    }
                    Err(e) => {
                        let result = json!({
                            "content": [{
                                "type": "text",
                                "text": format!(
                                    "Failed to start insight server: {}\n\
                                     Binary: {}\n\
                                     Try running manually: {} --help",
                                    e,
                                    bin.display(),
                                    bin.display()
                                )
                            }],
                            "isError": true,
                        });
                        format_tool_result(result)
                    }
                }
            } else {
                let result = json!({
                    "content": [{
                        "type": "text",
                        "text": "Dashboard URL: http://127.0.0.1:3000\n\
                                 Status: running (already started)\n\
                                 Open the URL above in your browser to view the insight dashboard."
                    }],
                    "isError": false,
                });
                format_tool_result(result)
            }
        } else {
            let result = json!({
                "content": [{
                    "type": "text",
                    "text": format!(
                        "Insight server binary not found.\n\
                         Expected: context-mode-insight{}\n\
                         Searched:\n  - {}\n  - {}\n  - {}\n\n\
                         Reinstall the plugin or build with:\n\
                         cargo build --release --bin context-mode-insight",
                        ext,
                        candidates[0].display(),
                        candidates[1].display(),
                        candidates[2].display()
                    )
                }],
                "isError": true,
            });
            format_tool_result(result)
        }
    }

    #[tool(description = "Search indexed embeddings semantically.")]
    async fn ctx_semantic_search(
        &self,
        Parameters(params): Parameters<CtxSemanticSearchParams>,
    ) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::context_stream::ctx_semantic_search(value)).await
    }

    #[tool(description = "Index content into the embedding store.")]
    async fn ctx_index_embeddings(
        &self,
        Parameters(params): Parameters<CtxIndexEmbeddingsParams>,
    ) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::context_stream::ctx_index_embeddings(value)).await
    }

    #[tool(description = "Build a context pack for a query under a token budget.")]
    async fn ctx_context_pack(
        &self,
        Parameters(params): Parameters<CtxContextPackParams>,
    ) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::context_stream::ctx_context_pack(value)).await
    }

    #[tool(description = "Index an Obsidian-style markdown vault graph.")]
    async fn ctx_vault_index(&self, Parameters(params): Parameters<CtxVaultIndexParams>) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::vault::ctx_vault_index(value)).await
    }

    #[tool(description = "Traverse indexed vault graph relationships.")]
    async fn ctx_vault_graph(&self, Parameters(params): Parameters<CtxVaultGraphParams>) -> String {
        let mut value = params_to_value(&params);
        if let serde_json::Value::Object(ref mut object) = value {
            if let Some(node_path) = object.remove("node_path") {
                object.insert("nodePath".to_string(), node_path);
            }
            if let Some(max_hops) = object.remove("max_hops") {
                object.insert("maxHops".to_string(), max_hops);
            }
        }
        call_tool(|| context_mode_tools::vault::ctx_vault_graph(value)).await
    }

    #[tool(description = "Find potentially dead functions and methods in a source file.")]
    async fn ctx_dead_code(&self, Parameters(params): Parameters<CtxDeadCodeParams>) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::code_analysis::ctx_dead_code(value)).await
    }

    #[tool(description = "Estimate function and method cyclomatic complexity in a source file.")]
    async fn ctx_complexity(&self, Parameters(params): Parameters<CtxComplexityParams>) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::code_analysis::ctx_complexity(value)).await
    }

    #[tool(description = "Build dependency graph edges from source file imports.")]
    async fn ctx_dep_graph(&self, Parameters(params): Parameters<CtxDepGraphParams>) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::code_analysis::ctx_dep_graph(value)).await
    }

    #[tool(
        description = "Analyze vault graph: detect god nodes, surprising connections, communities, and suggest questions."
    )]
    async fn ctx_graph_analyze(
        &self,
        Parameters(params): Parameters<CtxGraphAnalyzeParams>,
    ) -> String {
        let value = params_to_value(&params);
        call_tool(|| context_mode_tools::vault::ctx_graph_analyze(value)).await
    }

    #[tool(description = "List configured context-mode connectors.")]
    async fn ctx_connector_list(&self) -> String {
        call_tool(context_mode_tools::connectors::ctx_connector_list).await
    }

    #[tool(description = "Add a context-mode connector.")]
    async fn ctx_connector_add(
        &self,
        Parameters(params): Parameters<CtxConnectorAddParams>,
    ) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::connectors::ctx_connector_add(value)).await
    }

    #[tool(description = "Sync a context-mode connector.")]
    async fn ctx_connector_sync(
        &self,
        Parameters(params): Parameters<CtxConnectorAddParams>,
    ) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::connectors::ctx_connector_sync(value)).await
    }

    #[tool(description = "Index a local source code repository for semantic and full-text search.")]
    async fn ctx_local_index(&self, Parameters(params): Parameters<CtxLocalIndexParams>) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::local_index::ctx_local_index(value)).await
    }

    #[tool(description = "Search indexed local code repositories using FTS5 BM25.")]
    async fn ctx_local_search(
        &self,
        Parameters(params): Parameters<CtxLocalSearchParams>,
    ) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::local_index::ctx_local_search(value)).await
    }

    #[tool(description = "Check the status of a local indexing job.")]
    async fn ctx_local_status(
        &self,
        Parameters(params): Parameters<CtxLocalStatusParams>,
    ) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::local_index::ctx_local_status(value)).await
    }

    #[tool(description = "List all indexed local repositories.")]
    async fn ctx_local_repos(
        &self,
        Parameters(_params): Parameters<CtxLocalReposParams>,
    ) -> String {
        let value = params_to_value(_params);
        call_tool(|| context_mode_tools::local_index::ctx_local_repos(value)).await
    }

    #[tool(description = "Watch a local repository for changes and auto-reindex.")]
    async fn ctx_local_watch(&self, Parameters(params): Parameters<CtxLocalWatchParams>) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::local_index::ctx_local_watch(value)).await
    }

    #[tool(description = "Stop watching a local repository.")]
    async fn ctx_local_unwatch(
        &self,
        Parameters(params): Parameters<CtxLocalUnwatchParams>,
    ) -> String {
        let value = params_to_value(params);
        call_tool(|| context_mode_tools::local_index::ctx_local_unwatch(value)).await
    }
}

#[rmcp::tool_handler(router = ContextModeServer::tool_router(), name = "context-mode")]
impl rmcp::ServerHandler for ContextModeServer {}

pub async fn run_server() -> anyhow::Result<()> {
    // Cleanup stale temp DB files from dead processes
    let cleaned = context_mode_store::cleanup_stale_dbs();
    if cleaned > 0 {
        eprintln!("[context-mode] Cleaned up {cleaned} stale DB file(s)");
    }
    // Cleanup stale content DBs in .context-mode/ older than 7 days
    let content_cleaned = context_mode_store::cleanup_stale_content_dbs(".context-mode", 7);
    if content_cleaned > 0 {
        eprintln!("[context-mode] Cleaned up {content_cleaned} stale content DB(s)");
    }

    let server = ContextModeServer;
    let service = server.serve(stdio()).await?;

    tokio::select! {
        result = service.waiting() => {
            result?;
        }
        _ = shutdown_signal() => {
            eprintln!("[context-mode] Received shutdown signal, exiting");
        }
    }
    Ok(())
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
    tokio::select! {
        _ = sigterm.recv() => {}
        _ = tokio::signal::ctrl_c() => {}
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl+c event");
}
