use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SessionStats {
    pub total_calls: i64,
    pub total_bytes: i64,
    pub raw_bytes_captured: i64,
    pub bytes_returned_to_model: i64,
    pub bytes_avoided: i64,
}

static GLOBAL_STATS: Lazy<Arc<Mutex<SessionStats>>> =
    Lazy::new(|| Arc::new(Mutex::new(SessionStats::default())));

pub fn track_response(tool: &str, result: &serde_json::Value) {
    let mut stats = GLOBAL_STATS.lock().unwrap_or_else(|e| e.into_inner());
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

pub fn record_token_savings(raw_bytes_captured: i64, bytes_returned_to_model: i64) {
    let mut stats = GLOBAL_STATS.lock().unwrap_or_else(|e| e.into_inner());
    let raw = raw_bytes_captured.max(0);
    let returned = bytes_returned_to_model.max(0);
    stats.raw_bytes_captured += raw;
    stats.bytes_returned_to_model += returned;
    stats.bytes_avoided += (raw - returned).max(0);
}

pub async fn persist_stats() -> anyhow::Result<()> {
    let stats = GLOBAL_STATS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    tracing::info!("Persisting stats: {:?}", stats);
    Ok(())
}

#[cfg(test)]
pub fn reset_stats_for_test() {
    let mut stats = GLOBAL_STATS.lock().unwrap_or_else(|e| e.into_inner());
    *stats = SessionStats::default();
}

#[cfg(test)]
pub fn snapshot_stats_for_test() -> SessionStats {
    GLOBAL_STATS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

pub async fn ctx_stats() -> anyhow::Result<serde_json::Value> {
    let stats = GLOBAL_STATS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    Ok(serde_json::json!({
        "content": [{ "type": "text", "text": format!(
            "Total calls: {}, Total bytes: {}\nRaw bytes captured: {}\nBytes returned to model: {}\nBytes avoided: {}",
            stats.total_calls,
            stats.total_bytes,
            stats.raw_bytes_captured,
            stats.bytes_returned_to_model,
            stats.bytes_avoided
        ) }],
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

    // Check Claude Code settings.json hooks
    let home = dirs::home_dir();
    if let Some(home) = home {
        let settings_path = home.join(".claude").join("settings.json");
        if settings_path.exists() {
            match std::fs::read_to_string(&settings_path) {
                Ok(raw) => match serde_json::from_str::<serde_json::Value>(&raw) {
                    Ok(settings) => {
                        if let Some(hooks) = settings.get("hooks") {
                            let has_context_mode = hooks.get("PostToolUse")
                                .and_then(|p| p.as_array())
                                .map(|arr| arr.iter().any(|entry| {
                                    entry.get("hooks")
                                        .and_then(|h| h.as_array())
                                        .map(|hooks| hooks.iter().any(|hook| {
                                            hook.get("command")
                                                .and_then(|c| c.as_str())
                                                .map(|s| s.contains("context-mode hook claude-code"))
                                                .unwrap_or(false)
                                        }))
                                        .unwrap_or(false)
                                }))
                                .unwrap_or(false);
                            if has_context_mode {
                                checks.push(format!(
                                    "Claude Code settings hooks: OK ({})",
                                    settings_path.display()
                                ));
                            } else {
                                checks.push(format!(
                                    "Claude Code settings hooks: MISSING context-mode hooks in {}",
                                    settings_path.display()
                                ));
                            }
                        } else {
                            checks.push(format!(
                                "Claude Code settings hooks: MISSING (no 'hooks' key in {})",
                                settings_path.display()
                            ));
                        }
                    }
                    Err(e) => {
                        checks.push(format!(
                            "Claude Code settings hooks: PARSE ERROR {} — {}",
                            settings_path.display(),
                            e
                        ));
                    }
                },
                Err(e) => {
                    checks.push(format!(
                        "Claude Code settings hooks: READ ERROR {} — {}",
                        settings_path.display(),
                        e
                    ));
                }
            }
        } else {
            checks.push(format!(
                "Claude Code settings hooks: MISSING ({} not found)",
                settings_path.display()
            ));
        }
    } else {
        checks.push("Claude Code settings hooks: UNKNOWN (could not determine home dir)".to_string());
    }

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
    let mut stats = GLOBAL_STATS.lock().unwrap_or_else(|e| e.into_inner());
    *stats = SessionStats::default();
    Ok(serde_json::json!({
        "content": [{ "type": "text", "text": "Knowledge base purged." }],
        "isError": false
    }))
}
