use context_mode_core::types::EventCategory;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SessionError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub category: EventCategory,
    pub data: String,
    pub priority: i32,
    #[serde(default)]
    pub data_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribution_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribution_confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredEvent {
    pub id: i64,
    pub session_id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub category: EventCategory,
    pub priority: i32,
    pub data: String,
    pub project_dir: String,
    pub attribution_source: String,
    pub attribution_confidence: f64,
    pub source_hook: String,
    pub created_at: String,
    pub data_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMeta {
    pub session_id: String,
    pub project_dir: String,
    pub started_at: String,
    pub last_event_at: Option<String>,
    pub event_count: i64,
    pub compact_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResumeRow {
    pub id: i64,
    pub session_id: String,
    pub snapshot: String,
    pub event_count: i64,
    pub created_at: String,
    pub consumed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ToolCallRow {
    pub tool: String,
    pub calls: i64,
    pub bytes_returned: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ToolCallStats {
    pub total_calls: i64,
    pub total_bytes_returned: i64,
    pub by_tool: Vec<ToolCallRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct HookInput {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub transcript_path: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub hook_event_name: Option<String>,
    #[serde(default)]
    pub tool_name: String,
    #[serde(default)]
    pub tool_input: Value,
    #[serde(default)]
    pub tool_response: Value,
    #[serde(default)]
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildSnapshotOpts {
    pub compact_count: i64,
    pub search_tool: String,
}

impl Default for BuildSnapshotOpts {
    fn default() -> Self {
        Self {
            compact_count: 1,
            search_tool: "ctx_search".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RuntimeStats {
    pub session_id: String,
    pub uptime_ms: i64,
    pub tool_calls: i64,
    pub bytes_returned: i64,
    pub indexed_chunks: i64,
    pub search_calls: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ContextSavings {
    pub estimated_tokens_saved: i64,
    pub estimated_usd_saved: String,
    pub resume_events: i64,
    pub compact_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ThinkInCodeComparison {
    pub raw_output_bytes: i64,
    pub summarized_output_bytes: i64,
    pub bytes_saved: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ToolSavingsRow {
    pub tool: String,
    pub calls: i64,
    pub bytes_returned: i64,
    pub estimated_tokens_saved: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SandboxIO {
    pub stdout_bytes: i64,
    pub stderr_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct McpToolUsageRow {
    pub tool: String,
    pub calls: i64,
    pub bytes_returned: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct FullReport {
    pub generated_at: String,
    pub runtime: RuntimeStats,
    pub context_savings: ContextSavings,
    pub tool_savings: Vec<ToolSavingsRow>,
    pub mcp_tool_usage: Vec<McpToolUsageRow>,
    pub session_meta: Option<SessionMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LifetimeStats {
    pub sessions: i64,
    pub events: i64,
    pub tool_calls: i64,
    pub bytes_returned: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AttributionSource {
    DirectEventProjectDir,
    PathSignal,
    Cwd,
    LatestSessionProject,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ProjectAttribution {
    pub project_dir: String,
    pub source: AttributionSource,
    pub confidence: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AttributionContext {
    pub cwd: Option<String>,
    pub known_project_roots: Vec<String>,
    pub latest_session_project_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RestoredSessionStats {
    pub total_calls: i64,
    pub total_bytes_returned: i64,
    pub by_tool: std::collections::HashMap<String, ToolCallRow>,
}
