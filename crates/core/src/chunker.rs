use crate::types::ContentType;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub const SYMBOL_TYPES: &[(&str, &str)] = &[
    ("function", "function"),
    ("method", "method"),
    ("class", "class"),
    ("interface", "interface"),
    ("struct", "struct"),
    ("enum", "enum"),
    ("trait", "trait"),
    ("impl", "impl"),
    ("module", "module"),
    ("const", "const"),
    ("type", "type"),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Chunk {
    pub title: String,
    pub content: String,
    pub content_type: ContentType,
    pub metadata: ChunkMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ChunkMetadata {
    pub symbol_name: Option<String>,
    pub symbol_kind: Option<String>,
    pub byte_start: Option<usize>,
    pub byte_end: Option<usize>,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
    pub content_hash: Option<String>,
}

pub fn chunk_lines(text: &str, max_lines: usize) -> Vec<Chunk> {
    if text.is_empty() || max_lines == 0 {
        return Vec::new();
    }

    let lines: Vec<&str> = text.lines().collect();
    lines
        .chunks(max_lines)
        .enumerate()
        .map(|(index, lines)| {
            let content = lines.join("\n");
            let line_start = index * max_lines + 1;
            let line_end = line_start + lines.len() - 1;
            build_chunk(
                format!("Chunk {}", index + 1),
                content,
                Some(line_start),
                Some(line_end),
            )
        })
        .collect()
}

pub fn chunk_by_tokens(text: &str, max_tokens: usize) -> Vec<Chunk> {
    if text.trim().is_empty() || max_tokens == 0 {
        return Vec::new();
    }

    let tokens: Vec<&str> = text.split_whitespace().collect();
    tokens
        .chunks(max_tokens)
        .enumerate()
        .map(|(index, tokens)| {
            build_chunk(format!("Chunk {}", index + 1), tokens.join(" "), None, None)
        })
        .collect()
}

fn build_chunk(
    title: String,
    content: String,
    line_start: Option<usize>,
    line_end: Option<usize>,
) -> Chunk {
    let content_hash = hash_content(&content);
    Chunk {
        title,
        content,
        content_type: ContentType::Prose,
        metadata: ChunkMetadata {
            line_start,
            line_end,
            content_hash: Some(content_hash),
            ..ChunkMetadata::default()
        },
    }
}

fn hash_content(content: &str) -> String {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_lines_basic() {
        let chunks = chunk_lines("one\ntwo\nthree", 2);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].content, "one\ntwo");
        assert_eq!(chunks[0].metadata.line_start, Some(1));
        assert_eq!(chunks[0].metadata.line_end, Some(2));
        assert_eq!(chunks[1].content, "three");
        assert_eq!(chunks[1].metadata.line_start, Some(3));
        assert_eq!(chunks[1].metadata.line_end, Some(3));
    }

    #[test]
    fn test_chunk_by_tokens() {
        let chunks = chunk_by_tokens("one two three four five", 2);

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].content, "one two");
        assert_eq!(chunks[1].content, "three four");
        assert_eq!(chunks[2].content, "five");
    }

    #[test]
    fn test_empty_input() {
        assert!(chunk_lines("", 10).is_empty());
        assert!(chunk_by_tokens("", 10).is_empty());
    }
}
