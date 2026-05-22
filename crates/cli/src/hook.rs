use anyhow::Result;
use serde_json::Value;
use std::io::{self, Read};
use std::path::PathBuf;

use context_mode_session::{
    db::SessionDB,
    extract::{extract_events, extract_user_events},
    snapshot::build_resume_snapshot,
    types::{BuildSnapshotOpts, HookInput, SessionEvent},
};

pub async fn run(_platform: &str, hook_type: &str) -> Result<()> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    let db_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".context-mode")
        .join("session.db");
    let db = SessionDB::open(&db_path)?;

    match hook_type.to_lowercase().as_str() {
        "posttooluse" => handle_post_tool_use(&db, &input).await?,
        "pretooluse" => handle_pre_tool_use(&input).await?,
        "precompact" => handle_pre_compact(&db, &input).await?,
        "sessionstart" => handle_session_start(&db, &input).await?,
        "userpromptsubmit" => handle_user_prompt_submit(&db, &input).await?,
        _ => println!("{{}}"),
    }

    Ok(())
}

async fn handle_post_tool_use(db: &SessionDB, input: &str) -> Result<()> {
    let value: Value = serde_json::from_str(input)?;
    let hook_input = parse_hook_input(&value);
    let session_id = hook_input.session_id.clone().unwrap_or_default();

    if !session_id.is_empty() {
        let events = extract_events(hook_input);
        db.bulk_insert_events(&session_id, &events, "posttooluse")?;

        if let Some(tool) = value.get("tool_name").and_then(|v| v.as_str()) {
            let bytes = input.len() as i64;
            db.increment_tool_call(&session_id, tool, bytes)?;
        }
    }

    // PostToolUse: {} is valid, no action needed
    println!("{{}}");
    Ok(())
}

async fn handle_pre_tool_use(_input: &str) -> Result<()> {
    // PreToolUse: {} is valid, normal permission flow continues
    println!("{{}}");
    Ok(())
}

async fn handle_pre_compact(db: &SessionDB, input: &str) -> Result<()> {
    let value: Value = serde_json::from_str(input)?;
    let session_id = value
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if !session_id.is_empty() {
        let events = db.get_events(session_id, None)?;
        if !events.is_empty() {
            let snapshot = build_resume_snapshot(&events, Some(BuildSnapshotOpts::default()));
            db.upsert_resume(session_id, &snapshot, events.len() as i64)?;
        }
        db.increment_compact_count(session_id)?;
    }

    // PreCompact: {} is valid, compaction proceeds normally
    println!("{{}}");
    Ok(())
}

async fn handle_session_start(db: &SessionDB, input: &str) -> Result<()> {
    let value: Value = serde_json::from_str(input)?;
    let session_id = value
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut additional_context = String::new();

    if !session_id.is_empty() {
        db.ensure_session(&session_id, "")?;

        let source = value
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("startup");

        if source == "compact" || source == "resume" {
            if let Some(resume) = db.claim_latest_unconsumed_resume(Some(&session_id))? {
                additional_context = resume.snapshot;
            } else {
                let events = db.get_events(&session_id, None)?;
                if !events.is_empty() {
                    additional_context =
                        build_resume_snapshot(&events, Some(BuildSnapshotOpts::default()));
                }
            }
        } else if source == "startup" {
            db.cleanup_old_sessions(7)?;

            let cwd = value
                .get("project_dir")
                .or(value.get("cwd"))
                .and_then(|v| v.as_str())
                .unwrap_or(".");

            let claude_md_paths = [
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".claude")
                    .join("CLAUDE.md"),
                PathBuf::from(cwd).join("CLAUDE.md"),
                PathBuf::from(cwd).join(".claude").join("CLAUDE.md"),
            ];

            for path in &claude_md_paths {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        let event = SessionEvent {
                            event_type: "rule_content".to_string(),
                            category: context_mode_core::types::EventCategory::Rule,
                            data: trimmed.to_string(),
                            priority: 1,
                            data_hash: String::new(),
                            project_dir: Some(cwd.to_string()),
                            attribution_source: None,
                            attribution_confidence: None,
                        };
                        let _ = db.insert_event(&session_id, &event, "SessionStart", None);
                    }
                }
            }
        }
    }

    let output = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": additional_context
        }
    });
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}

async fn handle_user_prompt_submit(db: &SessionDB, input: &str) -> Result<()> {
    let value: Value = serde_json::from_str(input)?;
    let session_id = value
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let prompt = value
        .get("prompt")
        .or(value.get("message"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let trimmed = prompt.trim();

    let is_system = trimmed.starts_with("<task-notification>")
        || trimmed.starts_with("<system-reminder>")
        || trimmed.starts_with("<context_guidance>")
        || trimmed.starts_with("<tool-result>");

    if !session_id.is_empty() && !trimmed.is_empty() && !is_system {
        let project_dir = value
            .get("project_dir")
            .or(value.get("cwd"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        db.ensure_session(&session_id, &project_dir)?;

        let prompt_event = SessionEvent {
            event_type: "user_prompt".to_string(),
            category: context_mode_core::types::EventCategory::UserPrompt,
            data: prompt.to_string(),
            priority: 1,
            data_hash: String::new(),
            project_dir: Some(project_dir.clone()),
            attribution_source: None,
            attribution_confidence: None,
        };
        let _ = db.insert_event(&session_id, &prompt_event, "UserPromptSubmit", None);

        let user_events = extract_user_events(trimmed);
        db.bulk_insert_events(&session_id, &user_events, "UserPromptSubmit")?;
    }

    println!("{{}}");
    Ok(())
}

fn parse_hook_input(value: &Value) -> HookInput {
    HookInput {
        session_id: value
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(String::from),
        transcript_path: value
            .get("transcript_path")
            .and_then(|v| v.as_str())
            .map(String::from),
        cwd: value
            .get("project_dir")
            .or(value.get("cwd"))
            .and_then(|v| v.as_str())
            .map(String::from),
        hook_event_name: Some("posttooluse".to_string()),
        tool_name: value
            .get("tool_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        tool_input: value.get("tool_input").cloned().unwrap_or(Value::Null),
        tool_response: value
            .get("tool_output")
            .or(value.get("tool_response"))
            .cloned()
            .unwrap_or(Value::Null),
        is_error: value
            .get("is_error")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }
}
