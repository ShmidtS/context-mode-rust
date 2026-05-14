use anyhow::Result;

pub fn handle_tool_call(_name: &str, _params: serde_json::Value) -> Result<serde_json::Value> {
    Ok(serde_json::Value::Null)
}
