use std::collections::HashMap;

use crate::types::{HookEntry, SessionStartSource};

pub fn normalize_session_source(raw_source: Option<&str>) -> SessionStartSource {
    match raw_source.unwrap_or("startup") {
        "compact" => SessionStartSource::Compact,
        "resume" => SessionStartSource::Resume,
        "clear" => SessionStartSource::Clear,
        _ => SessionStartSource::Startup,
    }
}

pub fn upsert_hook_entry<F>(
    hooks: &mut HashMap<String, Vec<HookEntry>>,
    hook_type: &str,
    entry: HookEntry,
    changes: &mut Vec<String>,
    is_match: F,
) where
    F: Fn(&HookEntry) -> bool,
{
    let existing = hooks.entry(hook_type.to_string()).or_default();

    if let Some(index) = existing.iter().position(is_match) {
        existing[index] = entry;
        changes.push(format!("Updated existing {hook_type} hook entry"));
    } else {
        existing.push(entry);
        changes.push(format!("Added {hook_type} hook entry"));
    }
}
