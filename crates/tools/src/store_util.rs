use anyhow::Result;
use context_mode_store::ContentStore;
use serde_json::{Value, json};
use std::path::Path;

pub fn open_store() -> Result<ContentStore> {
    let path = Path::new(".context-mode/store.db");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    ContentStore::open(path)
        .or_else(|_| ContentStore::in_memory())
        .map_err(Into::into)
}

pub fn open_existing_store() -> Result<ContentStore> {
    let path = Path::new(".context-mode/store.db");
    if !path.exists() {
        anyhow::bail!("content store db not found");
    }
    ContentStore::open(path).map_err(Into::into)
}

pub fn text_response(text: impl Into<String>) -> Value {
    json!({
        "content": [{ "type": "text", "text": text.into() }],
        "isError": false
    })
}
