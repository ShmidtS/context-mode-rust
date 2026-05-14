use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────
// Session event types
// ─────────────────────────────────────────────────────────

/// Tool call representation used during event extraction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub tool_name: String,
    #[serde(default)]
    pub tool_input: HashMap<String, serde_json::Value>,
    pub tool_response: Option<String>,
    #[serde(default)]
    pub is_error: bool,
}

/// User message representation used during event extraction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserMessage {
    pub content: String,
    pub timestamp: Option<String>,
}

/// Session event category.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventCategory {
    File,
    Rule,
    Cwd,
    Error,
    Git,
    Task,
    Plan,
    Env,
    Skill,
    Constraint,
    Subagent,
    Mcp,
    #[serde(rename = "mcp_tool_call")]
    McpToolCall,
    Decision,
    #[serde(rename = "agent-finding")]
    AgentFinding,
    #[serde(rename = "external-ref")]
    ExternalRef,
    #[serde(rename = "blocked-on")]
    BlockedOn,
    Data,
    #[serde(rename = "error-resolution")]
    ErrorResolution,
    #[serde(rename = "iteration-loop")]
    IterationLoop,
    Intent,
    Role,
    Prompt,
    #[serde(rename = "user-prompt")]
    UserPrompt,
    Openclaw,
    Pi,
    Tool,
    Config,
    Test,
    Compaction,
    #[serde(rename = "rejected-approach")]
    RejectedApproach,
    #[serde(rename = "session-resume")]
    SessionResume,
    Status,
    Deploy,
    Log,
    Latency,
    Permission,
}

/// Priority levels for SessionEvent records.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventPriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

/// Session event as stored in SessionDB.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub category: EventCategory,
    pub data: String,
    pub priority: EventPriority,
    pub data_hash: String,
    pub project_dir: Option<String>,
    pub attribution_source: Option<String>,
    #[serde(rename = "attribution_confidence")]
    pub attribution_confidence: Option<f32>,
}

// ─────────────────────────────────────────────────────────
// Execution result
// ─────────────────────────────────────────────────────────

/// Result returned by PolyglotExecutor after running a code snippet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub timed_out: bool,
    #[serde(default)]
    pub backgrounded: bool,
}

// ─────────────────────────────────────────────────────────
// Content store shared types
// ─────────────────────────────────────────────────────────

/// Result returned after indexing content into the knowledge base.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexResult {
    pub source_id: i64,
    pub label: String,
    pub total_chunks: usize,
    pub code_chunks: usize,
}

/// Match layer for search results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MatchLayer {
    Porter,
    Trigram,
    Fuzzy,
    Rrf,
    #[serde(rename = "rrf-fuzzy")]
    RrfFuzzy,
    #[serde(rename = "rrf-3way")]
    Rrf3way,
    Semantic,
    Hybrid,
}

/// Confidence source for search results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfidenceSource {
    EXTRACTED,
    INFERRED,
    AMBIGUOUS,
}

/// A single search result returned from FTS5 BM25-ranked lookup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchResult {
    pub title: String,
    pub content: String,
    pub source: String,
    pub rank: f64,
    pub content_type: ContentType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_layer: Option<MatchLayer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlighted: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_source: Option<ConfidenceSource>,
}

/// Content type for search results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    Code,
    Prose,
}

/// AST-derived content chunk metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AstChunk {
    pub title: String,
    pub content: String,
    pub content_type: ContentType,
    pub symbol_name: Option<String>,
    pub symbol_kind: Option<String>,
    pub byte_start: Option<usize>,
    pub byte_end: Option<usize>,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
    pub content_hash: Option<String>,
}

/// Aggregate statistics for a ContentStore instance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoreStats {
    pub sources: usize,
    pub chunks: usize,
    pub code_chunks: usize,
}

// ─────────────────────────────────────────────────────────
// Resume snapshot
// ─────────────────────────────────────────────────────────

/// Structured representation of a session resume snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResumeSnapshot {
    pub generated_at: String,
    pub summary: String,
    pub events: Vec<SessionEvent>,
}

// ─────────────────────────────────────────────────────────
// Vault graph types
// ─────────────────────────────────────────────────────────

/// A node representing an Obsidian note in the vault graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VaultNode {
    pub id: i64,
    pub vault_path: String,
    pub note_path: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontmatter: Option<String>,
    pub content_hash: String,
    pub file_mtime: f64,
    pub out_degree: i64,
    pub in_degree: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<i64>,
    pub indexed_at: String,
    pub source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_meta: Option<String>,
}

/// A directed edge between two vault nodes (e.g. wikilink).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VaultEdge {
    pub id: i64,
    pub source_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<i64>,
    pub target_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    pub edge_type: String,
    pub confidence: VaultConfidence,
}

/// Confidence level for vault edges.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VaultConfidence {
    EXTRACTED,
    INFERRED,
    AMBIGUOUS,
}

/// A tag associated with a vault node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VaultTag {
    pub id: i64,
    pub tag: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_category_serde() {
        let cat = EventCategory::McpToolCall;
        let json = serde_json::to_string(&cat).unwrap();
        assert_eq!(json, "\"mcp_tool_call\"");
        let de: EventCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(de, cat);
    }

    #[test]
    fn test_session_event_serde() {
        let event = SessionEvent {
            event_type: "tool_use".to_string(),
            category: EventCategory::Tool,
            data: "test data".to_string(),
            priority: EventPriority::High,
            data_hash: "abc123".to_string(),
            project_dir: Some("/project".to_string()),
            attribution_source: None,
            attribution_confidence: Some(0.95),
        };
        let json = serde_json::to_string(&event).unwrap();
        let de: SessionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(de.event_type, "tool_use");
        assert_eq!(de.priority, EventPriority::High);
    }

    #[test]
    fn test_exec_result_defaults() {
        let result = ExecResult {
            stdout: "out".to_string(),
            stderr: "err".to_string(),
            exit_code: 0,
            timed_out: false,
            backgrounded: false,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"exit_code\":0"));
    }

    #[test]
    fn test_search_result_match_layer() {
        let result = SearchResult {
            title: "test".to_string(),
            content: "content".to_string(),
            source: "src".to_string(),
            rank: 1.5,
            content_type: ContentType::Code,
            match_layer: Some(MatchLayer::Rrf3way),
            highlighted: None,
            timestamp: None,
            confidence: Some(0.8),
            confidence_source: Some(ConfidenceSource::EXTRACTED),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("rrf-3way"));
    }

    #[test]
    fn test_vault_edge_confidence() {
        let edge = VaultEdge {
            id: 1,
            source_id: 1,
            target_id: Some(2),
            target_name: "target".to_string(),
            alias: None,
            line_number: Some(42),
            context: None,
            edge_type: "wikilink".to_string(),
            confidence: VaultConfidence::EXTRACTED,
        };
        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("EXTRACTED"));
    }

    #[test]
    fn test_event_priority_ordering() {
        assert!(EventPriority::Critical > EventPriority::High);
        assert!(EventPriority::High > EventPriority::Normal);
        assert!(EventPriority::Normal > EventPriority::Low);
    }
}
