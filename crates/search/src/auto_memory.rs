use context_mode_core::types::SearchResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutoMemoryResult {
    pub title: String,
    pub content: String,
    pub source: String,
    pub origin: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryPattern {
    pub term: String,
    pub count: usize,
    pub sources: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutoMemoryExtraction {
    pub summary: String,
    pub patterns: Vec<MemoryPattern>,
    pub source_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutoMemoryOptions {
    pub max_patterns: usize,
    pub min_term_len: usize,
}

impl Default for AutoMemoryOptions {
    fn default() -> Self {
        Self {
            max_patterns: 10,
            min_term_len: 3,
        }
    }
}

pub fn extract_auto_memory(results: &[SearchResult]) -> AutoMemoryExtraction {
    extract_auto_memory_with_options(results, AutoMemoryOptions::default())
}

pub fn extract_auto_memory_with_options(
    results: &[SearchResult],
    options: AutoMemoryOptions,
) -> AutoMemoryExtraction {
    let mut terms: HashMap<String, (usize, Vec<String>)> = HashMap::new();

    for result in results {
        for term in extract_terms(&result.content, options.min_term_len) {
            let entry = terms.entry(term).or_insert_with(|| (0, Vec::new()));
            entry.0 += 1;
            if !entry.1.contains(&result.source) {
                entry.1.push(result.source.clone());
            }
        }
    }

    let source_count = results.len().max(1);
    let mut patterns: Vec<_> = terms
        .into_iter()
        .filter(|(_, (count, _))| *count > 1)
        .map(|(term, (count, sources))| {
            let confidence = (sources.len() as f32 / source_count as f32).min(1.0);
            MemoryPattern {
                term,
                count,
                sources,
                confidence,
            }
        })
        .collect();

    patterns.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| b.confidence.total_cmp(&a.confidence))
            .then_with(|| a.term.cmp(&b.term))
    });
    patterns.truncate(options.max_patterns);

    let summary = if patterns.is_empty() {
        "No recurring memory patterns found".to_string()
    } else {
        let terms = patterns
            .iter()
            .take(5)
            .map(|pattern| pattern.term.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!("Recurring memory patterns: {terms}")
    };

    AutoMemoryExtraction {
        summary,
        patterns,
        source_count: results.len(),
    }
}

pub fn results_to_auto_memory(results: &[SearchResult], limit: usize) -> Vec<AutoMemoryResult> {
    results
        .iter()
        .take(limit)
        .map(|result| AutoMemoryResult {
            title: format!("[auto-memory] {}", result.title),
            content: result.content.clone(),
            source: result.source.clone(),
            origin: "auto-memory".to_string(),
            timestamp: result.timestamp.clone(),
        })
        .collect()
}

fn extract_terms(content: &str, min_len: usize) -> Vec<String> {
    content
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter_map(|raw| {
            let term = raw.trim().to_lowercase();
            (term.len() >= min_len && !is_stop_word(&term)).then_some(term)
        })
        .collect()
}

fn is_stop_word(term: &str) -> bool {
    matches!(
        term,
        "the"
            | "and"
            | "for"
            | "with"
            | "that"
            | "this"
            | "from"
            | "into"
            | "are"
            | "was"
            | "were"
            | "you"
            | "your"
            | "but"
            | "not"
            | "has"
            | "have"
            | "had"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_mode_core::types::{ContentType, SearchResult};

    fn result(source: &str, content: &str) -> SearchResult {
        SearchResult {
            title: source.into(),
            content: content.into(),
            source: source.into(),
            rank: 1.0,
            content_type: ContentType::Prose,
            match_layer: None,
            highlighted: None,
            timestamp: None,
            confidence: None,
            confidence_source: None,
        }
    }

    #[test]
    fn extracts_repeated_terms_as_patterns() {
        let extraction = extract_auto_memory(&[
            result("a", "rust search memory"),
            result("b", "rust vector memory"),
        ]);

        assert!(
            extraction
                .patterns
                .iter()
                .any(|pattern| pattern.term == "rust")
        );
        assert!(
            extraction
                .patterns
                .iter()
                .any(|pattern| pattern.term == "memory")
        );
    }
}
