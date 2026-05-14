use crate::types::{BuildSnapshotOpts, StoredEvent};
use chrono::Utc;
use context_mode_core::types::EventCategory;
use std::collections::BTreeMap;

pub fn render_task_state(events: &[StoredEvent]) -> String {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for event in events {
        let key = if event.data.contains("completed") || event.data.contains("done") {
            "completed"
        } else if event.data.contains("in_progress") || event.data.contains("active") {
            "in_progress"
        } else {
            "pending"
        };
        *counts.entry(key).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn build_resume_snapshot(events: &[StoredEvent], opts: Option<BuildSnapshotOpts>) -> String {
    let opts = opts.unwrap_or_default();
    let mut sections = Vec::new();
    sections.push(format!(
        "<session_resume generated_at=\"{}\" compact_count=\"{}\">",
        Utc::now().to_rfc3339(),
        opts.compact_count
    ));
    sections.push("  <how_to_search>\n  Each section below contains a summary of prior work. For full details, run the shown search tool with the provided query.\n  </how_to_search>".to_string());

    let mut grouped: BTreeMap<&'static str, Vec<&StoredEvent>> = BTreeMap::new();
    for event in events {
        grouped
            .entry(category_name(&event.category))
            .or_default()
            .push(event);
    }

    for (category, rows) in grouped {
        if rows.is_empty() {
            continue;
        }
        let query = rows
            .iter()
            .rev()
            .take(5)
            .map(|e| e.event_type.as_str())
            .collect::<Vec<_>>()
            .join(" OR ");
        sections.push(format!("  <{} count=\"{}\">", category, rows.len()));
        if category == "task" {
            sections.push(format!(
                "    <task_state>{}</task_state>",
                escape_xml(&render_task_state(
                    &rows.iter().map(|e| (*e).clone()).collect::<Vec<_>>()
                ))
            ));
        }
        for event in rows.iter().rev().take(8).rev() {
            sections.push(format!(
                "    <event id=\"{}\" type=\"{}\" priority=\"{}\" at=\"{}\">{}</event>",
                event.id,
                escape_xml(&event.event_type),
                event.priority,
                escape_xml(&event.created_at),
                escape_xml(&summarize(&event.data, 500))
            ));
        }
        sections.push(format!(
            "    <search>{}(queries: [\"{}\"])</search>",
            opts.search_tool,
            escape_xml(&query)
        ));
        sections.push(format!("  </{}>", category));
    }

    sections.push("</session_resume>".to_string());
    sections.join("\n")
}

fn category_name(category: &EventCategory) -> &'static str {
    match category {
        EventCategory::File => "file",
        EventCategory::Task => "task",
        EventCategory::Rule => "rule",
        EventCategory::Decision => "decision",
        EventCategory::Cwd => "cwd",
        EventCategory::Error => "error",
        EventCategory::Env => "env",
        EventCategory::Git => "git",
        EventCategory::Subagent => "subagent",
        EventCategory::Intent => "intent",
        EventCategory::Skill => "skill",
        EventCategory::Role => "role",
        EventCategory::Plan => "plan",
        EventCategory::Mcp => "mcp",
        EventCategory::McpToolCall => "mcp_tool_call",
        EventCategory::Prompt => "prompt",
        EventCategory::UserPrompt => "user_prompt",
        EventCategory::Test => "test",
        EventCategory::Compaction => "compaction",
        EventCategory::Status => "status",
        EventCategory::Deploy => "deploy",
        EventCategory::Log => "log",
        EventCategory::Latency => "latency",
        EventCategory::Permission => "permission",
        _ => "other",
    }
}

fn summarize(data: &str, max_len: usize) -> String {
    context_mode_utils::truncate::truncate(data, max_len).replace('\n', " ")
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
