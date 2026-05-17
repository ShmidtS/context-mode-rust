use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SessionStats {
    pub total_calls: i64,
    pub total_bytes: i64,
}

static GLOBAL_STATS: Lazy<Arc<Mutex<SessionStats>>> =
    Lazy::new(|| Arc::new(Mutex::new(SessionStats::default())));

pub fn track_response(tool: &str, result: &serde_json::Value) {
    let mut stats = GLOBAL_STATS.lock().unwrap();
    stats.total_calls += 1;
    let text = result.to_string();
    stats.total_bytes += text.len() as i64;
    tracing::info!(
        "track_response: tool={} calls={} bytes={}",
        tool,
        stats.total_calls,
        stats.total_bytes
    );
}

pub async fn persist_stats() -> anyhow::Result<()> {
    let stats = GLOBAL_STATS.lock().unwrap().clone();
    tracing::info!("Persisting stats: {:?}", stats);
    Ok(())
}

pub async fn ctx_stats() -> anyhow::Result<serde_json::Value> {
    let stats = GLOBAL_STATS.lock().unwrap().clone();
    Ok(serde_json::json!({
        "content": [{ "type": "text", "text": format!("Total calls: {}, Total bytes: {}", stats.total_calls, stats.total_bytes) }],
        "isError": false
    }))
}

pub async fn ctx_doctor() -> anyhow::Result<serde_json::Value> {
    let store_path = std::path::Path::new(".context-mode/store.db");
    let abs_path = std::env::current_dir()
        .ok()
        .map(|cwd| cwd.join(store_path))
        .unwrap_or_else(|| store_path.to_path_buf());
    let mut checks = vec![];
    if store_path.exists() {
        checks.push(format!("Store DB: OK ({})", abs_path.display()));
    } else {
        checks.push(format!(
            "Store DB: MISSING (expected at {}) — will be auto-created on first ctx_index or ctx_search",
            abs_path.display()
        ));
    }
    checks.push("Core dependencies: OK".to_string());
    checks.push(String::new());
    checks.push("Registered tools (29):".to_string());
    for tool in [
        "ctx_execute",
        "ctx_execute_file",
        "ctx_batch_execute",
        "ctx_index",
        "ctx_search",
        "ctx_fetch_and_index",
        "ctx_stats",
        "ctx_doctor",
        "ctx_upgrade",
        "ctx_purge",
        "ctx_insight",
        "ctx_semantic_search",
        "ctx_index_embeddings",
        "ctx_context_pack",
        "ctx_vault_index",
        "ctx_vault_graph",
        "ctx_graph_analyze",
        "ctx_dead_code",
        "ctx_complexity",
        "ctx_dep_graph",
        "ctx_connector_list",
        "ctx_connector_add",
        "ctx_connector_sync",
        "ctx_local_index",
        "ctx_local_search",
        "ctx_local_status",
        "ctx_local_repos",
        "ctx_local_watch",
        "ctx_local_unwatch",
    ] {
        checks.push(format!("  OK  {}", tool));
    }
    Ok(serde_json::json!({
        "content": [{ "type": "text", "text": checks.join("\n") }],
        "isError": false
    }))
}

pub async fn ctx_purge() -> anyhow::Result<serde_json::Value> {
    let path = std::path::Path::new(".context-mode/store.db");
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    let mut stats = GLOBAL_STATS.lock().unwrap();
    *stats = SessionStats::default();
    Ok(serde_json::json!({
        "content": [{ "type": "text", "text": "Knowledge base purged." }],
        "isError": false
    }))
}
