use crate::rrf::{DEFAULT_RRF_K, reciprocal_rank_fuse};
use context_mode_core::types::{ConfidenceSource, ContentType, MatchLayer, SearchResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SearchOrigin {
    CurrentSession,
    PriorSession,
    AutoMemory,
    VaultGraph,
    Semantic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedSearchResult {
    pub title: String,
    pub content: String,
    pub source: String,
    pub origin: SearchOrigin,
    pub rank: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_layer: Option<MatchLayer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlighted: Option<String>,
    pub content_type: ContentType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_source: Option<ConfidenceSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedSearchOptions {
    pub limit: usize,
    pub rrf_k: f64,
}

impl Default for UnifiedSearchOptions {
    fn default() -> Self {
        Self {
            limit: 10,
            rrf_k: DEFAULT_RRF_K,
        }
    }
}

impl UnifiedSearchResult {
    pub fn key(&self) -> String {
        format!("{}::{}", self.source, self.title)
    }

    pub fn from_search_result(result: SearchResult, origin: SearchOrigin) -> Self {
        let estimated_tokens = Some(estimate_tokens(&result.content));
        Self {
            title: result.title,
            content: result.content,
            source: result.source,
            origin,
            rank: result.rank,
            timestamp: result.timestamp,
            match_layer: result.match_layer,
            highlighted: result.highlighted,
            content_type: result.content_type,
            estimated_tokens,
            confidence: result.confidence,
            confidence_source: result.confidence_source,
        }
    }
}

impl From<UnifiedSearchResult> for SearchResult {
    fn from(result: UnifiedSearchResult) -> Self {
        SearchResult {
            title: result.title,
            content: result.content,
            source: result.source,
            rank: result.rank,
            content_type: result.content_type,
            match_layer: result.match_layer,
            highlighted: result.highlighted,
            timestamp: result.timestamp,
            confidence: result.confidence,
            confidence_source: result.confidence_source,
        }
    }
}

pub fn fuse_search_results(
    fts_results: Vec<SearchResult>,
    vector_results: Vec<SearchResult>,
    options: UnifiedSearchOptions,
) -> Vec<SearchResult> {
    let fts = fts_results
        .into_iter()
        .map(|result| UnifiedSearchResult::from_search_result(result, SearchOrigin::CurrentSession))
        .collect::<Vec<_>>();
    let semantic = vector_results
        .into_iter()
        .map(|mut result| {
            if result.match_layer.is_none() {
                result.match_layer = Some(MatchLayer::Semantic);
            }
            UnifiedSearchResult::from_search_result(result, SearchOrigin::Semantic)
        })
        .collect::<Vec<_>>();

    let unified = fuse_unified_results(vec![fts, semantic], options);
    let mut result = Vec::with_capacity(unified.len());
    for item in unified {
        result.push(SearchResult::from(item));
    }
    result
}

pub fn fuse_unified_results(
    result_lists: Vec<Vec<UnifiedSearchResult>>,
    options: UnifiedSearchOptions,
) -> Vec<UnifiedSearchResult> {
    let fused = reciprocal_rank_fuse(&result_lists, UnifiedSearchResult::key, options.rrf_k);
    let mut result = Vec::with_capacity(fused.len().min(options.limit));
    for scored in fused {
        if result.len() >= options.limit {
            break;
        }
        let mut item = scored.item;
        item.rank = -scored.rrf_score;
        item.match_layer = Some(MatchLayer::Hybrid);
        result.push(item);
    }
    result
}

pub fn estimate_tokens(text: &str) -> usize {
    (text.len() as f64 / 4.0).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(title: &str, rank: f64) -> SearchResult {
        SearchResult {
            title: title.into(),
            content: title.into(),
            source: "src".into(),
            rank,
            content_type: ContentType::Code,
            match_layer: None,
            highlighted: None,
            timestamp: None,
            confidence: None,
            confidence_source: None,
        }
    }

    #[test]
    fn fuse_search_results_prefers_items_seen_in_both_lists() {
        let fused = fuse_search_results(
            vec![result("a", 1.0), result("b", 2.0)],
            vec![result("b", 1.0), result("c", 2.0)],
            UnifiedSearchOptions::default(),
        );

        assert_eq!(fused[0].title, "b");
        assert_eq!(fused[0].match_layer, Some(MatchLayer::Hybrid));
    }
}
