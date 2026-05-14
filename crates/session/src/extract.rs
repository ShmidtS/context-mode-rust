use crate::types::{HookInput, Result, SessionEvent};
use context_mode_core::types::EventCategory;
use regex::Regex;
use serde_json::Value;

pub fn reset_error_resolution_state() {}
pub fn reset_iteration_loop_state() {}

pub fn extract_events(raw_input: HookInput) -> Vec<SessionEvent> {
    let input = normalize_hook_input(raw_input);
    let mut events = Vec::new();
    events.extend(extract_file_and_rule(&input));
    events.extend(extract_task(&input));
    events.extend(extract_plan(&input));
    events.extend(extract_skill(&input));
    events.extend(extract_subagent(&input));
    events.extend(extract_mcp(&input));
    events.extend(extract_decision(&input));
    events.extend(extract_cwd(&input));
    events.extend(extract_error(&input));
    events.extend(extract_git(&input));
    events.extend(extract_env(&input));
    events
}

pub fn extract_events_from_json(input: &str) -> Result<Vec<SessionEvent>> {
    Ok(extract_events(serde_json::from_str(input)?))
}

pub fn extract_user_events(message: &str) -> Vec<SessionEvent> {
    let redacted = redact_secrets(message);
    let mut events = Vec::new();
    if matches_regex(
        &redacted,
        r"(?i)\b(decided|decision|we will|use .+ instead of)\b",
    ) {
        events.push(event(
            "user_decision",
            EventCategory::Decision,
            redacted.clone(),
            3,
        ));
    }
    if matches_regex(
        &redacted,
        r"(?i)\b(todo|task|implement|fix|add|create|migrate)\b",
    ) {
        events.push(event(
            "user_intent",
            EventCategory::Intent,
            redacted.clone(),
            3,
        ));
    }
    if matches_regex(
        &redacted,
        r"(?i)\b(blocked|waiting|depends on|cannot proceed)\b",
    ) {
        events.push(event(
            "user_blocker",
            EventCategory::BlockedOn,
            redacted.clone(),
            4,
        ));
    }
    if matches_regex(
        &redacted,
        r"(?i)\b(role|you are|act as|executor|planner|verifier)\b",
    ) {
        events.push(event("user_role", EventCategory::Role, redacted, 3));
    }
    events
}

pub fn redact_secrets(input: &str) -> String {
    let patterns = [
        r#"(?i)(api[_-]?key|token|secret|password|authorization|bearer)\s*[:=]\s*["']?[^\s"']+"#,
        r#"sk-[A-Za-z0-9_-]{16,}"#,
        r#"gh[pousr]_[A-Za-z0-9_]{20,}"#,
        r#"[A-Za-z0-9+/]{32,}={0,2}"#,
    ];
    patterns.iter().fold(input.to_string(), |acc, pat| {
        Regex::new(pat)
            .map(|re| re.replace_all(&acc, "$1=[REDACTED]").to_string())
            .unwrap_or(acc)
    })
}

fn normalize_hook_input(mut input: HookInput) -> HookInput {
    input.tool_name = match input.tool_name.as_str() {
        "run_shell_command" => "Bash".to_string(),
        "read_file" => "Read".to_string(),
        "write_file" => "Write".to_string(),
        "grep_search" | "search_file_content" => "Grep".to_string(),
        "list_directory" => "LS".to_string(),
        other => other.to_string(),
    };
    input
}

fn extract_file_and_rule(input: &HookInput) -> Vec<SessionEvent> {
    let mut events = Vec::new();
    let text = json_text(&input.tool_input);
    for key in ["file_path", "path", "notebook_path"] {
        if let Some(path) = input.tool_input.get(key).and_then(Value::as_str) {
            events.push(event(
                "file_access",
                EventCategory::File,
                redact_secrets(path),
                2,
            ));
        }
    }
    if text.contains("CLAUDE.md") || text.contains("AGENTS.md") || text.contains("rules/") {
        events.push(event(
            "rule_context",
            EventCategory::Rule,
            redact_secrets(&text),
            3,
        ));
    }
    events
}

fn extract_task(input: &HookInput) -> Vec<SessionEvent> {
    if input.tool_name.eq_ignore_ascii_case("TodoWrite")
        || json_text(&input.tool_input).contains("todos")
    {
        vec![event(
            "task_state",
            EventCategory::Task,
            redact_secrets(&json_text(&input.tool_input)),
            3,
        )]
    } else {
        Vec::new()
    }
}

fn extract_plan(input: &HookInput) -> Vec<SessionEvent> {
    if input.tool_name.contains("Plan") || input.tool_name == "ExitPlanMode" {
        vec![event(
            "plan",
            EventCategory::Plan,
            redact_secrets(&json_text(&input.tool_input)),
            3,
        )]
    } else {
        Vec::new()
    }
}

fn extract_skill(input: &HookInput) -> Vec<SessionEvent> {
    if input.tool_name == "Skill" || json_text(&input.tool_input).contains("skill") {
        vec![event(
            "skill",
            EventCategory::Skill,
            redact_secrets(&json_text(&input.tool_input)),
            3,
        )]
    } else {
        Vec::new()
    }
}

fn extract_subagent(input: &HookInput) -> Vec<SessionEvent> {
    if input.tool_name == "Task" || input.tool_name.contains("Agent") {
        vec![event(
            "subagent",
            EventCategory::Subagent,
            redact_secrets(&json_text(&input.tool_input)),
            3,
        )]
    } else {
        Vec::new()
    }
}

fn extract_mcp(input: &HookInput) -> Vec<SessionEvent> {
    if input.tool_name.starts_with("mcp__") {
        vec![event(
            "mcp_tool_call",
            EventCategory::McpToolCall,
            redact_secrets(&input.tool_name),
            2,
        )]
    } else {
        Vec::new()
    }
}

fn extract_decision(input: &HookInput) -> Vec<SessionEvent> {
    let text = json_text(&input.tool_input);
    if matches_regex(&text, r"(?i)\b(decision|decided|rejected|accepted)\b") {
        vec![event(
            "decision",
            EventCategory::Decision,
            redact_secrets(&text),
            3,
        )]
    } else {
        Vec::new()
    }
}

fn extract_cwd(input: &HookInput) -> Vec<SessionEvent> {
    input
        .cwd
        .as_ref()
        .map(|cwd| vec![event("cwd", EventCategory::Cwd, cwd.clone(), 2)])
        .unwrap_or_default()
}

fn extract_error(input: &HookInput) -> Vec<SessionEvent> {
    let response = json_text(&input.tool_response);
    if input.is_error || matches_regex(&response, r"(?i)\b(error|failed|exception|panic|timeout)\b")
    {
        vec![event(
            "tool_error",
            EventCategory::Error,
            redact_secrets(&response),
            4,
        )]
    } else {
        Vec::new()
    }
}

fn extract_git(input: &HookInput) -> Vec<SessionEvent> {
    let text = json_text(&input.tool_input);
    if matches_regex(&text, r"\bgit\s+(status|diff|log|commit|push|pull|merge)\b") {
        vec![event("git", EventCategory::Git, redact_secrets(&text), 2)]
    } else {
        Vec::new()
    }
}

fn extract_env(input: &HookInput) -> Vec<SessionEvent> {
    let text = json_text(&input.tool_input);
    if matches_regex(&text, r"\b[A-Z][A-Z0-9_]{2,}\b") && text.contains('=') {
        vec![event("env", EventCategory::Env, redact_secrets(&text), 2)]
    } else {
        Vec::new()
    }
}

fn event(event_type: &str, category: EventCategory, data: String, priority: i32) -> SessionEvent {
    SessionEvent {
        event_type: event_type.to_string(),
        category,
        data,
        priority,
        data_hash: String::new(),
        project_dir: None,
        attribution_source: None,
        attribution_confidence: None,
    }
}

fn json_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn matches_regex(text: &str, pattern: &str) -> bool {
    Regex::new(pattern)
        .map(|re| re.is_match(text))
        .unwrap_or(false)
}
