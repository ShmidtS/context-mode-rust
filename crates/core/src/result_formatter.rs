pub struct FormattedChunk {
    pub path: String,
    pub symbol: String,
    pub lines: String,
    pub content: String,
    pub score: f64,
}

pub struct SearchChunk {
    pub file_path: String,
    pub symbol_name: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
    pub score: f64,
}

/// Format search chunks with token-aware truncation.
pub fn format_results(chunks: &[SearchChunk], max_tokens: usize) -> Vec<FormattedChunk> {
    chunks
        .iter()
        .map(|chunk| FormattedChunk {
            path: chunk.file_path.clone(),
            symbol: chunk
                .symbol_name
                .clone()
                .unwrap_or_else(|| "(anonymous)".to_string()),
            lines: format!("{}-{}", chunk.start_line, chunk.end_line),
            content: truncate_content(&chunk.content, max_tokens),
            score: chunk.score,
        })
        .collect()
}

fn truncate_content(content: &str, max_tokens: usize) -> String {
    let truncated = context_mode_utils::truncate::truncate_tokens(content, max_tokens);
    if truncated == content {
        content.to_string()
    } else {
        format!("{}\n... (truncated)", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_results_basic() {
        let chunks = vec![SearchChunk {
            file_path: "src/lib.rs".to_string(),
            symbol_name: Some("run".to_string()),
            start_line: 10,
            end_line: 15,
            content: "fn run() {}".to_string(),
            score: 0.95,
        }];

        let formatted = format_results(&chunks, 100);

        assert_eq!(formatted.len(), 1);
        assert_eq!(formatted[0].path, "src/lib.rs");
        assert_eq!(formatted[0].symbol, "run");
        assert_eq!(formatted[0].lines, "10-15");
        assert_eq!(formatted[0].content, "fn run() {}");
        assert_eq!(formatted[0].score, 0.95);
    }

    #[test]
    fn test_format_results_truncation() {
        let chunks = vec![SearchChunk {
            file_path: "src/lib.rs".to_string(),
            symbol_name: Some("long".to_string()),
            start_line: 1,
            end_line: 2,
            content: "one two three four five".to_string(),
            score: 0.5,
        }];

        let formatted = format_results(&chunks, 3);

        assert_eq!(formatted[0].content, "one two three\n... (truncated)");
    }

    #[test]
    fn test_format_results_anonymous_symbol() {
        let chunks = vec![SearchChunk {
            file_path: "src/lib.rs".to_string(),
            symbol_name: None,
            start_line: 1,
            end_line: 1,
            content: "let value = 1;".to_string(),
            score: 0.1,
        }];

        let formatted = format_results(&chunks, 100);

        assert_eq!(formatted[0].symbol, "(anonymous)");
    }
}
