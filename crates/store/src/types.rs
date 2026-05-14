use serde::{Deserialize, Serialize};

pub use context_mode_core::{
    AstChunk, ContentType, IndexResult, MatchLayer, SearchResult, StoreStats,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Chunk {
    pub title: String,
    pub content: String,
    pub has_code: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceMatchMode {
    Like,
    Exact,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchRow {
    pub title: String,
    pub content: String,
    pub content_type: String,
    pub timestamp: Option<String>,
    pub label: String,
    pub rank: f64,
    pub highlighted: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceMeta {
    pub label: String,
    pub chunk_count: usize,
    pub code_chunk_count: usize,
    pub indexed_at: String,
    pub file_path: Option<String>,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceListItem {
    pub label: String,
    pub chunk_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexOptions {
    pub content: Option<String>,
    pub path: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SearchMode {
    And,
    Or,
}

impl SearchMode {
    pub fn as_fts_joiner(self) -> &'static str {
        match self {
            Self::And => " ",
            Self::Or => " OR ",
        }
    }
}

pub const STOPWORDS: &[&str] = &[
    "the", "and", "for", "are", "but", "not", "you", "all", "can", "had", "her", "was", "one",
    "our", "out", "has", "his", "how", "its", "may", "new", "now", "old", "see", "way", "who",
    "did", "get", "got", "let", "say", "she", "too", "use", "will", "with", "this", "that", "from",
    "they", "been", "have", "many", "some", "them", "than", "each", "make", "like", "just", "over",
    "such", "take", "into", "year", "your", "good", "could", "would", "about", "which", "their",
    "there", "other", "after", "should", "through", "also", "more", "most", "only", "very", "when",
    "what", "then", "these", "those", "being", "does", "done", "both", "same", "still", "while",
    "where", "here", "were", "much", "update", "updates", "updated", "deps", "dev", "tests",
    "test", "add", "added", "fix", "fixed", "run", "running", "using",
];

pub const MAX_CHUNK_BYTES: usize = 4096;

pub const FTS5_COLUMNS: &str = "
  title,
  content,
  source_id UNINDEXED,
  content_type UNINDEXED,
  source_category UNINDEXED,
  session_id UNINDEXED,
  event_id UNINDEXED,
  timestamp UNINDEXED";

pub fn is_stopword(word: &str) -> bool {
    STOPWORDS.contains(&word)
}
