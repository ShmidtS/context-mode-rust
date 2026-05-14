use anyhow::{Result, anyhow};
use context_mode_vault::{IndexOpts, VaultGraphSearch, VaultGraphStore, index_vault};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn text_response(text: impl Into<String>) -> Value {
    json!({
        "content": [{ "type": "text", "text": text.into() }],
        "isError": false
    })
}

fn graph_db_path() -> PathBuf {
    PathBuf::from(".context-mode").join("vault-graph.db")
}

fn vault_path_from_params(params: &Value) -> Option<String> {
    params
        .get("path")
        .or_else(|| params.get("vaultPath"))
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned)
}

fn open_store(vault_path: &str) -> Result<VaultGraphStore> {
    std::fs::create_dir_all(".context-mode")?;
    Ok(VaultGraphStore::open(graph_db_path(), vault_path)?)
}

fn format_graph_results(results: &[context_mode_vault::GraphSearchResult]) -> String {
    if results.is_empty() {
        return "No graph results found.".to_string();
    }

    results
        .iter()
        .map(|result| {
            let hop = result
                .hop_distance
                .map(|distance| format!(" hop={distance}"))
                .unwrap_or_default();
            format!(
                "- {} ({}){} [{}]",
                result.title, result.path, hop, result.match_layer
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub async fn ctx_vault_index(params: Value) -> Result<Value> {
    let path = vault_path_from_params(&params).ok_or_else(|| anyhow!("missing vault path"))?;
    let store = open_store(&path)?;
    let result = index_vault(Path::new(&path), &store, Some(IndexOpts::default()))?;

    Ok(text_response(format!(
        "Indexed vault at: {path}\nIndexed: {}, Updated: {}, Skipped: {}, Broken links: {}",
        result.indexed, result.updated, result.skipped, result.broken_links
    )))
}

pub async fn ctx_vault_graph(params: Value) -> Result<Value> {
    let mode = params
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("neighbors");
    let vault_path = vault_path_from_params(&params).unwrap_or_default();
    let node_path = params
        .get("nodePath")
        .or_else(|| params.get("path"))
        .and_then(|v| v.as_str());
    let tag = params.get("tag").and_then(|v| v.as_str());
    let max_hops = params
        .get("maxHops")
        .or_else(|| params.get("max_hops"))
        .and_then(|v| v.as_u64())
        .unwrap_or(2) as usize;
    let edge_type = params.get("edgeType").and_then(|v| v.as_str());

    let store = open_store(&vault_path)?;
    let search = VaultGraphSearch::new(&store);

    let results = match mode {
        "neighbors" => {
            let node_path =
                node_path.ok_or_else(|| anyhow!("nodePath is required for neighbors mode"))?;
            let node = store
                .get_node_by_note_path(node_path)?
                .or_else(|| store.get_node_by_title(node_path).ok().flatten())
                .ok_or_else(|| anyhow!("node not found: {node_path}"))?;
            search.neighbors(node.id, max_hops, edge_type)?
        }
        "backlinks" => {
            let node_path =
                node_path.ok_or_else(|| anyhow!("nodePath is required for backlinks mode"))?;
            let node = store
                .get_node_by_note_path(node_path)?
                .or_else(|| store.get_node_by_title(node_path).ok().flatten())
                .ok_or_else(|| anyhow!("node not found: {node_path}"))?;
            search.backlinks(node.id, edge_type)?
        }
        "tag-cluster" | "tag_cluster" => {
            let tag = tag.ok_or_else(|| anyhow!("tag is required for tag-cluster mode"))?;
            search.tag_cluster(tag)?
        }
        other => return Err(anyhow!("unsupported vault graph mode: {other}")),
    };

    Ok(text_response(format_graph_results(&results)))
}
