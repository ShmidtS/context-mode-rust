use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum VaultConfidence {
    #[default]
    EXTRACTED,
    INFERRED,
    AMBIGUOUS,
}

impl std::fmt::Display for VaultConfidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EXTRACTED => f.write_str("EXTRACTED"),
            Self::INFERRED => f.write_str("INFERRED"),
            Self::AMBIGUOUS => f.write_str("AMBIGUOUS"),
        }
    }
}

impl From<&str> for VaultConfidence {
    fn from(value: &str) -> Self {
        match value {
            "INFERRED" => Self::INFERRED,
            "AMBIGUOUS" => Self::AMBIGUOUS,
            _ => Self::EXTRACTED,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LinkType {
    Wikilink,
    Embed,
    Markdown,
    Import,
    Reference,
    External,
    Calls,
    Inherits,
    Implements,
    TypeRef,
    Decorates,
}

impl LinkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Wikilink => "wikilink",
            Self::Embed => "embed",
            Self::Markdown => "markdown",
            Self::Import => "import",
            Self::Reference => "reference",
            Self::External => "external",
            Self::Calls => "calls",
            Self::Inherits => "inherits",
            Self::Implements => "implements",
            Self::TypeRef => "type-ref",
            Self::Decorates => "decorates",
        }
    }
}

impl std::fmt::Display for LinkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for LinkType {
    fn from(value: &str) -> Self {
        match value {
            "embed" => Self::Embed,
            "markdown" => Self::Markdown,
            "import" => Self::Import,
            "reference" => Self::Reference,
            "external" => Self::External,
            "calls" => Self::Calls,
            "inherits" => Self::Inherits,
            "implements" => Self::Implements,
            "type-ref" => Self::TypeRef,
            "decorates" => Self::Decorates,
            _ => Self::Wikilink,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VaultNodeInput {
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub frontmatter: HashMap<String, Value>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub content_hash: String,
    pub mtime_ms: f64,
    #[serde(default)]
    pub in_degree: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VaultEdgeInput {
    pub source_path: String,
    pub target_path: Option<String>,
    pub link_type: LinkType,
    pub alias: Option<String>,
    pub target_name: Option<String>,
    pub context: String,
    pub line_number: i64,
    pub confidence: Option<VaultConfidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VaultNode {
    pub id: i64,
    pub vault_path: String,
    pub note_path: String,
    pub title: String,
    pub frontmatter: Option<String>,
    pub content_hash: String,
    pub file_mtime: f64,
    pub out_degree: i64,
    pub in_degree: i64,
    pub source_id: Option<i64>,
    pub indexed_at: String,
    pub source_type: String,
    pub connector_meta: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VaultEdge {
    pub id: i64,
    pub source_id: i64,
    pub target_id: Option<i64>,
    pub target_name: String,
    pub alias: Option<String>,
    pub line_number: Option<i64>,
    pub context: Option<String>,
    pub edge_type: String,
    pub confidence: VaultConfidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VaultTag {
    pub id: i64,
    pub tag: String,
    pub node_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VaultFrontmatterKey {
    pub id: i64,
    pub node_id: i64,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VaultConfig {
    pub vault_path: String,
    pub last_indexed_at: String,
    pub note_count: i64,
    pub edge_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiLink {
    pub target: String,
    pub alias: Option<String>,
    pub line_number: usize,
    pub context: String,
    pub link_type: LinkType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarkdownLink {
    pub text: String,
    pub target: String,
    pub line_number: usize,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParsedNote {
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub frontmatter: HashMap<String, Value>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub wiki_links: Vec<WikiLink>,
    #[serde(default)]
    pub markdown_links: Vec<MarkdownLink>,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImportKind {
    Static,
    Dynamic,
    Require,
    #[serde(rename = "export-from")]
    ExportFrom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportEntry {
    pub specifier: String,
    pub resolved_path: Option<String>,
    pub line_number: usize,
    pub context: String,
    pub kind: ImportKind,
    pub is_external: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParsedCodeFile {
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub imports: Vec<ImportEntry>,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodeSymbol {
    pub name: String,
    pub kind: String,
    pub scope: Option<String>,
    pub byte_start: usize,
    pub byte_end: usize,
    pub line_start: usize,
    pub line_end: usize,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SymbolEdge {
    pub source_symbol: String,
    pub target_symbol: String,
    pub edge_type: LinkType,
    pub confidence: VaultConfidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphSearchResult {
    pub id: i64,
    pub title: String,
    pub path: String,
    pub hop_distance: Option<usize>,
    pub backlink_count: i64,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub frontmatter: Option<HashMap<String, String>>,
    pub snippet: Option<String>,
    pub fusion_score: Option<f64>,
    pub page_rank: Option<f64>,
    pub text_rank: Option<usize>,
    pub match_layer: String,
    pub origin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextSearchResult {
    pub title: String,
    pub source: String,
    pub rank: f64,
}
