use context_mode_core::types::SearchResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PackOptions {
    pub token_budget: usize,
    pub dedup: bool,
}

impl Default for PackOptions {
    fn default() -> Self {
        Self {
            token_budget: 8_000,
            dedup: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PackedItem {
    pub title: String,
    pub tokens: usize,
    pub rank: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PackResult {
    pub packed: String,
    pub tokens_used: usize,
    pub items: Vec<PackedItem>,
}

#[derive(Debug, Clone)]
pub struct ContextPacker {
    pub max_tokens: usize,
}

impl Default for ContextPacker {
    fn default() -> Self {
        Self::new(8_000)
    }
}

impl ContextPacker {
    pub fn new(max_tokens: usize) -> Self {
        Self { max_tokens }
    }

    pub fn estimate_tokens(&self, text: &str) -> usize {
        (text.chars().count() as f64 / 3.2).ceil() as usize
    }

    pub fn pack(
        &self,
        _query: &str,
        results: &[SearchResult],
        options: Option<PackOptions>,
    ) -> PackResult {
        let options = options.unwrap_or(PackOptions {
            token_budget: self.max_tokens,
            dedup: true,
        });

        let mut sorted = results.to_vec();
        sorted.sort_by(|a, b| a.rank.total_cmp(&b.rank));

        let mut packed = Vec::new();
        let mut items = Vec::new();
        let mut seen_contents: Vec<String> = Vec::new();
        let mut tokens_used = 0;

        for (index, result) in sorted.iter().enumerate() {
            let title = if result.title.is_empty() {
                "Untitled"
            } else {
                &result.title
            };
            let content = &result.content;
            let block = format!(
                "## {}. {} (score: {:.3})\n{}",
                index + 1,
                title,
                -result.rank,
                content
            );
            let item_tokens = self.estimate_tokens(&block);

            if tokens_used + item_tokens > options.token_budget {
                break;
            }

            if options.dedup && !content.is_empty() {
                let lower = content.to_lowercase();
                let is_duplicate = seen_contents.iter().any(|seen| {
                    if lower.len() <= 50 {
                        seen == &lower
                    } else {
                        longest_common_substring_len(&lower, seen) as f64 / lower.len() as f64 > 0.8
                    }
                });
                if is_duplicate {
                    continue;
                }
                seen_contents.push(lower);
            }

            packed.push(block);
            items.push(PackedItem {
                title: title.to_string(),
                tokens: item_tokens,
                rank: items.len() + 1,
            });
            tokens_used += item_tokens;
        }

        PackResult {
            packed: packed.join("\n\n"),
            tokens_used,
            items,
        }
    }
}

pub fn pack_search_results(results: &[SearchResult], token_budget: usize) -> PackResult {
    ContextPacker::new(token_budget).pack(
        "",
        results,
        Some(PackOptions {
            token_budget,
            dedup: true,
        }),
    )
}

fn longest_common_substring_len(a: &str, b: &str) -> usize {
    if a.is_empty() || b.is_empty() {
        return 0;
    }

    let shorter = if a.len() < b.len() { a } else { b };
    let longer = if a.len() < b.len() { b } else { a };
    let mut best = 0;

    for start in shorter.char_indices().map(|(index, _)| index) {
        for end in shorter[start..]
            .char_indices()
            .map(|(index, _)| start + index)
            .skip(1)
            .chain(std::iter::once(shorter.len()))
        {
            let len = end - start;
            if len > best && longer.contains(&shorter[start..end]) {
                best = len;
            }
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_mode_core::types::{ContentType, SearchResult};

    fn result(title: &str, content: &str, rank: f64) -> SearchResult {
        SearchResult {
            title: title.into(),
            content: content.into(),
            source: "src".into(),
            rank,
            content_type: ContentType::Prose,
            match_layer: None,
            highlighted: None,
            timestamp: None,
            confidence: None,
            confidence_source: None,
        }
    }

    #[test]
    fn pack_respects_budget_and_rank_order() {
        let packer = ContextPacker::new(20);
        let packed = packer.pack(
            "",
            &[result("b", "short", 2.0), result("a", "short", 1.0)],
            None,
        );

        assert_eq!(packed.items[0].title, "a");
        assert!(packed.tokens_used <= 20);
    }
}
