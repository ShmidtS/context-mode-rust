use anyhow::Result;
use context_mode_core::db_schema;
use context_mode_core::local_indexer::LocalIndexer;
use context_mode_core::local_searcher::LocalSearcher;
use context_mode_core::watch_manager;
use context_mode_store::invalidate;
use serde_json::{Value, json};

fn derive_repo_id(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();
    parts.last().unwrap_or(&"repo").to_string()
}

pub async fn ctx_local_index(params: Value) -> Result<Value> {
    let path = params["path"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'path' parameter"))?;

    let repo_id = params["repo_id"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| derive_repo_id(path));

    let fresh = params["fresh"].as_bool().unwrap_or(false);

    let resolved = std::path::Path::new(path);
    if !resolved.exists() || !resolved.is_dir() {
        return Ok(json!({
            "content": [{ "type": "text", "text": format!("Directory not found: {}", path) }],
            "isError": true
        }));
    }

    let mut indexer = LocalIndexer::open(None)?;

    match indexer.index_repository(resolved, &repo_id, fresh) {
        Ok(report) => Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Indexed {} files ({} chunks) for repo \"{}\".\nJob ID: {}\nStatus: {}",
                    report.files_indexed, report.chunks_indexed, repo_id, report.id, report.status
                )
            }],
            "isError": false
        })),
        Err(e) => Ok(json!({
            "content": [{ "type": "text", "text": format!("Index failed: {}", e) }],
            "isError": true
        })),
    }
}

pub async fn ctx_local_search(params: Value) -> Result<Value> {
    let query = params["query"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'query' parameter"))?;
    let repo_id = params["repo_id"].as_str();
    let limit = params["limit"].as_u64().unwrap_or(10) as usize;

    let searcher = LocalSearcher::open(None)?;

    match searcher.search(query, repo_id, limit) {
        Ok(results) if results.is_empty() => Ok(json!({
            "content": [{ "type": "text", "text": "No results found.\nTip: run ctx_local_index first." }],
            "isError": false
        })),
        Ok(results) => {
            let text = results
                .iter()
                .map(|r| {
                    format!(
                        "[{}] {} (lines {}-{})\n{}",
                        r.source, r.title, 0, 0, r.content
                    )
                })
                .collect::<Vec<_>>()
                .join("\n---\n");
            Ok(json!({
                "content": [{ "type": "text", "text": text }],
                "isError": false
            }))
        }
        Err(e) => Ok(json!({
            "content": [{ "type": "text", "text": format!("Search failed: {}", e) }],
            "isError": true
        })),
    }
}

pub async fn ctx_local_status(params: Value) -> Result<Value> {
    let job_id = params["job_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'job_id' parameter"))?;

    let indexer = LocalIndexer::open(None)?;

    match db_schema::get_job(indexer.conn(), job_id)? {
        None => Ok(json!({
            "content": [{ "type": "text", "text": format!("Job {} not found.", job_id) }],
            "isError": true
        })),
        Some(job) => Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Job {}\n  status: {}\n  files: {}\n  chunks: {}\n  error: {}",
                    job.id,
                    job.status,
                    job.nodes_indexed.unwrap_or(0),
                    job.edges_indexed.unwrap_or(0),
                    job.error.unwrap_or_else(|| "none".to_string())
                )
            }],
            "isError": false
        })),
    }
}

pub async fn ctx_local_repos(_params: Value) -> Result<Value> {
    let indexer = LocalIndexer::open(None)?;

    let repos = db_schema::list_repos(indexer.conn())?;

    if repos.is_empty() {
        return Ok(json!({
            "content": [{ "type": "text", "text": "No repositories indexed yet.\nRun ctx_local_index to get started." }],
            "isError": false
        }));
    }

    let text = repos
        .iter()
        .map(|(id, count)| format!("  {}: {} files", id, count))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(json!({
        "content": [{ "type": "text", "text": format!("Indexed repositories:\n{}", text) }],
        "isError": false
    }))
}

pub async fn ctx_local_watch(params: Value) -> Result<Value> {
    let path = params["path"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'path' parameter"))?;
    let repo_id = params["repo_id"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| derive_repo_id(path));

    let resolved = std::path::Path::new(path);
    if !resolved.exists() || !resolved.is_dir() {
        return Ok(json!({
            "content": [{ "type": "text", "text": format!("Directory not found: {}", path) }],
            "isError": true
        }));
    }

    match watch_manager::start_watching_with_invalidator(resolved, &repo_id, |path| {
        context_mode_store::invalidate_blocking(&path.to_string_lossy());
    }) {
        Ok(()) => Ok(json!({
            "content": [{ "type": "text", "text": format!("Watching {} as \"{}\".", path, repo_id) }],
            "isError": false
        })),
        Err(e) => Ok(json!({
            "content": [{ "type": "text", "text": format!("Watch failed: {}", e) }],
            "isError": true
        })),
    }
}

pub async fn ctx_local_unwatch(params: Value) -> Result<Value> {
    let repo_id = params["repo_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'repo_id' parameter"))?;

    invalidate(repo_id).await;

    match watch_manager::stop_watching(repo_id) {
        Ok(()) => Ok(json!({
            "content": [{ "type": "text", "text": format!("Stopped watching \"{}\".", repo_id) }],
            "isError": false
        })),
        Err(e) => Ok(json!({
            "content": [{ "type": "text", "text": format!("Unwatch failed: {}", e) }],
            "isError": true
        })),
    }
}
