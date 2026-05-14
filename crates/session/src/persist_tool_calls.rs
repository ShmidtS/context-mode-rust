use crate::db::SessionDB;
use crate::types::{RestoredSessionStats, Result, ToolCallRow};
use std::collections::HashMap;

pub fn persist_tool_call_counter(
    db: &SessionDB,
    session_id: &str,
    tool: &str,
    bytes_returned: i64,
) -> Result<()> {
    db.increment_tool_call(session_id, tool, bytes_returned)
}

pub fn restore_session_stats(db: &SessionDB, session_id: &str) -> Result<RestoredSessionStats> {
    let stats = db.get_tool_call_stats(session_id)?;
    let by_tool: HashMap<String, ToolCallRow> = stats
        .by_tool
        .into_iter()
        .map(|row| (row.tool.clone(), row))
        .collect();
    Ok(RestoredSessionStats {
        total_calls: stats.total_calls,
        total_bytes_returned: stats.total_bytes_returned,
        by_tool,
    })
}

pub fn persist(db: &SessionDB, session_id: &str, tool: &str, bytes_returned: i64) -> Result<()> {
    persist_tool_call_counter(db, session_id, tool, bytes_returned)
}
