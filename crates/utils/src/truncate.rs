/// Truncate text to a maximum character length, adding ellipsis if truncated.
pub fn truncate(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        let mut result = text
            .chars()
            .take(max_len.saturating_sub(3))
            .collect::<String>();
        result.push_str("...");
        result
    }
}

/// Truncate by token count (approximate via whitespace split).
pub fn truncate_tokens(text: &str, max_tokens: usize) -> String {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.len() <= max_tokens {
        text.to_string()
    } else {
        tokens
            .into_iter()
            .take(max_tokens)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_no_change() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_with_ellipsis() {
        assert_eq!(truncate("hello world this is long", 10), "hello w...");
    }

    #[test]
    fn test_truncate_tokens() {
        assert_eq!(truncate_tokens("one two three four", 2), "one two");
    }
}
