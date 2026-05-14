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

#[tool_router(server_handler)]
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

    #[tool(description = "Fetch remote content and index it. Not yet implemented in Rust server.")]
    async fn ctx_fetch_and_index(&self, Parameters(_params): Parameters<CtxIndexParams>) -> String {
        "Not yet implemented".to_string()
    }

    #[tool(description = "Show context-mode session statistics.")]
    async fn ctx_stats(&self) -> String {
        call_tool(context_mode_tools::stats::ctx_stats).await
    }

    #[tool(description = "Run context-mode diagnostics.")]
    async fn ctx_doctor(&self) -> String {
        call_tool(context_mode_tools::stats::ctx_doctor).await
    }

    #[tool(description = "Show current context-mode server version.")]
    async fn ctx_upgrade(&self) -> String {
        "1.0.0".to_string()
    }

    #[tool(description = "Purge the context-mode knowledge base and reset session stats.")]
    async fn ctx_purge(&self) -> String {
        call_tool(context_mode_tools::stats::ctx_purge).await
    }

    #[tool(description = "Return the context-mode insight dashboard URL.")]
    async fn ctx_insight(&self) -> String {
        format_tool_result(json!({
            "content": [{ "type": "text", "text": "Insight dashboard URL: http://127.0.0.1:3030" }],
            "isError": false
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

    #[tool(description = "Find dead code. Not yet implemented in Rust server.")]
    async fn ctx_dead_code(&self) -> String {
        "Not yet implemented".to_string()
    }

    #[tool(description = "Analyze code complexity. Not yet implemented in Rust server.")]
    async fn ctx_complexity(&self) -> String {
        "Not yet implemented".to_string()
    }

    #[tool(description = "Build dependency graph. Not yet implemented in Rust server.")]
    async fn ctx_dep_graph(&self) -> String {
        "Not yet implemented".to_string()
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
}

pub async fn run_server() -> anyhow::Result<()> {
    let server = ContextModeServer;
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
