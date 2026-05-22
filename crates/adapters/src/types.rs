use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub type HookRegistration = HashMap<String, Vec<HookEntry>>;

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(PlatformId),
    #[error("missing hook script for hook type: {0}")]
    MissingHookScript(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlatformId {
    ClaudeCode,
    Codex,
    Cursor,
    GeminiCli,
    JetbrainsCopilot,
    Kiro,
    OpenClaw,
    OpenCode,
    QwenCode,
    VscodeCopilot,
    Zed,
    Unknown,
}

impl std::fmt::Display for PlatformId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PlatformId::ClaudeCode => "claude-code",
            PlatformId::Codex => "codex",
            PlatformId::Cursor => "cursor",
            PlatformId::GeminiCli => "gemini-cli",
            PlatformId::JetbrainsCopilot => "jetbrains-copilot",
            PlatformId::Kiro => "kiro",
            PlatformId::OpenClaw => "openclaw",
            PlatformId::OpenCode => "opencode",
            PlatformId::QwenCode => "qwen-code",
            PlatformId::VscodeCopilot => "vscode-copilot",
            PlatformId::Zed => "zed",
            PlatformId::Unknown => "unknown",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookParadigm {
    JsonStdio,
    TsPlugin,
    McpOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformCapabilities {
    pub pre_tool_use: bool,
    pub post_tool_use: bool,
    pub pre_compact: bool,
    pub session_start: bool,
    pub can_modify_args: bool,
    pub can_modify_output: bool,
    pub can_inject_session_context: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreToolUseEvent {
    pub tool_name: String,
    pub tool_input: HashMap<String, Value>,
    pub session_id: String,
    pub project_dir: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostToolUseEvent {
    pub tool_name: String,
    pub tool_input: HashMap<String, Value>,
    pub tool_output: Option<String>,
    pub is_error: Option<bool>,
    pub session_id: String,
    pub project_dir: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreCompactEvent {
    pub session_id: String,
    pub project_dir: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStartSource {
    Startup,
    Compact,
    Resume,
    Clear,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionStartEvent {
    pub session_id: String,
    pub source: SessionStartSource,
    pub project_dir: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PreToolUseDecision {
    Allow,
    Deny,
    Modify,
    Context,
    Ask,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreToolUseResponse {
    pub permission_decision: PreToolUseDecision,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_decision_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostToolUseResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_output: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreCompactResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_user_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookCommand {
    #[serde(rename = "type")]
    pub hook_type: String,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookEntry {
    pub matcher: String,
    pub hooks: Vec<HookCommand>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticStatus {
    Pass,
    Fail,
    Warn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticResult {
    pub check: String,
    pub status: DiagnosticStatus,
    pub message: String,
    pub fix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DetectionSignal {
    pub env_vars: HashMap<String, String>,
    pub config_paths: Vec<PathBuf>,
    pub process_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlatformDetection {
    pub platform: Option<PlatformId>,
    pub confidence: f32,
    pub reason: String,
}

pub trait HookAdapter: Send + Sync {
    fn install(&self, plugin_root: &str) -> Result<Vec<String>, AdapterError>;
    fn uninstall(&self) -> Result<Vec<String>, AdapterError>;
    fn diagnostics(&self, plugin_root: &str) -> Result<Vec<DiagnosticResult>, AdapterError>;
    fn settings_path(&self) -> PathBuf;
    fn hook_paths(&self, plugin_root: &str) -> Vec<PathBuf>;
    fn platform_id(&self) -> PlatformId;
}
