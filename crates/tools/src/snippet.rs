pub fn extract_snippet(
    text: &str,
    query: &str,
    max_len: usize,
    highlighted: Option<&str>,
) -> String {
    let source = highlighted.unwrap_or(text);
    let marker_region = source.find('\x02').and_then(|start| {
        source[start + 1..]
            .find('\x03')
            .map(|end| (start, start + 1 + end))
    });

    let clean = source.replace(['\x02', '\x03'], "");
    if clean.is_empty() {
        return String::new();
    }

    let center = marker_region
        .map(|(start, end)| start + (end.saturating_sub(start) / 2))
        .or_else(|| find_query_match(&clean, query))
        .unwrap_or(0);

    window(&clean, center, max_len)
}

fn find_query_match(text: &str, query: &str) -> Option<usize> {
    let lower = text.to_lowercase();
    query
        .split_whitespace()
        .filter(|term| !term.is_empty())
        .filter_map(|term| lower.find(&term.to_lowercase()))
        .next()
}

fn window(text: &str, center: usize, max_len: usize) -> String {
    if max_len == 0 {
        return text.chars().next().map(String::from).unwrap_or_default();
    }

    let char_positions: Vec<usize> = text.char_indices().map(|(idx, _)| idx).collect();
    let char_count = char_positions.len();
    if char_count <= max_len {
        return text.to_string();
    }

    let center_char = char_positions.partition_point(|idx| *idx < center);
    let mut start_char = center_char.saturating_sub(max_len / 2);
    if start_char + max_len > char_count {
        start_char = char_count.saturating_sub(max_len);
    }
    let end_char = (start_char + max_len).min(char_count);

    let start_byte = char_positions[start_char];
    let end_byte = char_positions.get(end_char).copied().unwrap_or(text.len());
    let mut snippet = text[start_byte..end_byte].to_string();
    if start_char > 0 {
        snippet.insert_str(0, "...");
    }
    if end_char < char_count {
        snippet.push_str("...");
    }
    snippet
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_window_around_highlight_markers() {
        let text = "alpha beta \x02needle\x03 gamma delta epsilon";
        let snippet = extract_snippet(text, "needle", 20, Some(text));

        assert!(snippet.contains("needle"));
        assert!(!snippet.contains('\x02'));
        assert!(!snippet.contains('\x03'));
        assert!(snippet.starts_with("..."));
        assert!(snippet.ends_with("..."));
    }

    #[test]
    fn falls_back_to_non_empty_prefix_when_query_missing() {
        let snippet = extract_snippet("short text", "missing", 50, None);

        assert_eq!(snippet, "short text");
    }
}
