use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryClassification {
    pub query: String,
    pub symbol_like: bool,
    pub terms: Vec<String>,
    pub alpha: f64,
}

pub fn classify_query(query: &str) -> QueryClassification {
    let symbol_like = is_symbol_like(query);
    QueryClassification {
        query: query.to_string(),
        symbol_like,
        terms: split_query_identifiers(query),
        alpha: if symbol_like { 0.3 } else { 0.7 },
    }
}

pub fn is_symbol_like(query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.contains("::") || trimmed.contains('.') || trimmed.contains('_') {
        return true;
    }

    let has_lower = trimmed.chars().any(|ch| ch.is_ascii_lowercase());
    let has_upper = trimmed.chars().any(|ch| ch.is_ascii_uppercase());
    has_lower && has_upper && trimmed.chars().all(|ch| ch.is_ascii_alphanumeric())
}

pub fn split_query_identifiers(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .flat_map(split_identifier)
        .filter(|term| !term.is_empty())
        .collect()
}

pub fn split_identifier(identifier: &str) -> Vec<String> {
    identifier
        .split('_')
        .flat_map(split_camel_case)
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

fn split_camel_case(identifier: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut start = 0;
    let chars = identifier.char_indices().collect::<Vec<_>>();

    for index in 1..chars.len() {
        let (_, current) = chars[index];
        let (_, previous) = chars[index - 1];
        let next = chars.get(index + 1).map(|(_, ch)| *ch);
        let boundary = (current.is_ascii_uppercase() && previous.is_ascii_lowercase())
            || (current.is_ascii_uppercase()
                && previous.is_ascii_uppercase()
                && next.is_some_and(|ch| ch.is_ascii_lowercase()));
        if boundary {
            parts.push(identifier[start..chars[index].0].to_string());
            start = chars[index].0;
        }
    }

    parts.push(identifier[start..].to_string());
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_symbol_queries() {
        assert!(is_symbol_like("foo_bar"));
        assert!(is_symbol_like("foo::bar"));
        assert!(is_symbol_like("foo.bar"));
        assert!(is_symbol_like("fooBar"));
        assert!(!is_symbol_like("natural language query"));
    }

    #[test]
    fn splits_identifiers() {
        assert_eq!(split_identifier("foo_bar_baz"), vec!["foo", "bar", "baz"]);
        assert_eq!(split_identifier("fooBarBaz"), vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn classifier_sets_alpha() {
        assert_eq!(classify_query("foo_bar").alpha, 0.3);
        assert_eq!(classify_query("how to search").alpha, 0.7);
    }
}
