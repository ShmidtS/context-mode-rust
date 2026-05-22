use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Connector {
    name: String,
    enabled: bool,
    #[serde(rename = "type")]
    connector_type: String,
}

pub fn register_connector_tools() -> Result<()> {
    Ok(())
}

pub async fn ctx_connector_list() -> Result<Value> {
    match load_registry() {
        Ok(connectors) => Ok(serde_json::json!({
            "content": [{ "type": "text", "text": format!("{} connector(s) configured\n{}", connectors.len(), serde_json::to_string_pretty(&connectors).unwrap_or_default()) }],
            "isError": false
        })),
        Err(err) => Ok(error_response(format!(
            "Failed to read connector registry: {err}"
        ))),
    }
}

pub async fn ctx_connector_add(params: Value) -> Result<Value> {
    let Some(name) = params.get("name").and_then(|v| v.as_str()).map(str::trim) else {
        return Ok(error_response("Missing connector name"));
    };
    if name.is_empty() {
        return Ok(error_response("Missing connector name"));
    }

    let mut connectors = match load_registry() {
        Ok(connectors) => connectors,
        Err(err) => {
            return Ok(error_response(format!(
                "Failed to read connector registry: {err}"
            )));
        }
    };

    let connector = Connector {
        name: name.to_string(),
        enabled: true,
        connector_type: "generic".to_string(),
    };

    if let Some(existing) = connectors.iter_mut().find(|item| item.name == name) {
        *existing = connector.clone();
    } else {
        connectors.push(connector.clone());
    }

    match save_registry(&connectors) {
        Ok(()) => Ok(serde_json::json!({
            "content": [{ "type": "text", "text": format!("Connector '{}' added", name) }],
            "isError": false
        })),
        Err(err) => Ok(error_response(format!(
            "Failed to save connector registry: {err}"
        ))),
    }
}

pub async fn ctx_connector_sync(params: Value) -> Result<Value> {
    let Some(name) = params.get("name").and_then(|v| v.as_str()).map(str::trim) else {
        return Ok(error_response("Missing connector name"));
    };
    if name.is_empty() {
        return Ok(error_response("Missing connector name"));
    }

    let connectors = match load_registry() {
        Ok(connectors) => connectors,
        Err(err) => {
            return Ok(error_response(format!(
                "Failed to read connector registry: {err}"
            )));
        }
    };

    if !connectors.iter().any(|connector| connector.name == name) {
        return Ok(error_response(format!("Connector '{name}' not found")));
    }

    Ok(serde_json::json!({
        "content": [{ "type": "text", "text": format!("Connector '{}' synced", name) }],
        "isError": false
    }))
}

fn registry_path() -> Result<PathBuf> {
    let base = dirs::home_dir()
        .or_else(dirs::config_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(base.join(".context-mode").join("connectors.json"))
}

fn load_registry() -> Result<Vec<Connector>> {
    let path = registry_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let data = std::fs::read_to_string(path)?;
    if data.trim().is_empty() {
        return Ok(Vec::new());
    }

    Ok(serde_json::from_str(&data)?)
}

fn save_registry(connectors: &[Connector]) -> Result<()> {
    let path = registry_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(connectors)?)?;
    Ok(())
}

fn error_response(message: impl Into<String>) -> Value {
    serde_json::json!({
        "content": [{ "type": "text", "text": message.into() }],
        "isError": true
    })
}
