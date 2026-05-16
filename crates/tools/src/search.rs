use anyhow::Result;
use context_mode_core::ContentType;
use context_mode_store::{IndexOptions, SearchMode, SourceMatchMode};
use serde_json::json;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct IndexParams {
    pub content: Option<String>,
    pub path: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SearchParams {
    pub queries: Vec<String>,
    pub limit: Option<usize>,
    pub source: Option<String>,
    pub content_type: Option<String>,
    pub sort: Option<String>,
    pub token_budget: Option<usize>,
    pub min_confidence: Option<f32>,
}

pub async fn ctx_index(params: serde_json::Value) -> Result<serde_json::Value> {
    let params: IndexParams = serde_json::from_value(params)?;
    let mut store = crate::store_util::open_store()?;
    let label = params
        .source
        .clone()
        .or_else(|| params.path.clone())
        .unwrap_or_else(|| "content".to_string());
    let indexed = if let Some(path) = params.path {
        store.index(IndexOptions {
            path: Some(path),
            content: None,
            source: params.source,
        })?
    } else {
        store.index(IndexOptions {
            content: params.content,
            path: None,
            source: params.source,
        })?
    };

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!("Indexed {} sections from: {}", indexed.total_chunks, label),
        }],
        "isError": false,
    }))
}

pub async fn ctx_search(params: serde_json::Value) -> Result<serde_json::Value> {
    let params: SearchParams = serde_json::from_value(params)?;
    let store = crate::store_util::open_store()?;
    let limit = params.limit.unwrap_or(3);
    let content_type = parse_content_type(params.content_type.as_deref());
    let mut text = String::new();
    let mut tokens = 0usize;

    for query in &params.queries {
        text.push_str(&format!("## {query}\n"));
        let mut results = store.search(
            query,
            limit,
            params.source.as_deref(),
            SearchMode::Or,
            content_type.clone(),
            SourceMatchMode::Like,
        )?;

        if params.sort.as_deref() == Some("timeline") {
            results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        }

        for result in results {
            if let Some(min_confidence) = params.min_confidence {
                if result.confidence.unwrap_or(1.0) < min_confidence {
                    continue;
                }
            }

            let snippet = crate::snippet::extract_snippet(
                &result.content,
                query,
                1500,
                result.highlighted.as_deref(),
            );
            let entry = format!("### {} ({})\n{}\n\n", result.title, result.source, snippet);
            tokens += context_mode_search::estimate_tokens(&entry);
            if params.token_budget.is_some_and(|budget| tokens > budget) {
                break;
            }
            text.push_str(&entry);
        }
    }

    if text.trim().is_empty() {
        text = "No matching sections found.".to_string();
    }

    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false,
    }))
}

fn parse_content_type(content_type: Option<&str>) -> Option<ContentType> {
    match content_type.map(str::to_ascii_lowercase).as_deref() {
        Some("code") => Some(ContentType::Code),
        Some("prose") => Some(ContentType::Prose),
        _ => None,
    }
}
