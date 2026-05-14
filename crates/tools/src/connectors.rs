use anyhow::Result;

pub fn register_connector_tools() -> Result<()> {
    Ok(())
}

pub async fn ctx_connector_list() -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::json!({
        "content": [{ "type": "text", "text": "Available connectors: Context7, GitHub, Tavily, DDG Search, Playwright" }],
        "isError": false
    }))
}

pub async fn ctx_connector_add(params: serde_json::Value) -> anyhow::Result<serde_json::Value> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    Ok(serde_json::json!({
        "content": [{ "type": "text", "text": format!("Connector '{}' registration not yet implemented.", name) }],
        "isError": false
    }))
}

pub async fn ctx_connector_sync(_params: serde_json::Value) -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::json!({
        "content": [{ "type": "text", "text": "Connector sync not yet implemented." }],
        "isError": false
    }))
}
