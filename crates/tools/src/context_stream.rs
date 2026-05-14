use anyhow::Result;
use context_mode_search::VectorStore;
use serde_json::{Value, json};

fn text_response(text: impl Into<String>) -> Value {
    json!({
        "content": [{ "type": "text", "text": text.into() }],
        "isError": false
    })
}

pub fn reset_context_stream() -> Result<()> {
    Ok(())
}

pub async fn ctx_semantic_search(params: Value) -> Result<Value> {
    let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(5);
    let store = VectorStore::new();

    Ok(text_response(format!(
        "Semantic search parsed query='{query}' limit={limit}. Vector store records: {}. Embedding search not yet implemented.",
        store.count()
    )))
}

pub async fn ctx_index_embeddings(params: Value) -> Result<Value> {
    let content = params.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let source = params
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    Ok(text_response(format!(
        "Embedding indexing not yet implemented. Parsed source='{source}', content_bytes={}.",
        content.len()
    )))
}

pub async fn ctx_context_pack(params: Value) -> Result<Value> {
    let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let token_budget = params
        .get("token_budget")
        .or_else(|| params.get("tokenBudget"))
        .and_then(|v| v.as_u64())
        .unwrap_or(4000);

    Ok(text_response(format!(
        "Context pack parsed query='{query}' token_budget={token_budget}. Context packing not yet implemented."
    )))
}
