use anyhow::Result;
use context_mode_core::local_searcher::LocalSearcher;
use context_mode_search::VectorStore;
use context_mode_store::{SearchMode, SearchResult, SourceMatchMode};
use serde_json::{Value, json};

const SEMANTIC_FALLBACK_NOTE: &str =
    "Semantic search requires embedding backend. Falling back to keyword search...";

fn text_response(text: impl Into<String>) -> Value {
    crate::store_util::text_response(text)
}

pub fn reset_context_stream() -> Result<()> {
    Ok(())
}

pub async fn ctx_semantic_search(params: Value) -> Result<Value> {
    let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
    let vector_store = VectorStore::new();

    let results = match crate::store_util::open_existing_store() {
        Ok(store) => store.search(
            query,
            limit,
            None,
            SearchMode::Or,
            None,
            SourceMatchMode::Like,
        )?,
        Err(_) => Vec::new(),
    };

    Ok(text_response(serde_json::to_string_pretty(&json!({
        "results": format_results(results),
        "note": SEMANTIC_FALLBACK_NOTE,
        "vector_records": vector_store.count(),
    }))?))
}

pub async fn ctx_index_embeddings(params: Value) -> Result<Value> {
    let content = params.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let source = params
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let mut store = crate::store_util::open_store()?;
    let indexed = store.index_plain_text(content, source, 20)?;

    Ok(text_response(serde_json::to_string_pretty(&json!({
        "indexed": indexed.total_chunks,
        "source": source,
        "chunks": indexed.total_chunks,
        "type": "embedding-fallback",
    }))?))
}

pub async fn ctx_context_pack(params: Value) -> Result<Value> {
    let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let token_budget = params
        .get("token_budget")
        .or_else(|| params.get("tokenBudget"))
        .and_then(|v| v.as_u64())
        .unwrap_or(4000) as usize;

    let mut results = match crate::store_util::open_existing_store() {
        Ok(store) => store.search(query, 50, None, SearchMode::Or, None, SourceMatchMode::Like)?,
        Err(_) => {
            let searcher = LocalSearcher::open(None)?;
            searcher.search(query, None, 50)?
        }
    };
    results.sort_by(|a, b| a.rank.total_cmp(&b.rank));

    let mut packed = Vec::new();
    let mut sources = Vec::new();
    let mut chars_used = 0usize;
    let char_budget = token_budget.saturating_mul(4);

    for result in results {
        let block = format!(
            "## {} ({})\n{}",
            result.title, result.source, result.content
        );
        let block_chars = block.chars().count();
        if chars_used + block_chars > char_budget {
            break;
        }

        chars_used += block_chars;
        sources.push(json!({
            "title": result.title,
            "source": result.source,
            "rank": result.rank,
        }));
        packed.push(block);
    }

    Ok(text_response(serde_json::to_string_pretty(&json!({
        "packed_text": packed.join("\n\n"),
        "sources": sources,
        "token_estimate": chars_used.div_ceil(4),
    }))?))
}

fn format_results(results: Vec<SearchResult>) -> Vec<Value> {
    results
        .into_iter()
        .map(|result| {
            json!({
                "title": result.title,
                "content": result.content,
                "source": result.source,
                "rank": result.rank,
                "content_type": result.content_type,
                "match_layer": result.match_layer,
                "highlighted": result.highlighted,
                "timestamp": result.timestamp,
                "confidence": result.confidence,
            })
        })
        .collect()
}
