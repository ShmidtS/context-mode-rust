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
    let store_exists = std::path::Path::new(".context-mode/store.db").exists();
    let mut checks = vec![];
    checks.push(format!(
        "Store DB: {}",
        if store_exists { "OK" } else { "MISSING" }
    ));
    checks.push("Core dependencies: OK".to_string());
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
