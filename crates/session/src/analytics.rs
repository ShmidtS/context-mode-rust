use crate::db::SessionDB;
use crate::types::{
    ContextSavings, FullReport, LifetimeStats, McpToolUsageRow, Result, RuntimeStats,
    ToolSavingsRow,
};
use chrono::Utc;
use rusqlite::OptionalExtension;
use std::collections::BTreeMap;

pub const OPUS_INPUT_PRICE_PER_TOKEN: f64 = 15.0 / 1_000_000.0;

pub trait DatabaseAdapter {
    fn session_db(&self) -> &SessionDB;
}

impl DatabaseAdapter for SessionDB {
    fn session_db(&self) -> &SessionDB {
        self
    }
}

#[derive(Debug)]
pub struct AnalyticsEngine<'a> {
    db: &'a SessionDB,
}

impl<'a> AnalyticsEngine<'a> {
    pub fn new(db: &'a SessionDB) -> Self {
        Self { db }
    }

    pub fn get_mcp_tool_usage(&self, session_id: &str) -> Result<Vec<McpToolUsageRow>> {
        Ok(self
            .db
            .get_tool_call_stats(session_id)?
            .by_tool
            .into_iter()
            .map(|row| McpToolUsageRow {
                tool: row.tool,
                calls: row.calls,
                bytes_returned: row.bytes_returned,
            })
            .collect())
    }

    pub fn query_all(&self, runtime_stats: RuntimeStats) -> Result<FullReport> {
        let tool_stats = self.db.get_tool_call_stats(&runtime_stats.session_id)?;
        let session_meta = self.db.get_session_stats(&runtime_stats.session_id)?;
        let compact_count = session_meta
            .as_ref()
            .map(|m| m.compact_count)
            .unwrap_or_default();
        let resume_events = session_meta
            .as_ref()
            .map(|m| m.event_count)
            .unwrap_or_default();
        let estimated_tokens_saved = estimate_tokens(tool_stats.total_bytes_returned);
        let mcp_tool_usage = tool_stats
            .by_tool
            .iter()
            .map(|row| McpToolUsageRow {
                tool: row.tool.clone(),
                calls: row.calls,
                bytes_returned: row.bytes_returned,
            })
            .collect::<Vec<_>>();
        let tool_savings = tool_stats
            .by_tool
            .iter()
            .map(|row| ToolSavingsRow {
                tool: row.tool.clone(),
                calls: row.calls,
                bytes_returned: row.bytes_returned,
                estimated_tokens_saved: estimate_tokens(row.bytes_returned),
            })
            .collect::<Vec<_>>();

        Ok(FullReport {
            generated_at: Utc::now().to_rfc3339(),
            runtime: runtime_stats,
            context_savings: ContextSavings {
                estimated_tokens_saved,
                estimated_usd_saved: tokens_to_usd(estimated_tokens_saved),
                resume_events,
                compact_count,
            },
            tool_savings,
            mcp_tool_usage,
            session_meta,
        })
    }
}

pub fn category_labels() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("file", "Files"),
        ("task", "Tasks"),
        ("rule", "Rules"),
        ("decision", "Decisions"),
        ("error", "Errors"),
        ("git", "Git"),
        ("subagent", "Subagents"),
        ("intent", "Intent"),
    ])
}

pub fn category_hints() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("file", "Files touched during the session"),
        ("task", "Todo and task state changes"),
        ("decision", "Important choices and rejected approaches"),
        ("error", "Failures and recovery attempts"),
    ])
}

pub fn get_lifetime_stats(db: &SessionDB) -> Result<LifetimeStats> {
    let conn = db.connection();
    let sessions = conn
        .query_row("SELECT COUNT(*) FROM session_meta", [], |row| row.get(0))
        .optional()?
        .unwrap_or(0);
    let events = conn
        .query_row("SELECT COUNT(*) FROM session_events", [], |row| row.get(0))
        .optional()?
        .unwrap_or(0);
    let (tool_calls, bytes_returned) = conn
        .query_row(
            "SELECT COALESCE(SUM(calls), 0), COALESCE(SUM(bytes_returned), 0) FROM tool_calls",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .unwrap_or((0, 0));
    Ok(LifetimeStats {
        sessions,
        events,
        tool_calls,
        bytes_returned,
    })
}

pub fn tokens_to_usd(tokens: i64) -> String {
    format!("${:.4}", tokens as f64 * OPUS_INPUT_PRICE_PER_TOKEN)
}

pub fn format_report(report: &FullReport) -> String {
    let mut lines = Vec::new();
    lines.push("Context Mode Session Report".to_string());
    lines.push(format!("Generated: {}", report.generated_at));
    lines.push(format!("Session: {}", report.runtime.session_id));
    lines.push(format!("Tool calls: {}", report.runtime.tool_calls));
    lines.push(format!("Bytes returned: {}", report.runtime.bytes_returned));
    lines.push(format!(
        "Estimated tokens saved: {}",
        report.context_savings.estimated_tokens_saved
    ));
    lines.push(format!(
        "Estimated cost saved: {}",
        report.context_savings.estimated_usd_saved
    ));
    if !report.tool_savings.is_empty() {
        lines.push("Tools:".to_string());
        for row in &report.tool_savings {
            lines.push(format!(
                "- {}: {} calls, {} bytes",
                row.tool, row.calls, row.bytes_returned
            ));
        }
    }
    lines.join("\n")
}

fn estimate_tokens(bytes: i64) -> i64 {
    (bytes.max(0) as f64 / 4.0).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn records_tool_call_and_reads_stats() {
        let db = SessionDB::new(Connection::open_in_memory().unwrap()).unwrap();
        db.increment_tool_call("session-1", "ctx_search", 128)
            .unwrap();

        let stats = db.get_tool_call_stats("session-1").unwrap();

        assert_eq!(stats.total_calls, 1);
        assert_eq!(stats.total_bytes_returned, 128);
        assert_eq!(stats.by_tool[0].tool, "ctx_search");
    }
}
