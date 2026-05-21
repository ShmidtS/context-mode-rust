use crate::query::split_query_identifiers;
use context_mode_core::SearchResult;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

pub const EXACT_IDENTIFIER_BOOST: f64 = 1.25;
pub const PATH_MATCH_BOOST: f64 = 1.15;
pub const TEST_FILE_PENALTY: f64 = 0.85;
pub const GENERATED_FILE_PENALTY: f64 = 0.70;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RankedSearchResult {
    pub result: SearchResult,
    pub score: f64,
}

pub fn rank_results(query: &str, results: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut ranked = results
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            let base_score = if result.rank.is_sign_negative() {
                -result.rank
            } else {
                1.0 / (index as f64 + 1.0)
            };
            let score = apply_rank_modifiers(query, &result, base_score);
            RankedSearchResult { result, score }
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|a, b| b.score.total_cmp(&a.score));
    deduplicate_file_chunks(ranked)
        .into_iter()
        .map(|item| item.result)
        .collect()
}

pub fn apply_rank_modifiers(query: &str, result: &SearchResult, base_score: f64) -> f64 {
    let mut score = base_score;
    if has_exact_identifier_match(query, &result.content)
        || has_exact_identifier_match(query, &result.title)
    {
        score *= EXACT_IDENTIFIER_BOOST;
    }
    if has_path_match(query, &result.source) || has_path_match(query, &result.title) {
        score *= PATH_MATCH_BOOST;
    }
    if is_test_file(&result.source) {
        score *= TEST_FILE_PENALTY;
    }
    if is_generated_file(&result.source) {
        score *= GENERATED_FILE_PENALTY;
    }
    score
}

pub fn deduplicate_file_chunks(results: Vec<RankedSearchResult>) -> Vec<RankedSearchResult> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for result in results {
        let key = file_key(&result.result.source);
        if seen.insert(key) {
            deduped.push(result);
        }
    }

    deduped
}

pub fn has_exact_identifier_match(query: &str, text: &str) -> bool {
    split_query_identifiers(query)
        .into_iter()
        .any(|term| contains_identifier(text, &term))
}

pub fn has_path_match(query: &str, path: &str) -> bool {
    let lower_path = path.to_ascii_lowercase();
    split_query_identifiers(query)
        .into_iter()
        .any(|term| lower_path.contains(&term))
}

pub fn is_test_file(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized.contains("/tests/") || normalized.ends_with("_test.rs")
}

pub fn is_generated_file(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized.contains("/generated/") || normalized.contains("/target/")
}

fn contains_identifier(text: &str, identifier: &str) -> bool {
    let text = text.to_ascii_lowercase();
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .any(|token| token == identifier)
}

fn file_key(source: &str) -> String {
    Path::new(source)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| source.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_mode_core::{ContentType, MatchLayer};

    fn result(source: &str, title: &str, content: &str, rank: f64) -> SearchResult {
        SearchResult {
            title: title.into(),
            content: content.into(),
            source: source.into(),
            rank,
            content_type: ContentType::Code,
            match_layer: Some(MatchLayer::Porter),
            highlighted: None,
            timestamp: None,
            confidence: None,
            confidence_source: None,
        }
    }

    #[test]
    fn boosts_exact_identifier_and_path_matches() {
        let item = result("src/foo.rs", "foo", "fn foo_bar() {}", -1.0);
        assert!(apply_rank_modifiers("foo_bar", &item, 1.0) > 1.0);
    }

    #[test]
    fn penalizes_tests_and_generated_files() {
        let test = result("crates/core/tests/foo_test.rs", "foo", "foo", -1.0);
        let generated = result("src/generated/foo.rs", "foo", "foo", -1.0);

        assert!(apply_rank_modifiers("missing", &test, 1.0) < 1.0);
        assert!(apply_rank_modifiers("missing", &generated, 1.0) < 1.0);
    }

    #[test]
    fn deduplicates_chunks_by_file() {
        let ranked = vec![
            RankedSearchResult {
                result: result("src/lib.rs", "a", "a", -1.0),
                score: 2.0,
            },
            RankedSearchResult {
                result: result("other/lib.rs", "b", "b", -2.0),
                score: 1.0,
            },
            RankedSearchResult {
                result: result("src/main.rs", "c", "c", -3.0),
                score: 0.5,
            },
        ];

        let deduped = deduplicate_file_chunks(ranked);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].result.source, "src/lib.rs");
    }
}
