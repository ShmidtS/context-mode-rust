use anyhow::{Result, anyhow};
use context_mode_vault::{IndexOpts, VaultGraphSearch, VaultGraphStore, index_vault};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn text_response(text: impl Into<String>) -> Value {
    crate::store_util::text_response(text)
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

/// Try to find a node by matching the given path against stored relative note paths.
/// Users often pass absolute paths, but the DB stores paths relative to vault_path.
fn resolve_node(
    store: &context_mode_vault::VaultGraphStore,
    vault_path: &str,
    node_path: &str,
) -> Option<context_mode_vault::VaultNode> {
    // Exact match on stored relative path
    if let Some(node) = store.get_node_by_note_path(node_path).ok().flatten() {
        return Some(node);
    }
    // Title match
    if let Some(node) = store.get_node_by_title(node_path).ok().flatten() {
        return Some(node);
    }
    // Strip vault_path prefix to get relative path
    if !vault_path.is_empty() {
        let normalized_input = node_path.replace('\\', "/");
        let normalized_vault = vault_path.replace('\\', "/");
        let normalized_vault = normalized_vault.trim_end_matches('/');
        if let Some(relative) = normalized_input.strip_prefix(normalized_vault) {
            let relative = relative.trim_start_matches('/');
            if let Some(node) = store.get_node_by_note_path(relative).ok().flatten() {
                return Some(node);
            }
        }
    }
    // LIKE match on the basename (last path segment) — handles absolute paths
    let normalized = node_path.replace('\\', "/");
    let basename = normalized.rsplit('/').next().unwrap_or(&normalized);
    let pattern = format!("%{basename}");
    if let Some(node) = store.find_node_by_path_like(&pattern).ok().flatten() {
        return Some(node);
    }
    None
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
            let node = resolve_node(&store, &vault_path, node_path)
                .ok_or_else(|| anyhow!("node not found: {node_path}"))?;
            search.neighbors(node.id, max_hops, edge_type)?
        }
        "backlinks" => {
            let node_path =
                node_path.ok_or_else(|| anyhow!("nodePath is required for backlinks mode"))?;
            let node = resolve_node(&store, &vault_path, node_path)
                .ok_or_else(|| anyhow!("node not found: {node_path}"))?;
            search.backlinks(node.id, edge_type)?
        }
        "tag-cluster" | "tag_cluster" => {
            let tag = tag.ok_or_else(|| anyhow!("tag is required for tag-cluster mode"))?;
            search.tag_cluster(tag)?
        }
        "surprises" => {
            let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let analysis = context_mode_vault::analyze_graph(&store, None)?;
            let formatted = analysis
                .surprising_connections
                .iter()
                .take(limit)
                .map(|conn| {
                    let explanation = conn.explanation.as_deref().unwrap_or("(no explanation)");
                    format!(
                        "- {} → {} (type={}, score={:.2})\n  {}\n  Tags: {:?} ↔ {:?}",
                        conn.source_path,
                        conn.target_path,
                        conn.edge_type,
                        conn.unexpectedness,
                        explanation,
                        conn.source_tags,
                        conn.target_tags
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(text_response(if formatted.is_empty() {
                "No surprising connections found.".to_string()
            } else {
                format!(
                    "Found {} surprising connections:\n{}",
                    analysis.surprising_connections.len().min(limit),
                    formatted
                )
            }));
        }
        "confidence-filter" => {
            use context_mode_vault::VaultConfidence;
            let min_confidence = params
                .get("minConfidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5);
            let score_for = |c: &VaultConfidence| -> f64 {
                match c {
                    VaultConfidence::EXTRACTED => 0.9,
                    VaultConfidence::INFERRED => 0.5,
                    VaultConfidence::AMBIGUOUS => 0.2,
                }
            };
            let all_edges = store.get_all_edges()?;
            let filtered: Vec<_> = all_edges
                .iter()
                .filter(|e| score_for(&e.confidence) >= min_confidence)
                .collect();
            let formatted = filtered
                .iter()
                .take(50)
                .map(|edge| {
                    let source = store.get_node_by_id(edge.source_id).ok().flatten();
                    let target = edge
                        .target_id
                        .and_then(|id| store.get_node_by_id(id).ok().flatten());
                    format!(
                        "- {} → {} (type={}, confidence={})",
                        source.as_ref().map(|n| n.title.as_str()).unwrap_or("?"),
                        target.as_ref().map(|n| n.title.as_str()).unwrap_or("?"),
                        edge.edge_type,
                        edge.confidence,
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(text_response(if formatted.is_empty() {
                format!("No edges found with confidence >= {min_confidence}.")
            } else {
                format!(
                    "Found {} edges with confidence >= {}:\n{}",
                    filtered.len().min(50),
                    min_confidence,
                    formatted
                )
            }));
        }
        other => return Err(anyhow!("unsupported vault graph mode: {other}")),
    };

    Ok(text_response(format_graph_results(&results)))
}

pub async fn ctx_graph_analyze(params: Value) -> Result<Value> {
    let vault_path = vault_path_from_params(&params).unwrap_or_default();
    let opts = context_mode_vault::AnalyzeOpts {
        god_node_limit: params
            .get("godNodeLimit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize),
        surprise_limit: params
            .get("surpriseLimit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize),
        community_limit: params
            .get("communityLimit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize),
        question_limit: params
            .get("questionLimit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize),
    };

    let store = open_store(&vault_path)?;
    let result = context_mode_vault::analyze_graph(&store, Some(opts))?;

    Ok(text_response(format!(
        "{summary}\n\nGod nodes: {god}\nSurprising connections: {surprise}\nCommunity hints: {comm}\nSuggested questions: {q}",
        summary = result.summary,
        god = result.god_nodes.len(),
        surprise = result.surprising_connections.len(),
        comm = result.community_hints.len(),
        q = result.suggested_questions.len(),
    )))
}
