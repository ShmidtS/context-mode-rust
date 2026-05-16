use crate::tool_handlers::format_tool_result;
use rmcp::{
    ServiceExt, handler::server::wrapper::Parameters, schemars, tool, tool_router, transport::stdio,
};
use serde_json::json;

#[derive(Clone)]
pub struct ContextModeServer;

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxExecuteParams {
    language: String,
    code: String,
    timeout: Option<u64>,
    background: Option<bool>,
    intent: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxExecuteFileParams {
    path: String,
    language: String,
    code: String,
    timeout: Option<u64>,
    intent: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxBatchExecuteParams {
    commands: Vec<CtxBatchCommand>,
    queries: Option<Vec<String>>,
    concurrency: Option<u64>,
    timeout: Option<u64>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxBatchCommand {
    label: String,
    command: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxIndexParams {
    content: Option<String>,
    path: Option<String>,
    source: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxFetchAndIndexParams {
    url: String,
    timeout: Option<u64>,
    source: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxSearchParams {
    queries: Vec<String>,
    limit: Option<usize>,
    source: Option<String>,
    content_type: Option<String>,
    sort: Option<String>,
    token_budget: Option<usize>,
    min_confidence: Option<f32>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxVaultIndexParams {
    path: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxVaultGraphParams {
    mode: String,
    node_path: Option<String>,
    max_hops: Option<usize>,
    tag: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxSemanticSearchParams {
    query: String,
    limit: Option<usize>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxIndexEmbeddingsParams {
    content: String,
    source: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxContextPackParams {
    query: String,
    token_budget: Option<usize>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxConnectorAddParams {
    name: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxLocalIndexParams {
    path: String,
    repo_id: Option<String>,
    fresh: Option<bool>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxLocalSearchParams {
    query: String,
    repo_id: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxLocalStatusParams {
    job_id: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxLocalReposParams {}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxLocalWatchParams {
    path: String,
    repo_id: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxLocalUnwatchParams {
    repo_id: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxDeadCodeParams {
    path: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxComplexityParams {
    path: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxDepGraphParams {
    paths: Option<Vec<String>>,
    path: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
struct CtxGraphAnalyzeParams {
    path: String,
    god_node_limit: Option<usize>,
    surprise_limit: Option<usize>,
    community_limit: Option<usize>,
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
        env!("CARGO_PKG_VERSION").to_string()
    }

    #[tool(description = "Purge the context-mode knowledge base and reset session stats.")]
    async fn ctx_purge(&self) -> String {
        call_tool(context_mode_tools::stats::ctx_purge).await
    }

    #[tool(description = "Return the context-mode insight dashboard URL.")]
    async fn ctx_insight(&self) -> String {
        format_tool_result(json!({
            "url": "http://127.0.0.1:3030",
            "running": false,
            "note": "Insight dashboard not yet started. Run the insight server binary manually."
        }))
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

    #[tool(description = "Analyze vault graph: detect god nodes, surprising connections, communities, and suggest questions.")]
    async fn ctx_graph_analyze(
        &self,
        Parameters(params): Parameters<CtxGraphAnalyzeParams>,
    ) -> String {
        let mut value = params_to_value(&params);
        if let serde_json::Value::Object(ref mut object) = value {
            object.insert("path".to_string(), serde_json::Value::String(params.path));
        }
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
    async fn ctx_local_index(
        &self,
        Parameters(params): Parameters<CtxLocalIndexParams>,
    ) -> String {
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
    async fn ctx_local_watch(
        &self,
        Parameters(params): Parameters<CtxLocalWatchParams>,
    ) -> String {
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
    tokio::signal::ctrl_c().await.expect("failed to listen for ctrl+c event");
}
