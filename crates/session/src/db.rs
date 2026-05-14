use crate::project_attribution::clamp_confidence;
use crate::types::{
    ProjectAttribution, Result, ResumeRow, SessionEvent, SessionMeta, StoredEvent, ToolCallRow,
    ToolCallStats,
};
use context_mode_core::types::EventCategory;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const MAX_EVENTS_PER_SESSION: i64 = 1_000;
const DEDUP_WINDOW: i64 = 200;

#[derive(Debug)]
pub struct SessionDB {
    conn: Connection,
}

impl SessionDB {
    pub fn new(conn: Connection) -> Result<Self> {
        let db = Self { conn };
        db.init()?;
        Ok(db)
    }

    pub fn open<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        if let Some(parent) = db_path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::new(Connection::open(db_path)?)
    }

    pub fn default_path() -> PathBuf {
        std::env::var_os("CONTEXT_MODE_SESSION_DB")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|h| PathBuf::from(h).join(".context-mode").join("session.db"))
            })
            .unwrap_or_else(|| PathBuf::from("session.db"))
    }

    pub fn open_default() -> Result<Self> {
        Self::open(Self::default_path())
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    fn init(&self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS session_events (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               session_id TEXT NOT NULL,
               type TEXT NOT NULL,
               category TEXT NOT NULL,
               priority INTEGER NOT NULL DEFAULT 2,
               data TEXT NOT NULL,
               project_dir TEXT NOT NULL DEFAULT '',
               attribution_source TEXT NOT NULL DEFAULT 'unknown',
               attribution_confidence REAL NOT NULL DEFAULT 0,
               source_hook TEXT NOT NULL,
               created_at TEXT NOT NULL DEFAULT (datetime('now')),
               data_hash TEXT NOT NULL DEFAULT ''
             );
             CREATE INDEX IF NOT EXISTS idx_session_events_session ON session_events(session_id);
             CREATE INDEX IF NOT EXISTS idx_session_events_type ON session_events(session_id, type);
             CREATE INDEX IF NOT EXISTS idx_session_events_priority ON session_events(session_id, priority);
             CREATE INDEX IF NOT EXISTS idx_session_events_project ON session_events(session_id, project_dir);
             CREATE TABLE IF NOT EXISTS session_meta (
               session_id TEXT PRIMARY KEY,
               project_dir TEXT NOT NULL,
               started_at TEXT NOT NULL DEFAULT (datetime('now')),
               last_event_at TEXT,
               event_count INTEGER NOT NULL DEFAULT 0,
               compact_count INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS session_resume (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               session_id TEXT NOT NULL UNIQUE,
               snapshot TEXT NOT NULL,
               event_count INTEGER NOT NULL,
               created_at TEXT NOT NULL DEFAULT (datetime('now')),
               consumed INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS tool_calls (
               session_id TEXT NOT NULL,
               tool TEXT NOT NULL,
               calls INTEGER NOT NULL DEFAULT 0,
               bytes_returned INTEGER NOT NULL DEFAULT 0,
               updated_at TEXT NOT NULL DEFAULT (datetime('now')),
               PRIMARY KEY (session_id, tool)
             );
             CREATE INDEX IF NOT EXISTS idx_tool_calls_session ON tool_calls(session_id);",
        )?;
        Ok(())
    }

    pub fn ensure_session(&self, session_id: &str, project_dir: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO session_meta (session_id, project_dir, last_event_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(session_id) DO UPDATE SET
               project_dir = CASE WHEN excluded.project_dir != '' THEN excluded.project_dir ELSE session_meta.project_dir END",
            params![session_id, project_dir],
        )?;
        Ok(())
    }

    pub fn insert_event(
        &self,
        session_id: &str,
        event: &SessionEvent,
        source_hook: &str,
        attribution: Option<&ProjectAttribution>,
    ) -> Result<()> {
        let data_hash = hash_data(&event.data);
        let project_dir = attribution
            .map(|a| a.project_dir.as_str())
            .or(event.project_dir.as_deref())
            .unwrap_or("")
            .trim()
            .to_string();
        let attribution_source = attribution
            .map(|a| format!("{:?}", a.source))
            .or_else(|| event.attribution_source.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let attribution_confidence = clamp_confidence(
            attribution
                .map(|a| a.confidence)
                .or(event.attribution_confidence)
                .unwrap_or(0.0),
        );

        self.ensure_session(session_id, &project_dir)?;
        let tx = self.conn.unchecked_transaction()?;
        let dup: Option<i64> = tx
            .query_row(
                "SELECT id FROM session_events
             WHERE session_id = ?1 AND type = ?3 AND data_hash = ?4
             ORDER BY id DESC LIMIT ?2",
                params![session_id, DEDUP_WINDOW, event.event_type, data_hash],
                |row| row.get(0),
            )
            .optional()?;
        if dup.is_none() {
            let count = self.get_event_count_tx(&tx, session_id)?;
            if count >= MAX_EVENTS_PER_SESSION {
                tx.execute(
                    "DELETE FROM session_events WHERE id = (
                       SELECT id FROM session_events WHERE session_id = ?1 ORDER BY priority ASC, id ASC LIMIT 1
                     )",
                    params![session_id],
                )?;
            }
            tx.execute(
                "INSERT INTO session_events
                 (session_id, type, category, priority, data, project_dir, attribution_source, attribution_confidence, source_hook, data_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    session_id,
                    event.event_type,
                    category_to_string(&event.category),
                    event.priority,
                    event.data,
                    project_dir,
                    attribution_source,
                    attribution_confidence,
                    source_hook,
                    data_hash
                ],
            )?;
            tx.execute(
                "UPDATE session_meta SET last_event_at = datetime('now'), event_count = event_count + 1 WHERE session_id = ?1",
                params![session_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn bulk_insert_events(
        &self,
        session_id: &str,
        events: &[SessionEvent],
        source_hook: &str,
    ) -> Result<()> {
        for event in events {
            self.insert_event(session_id, event, source_hook, None)?;
        }
        Ok(())
    }

    pub fn get_events(&self, session_id: &str, limit: Option<i64>) -> Result<Vec<StoredEvent>> {
        let sql = if limit.is_some() {
            "SELECT * FROM session_events WHERE session_id = ?1 ORDER BY id ASC LIMIT ?2"
        } else {
            "SELECT * FROM session_events WHERE session_id = ?1 ORDER BY id ASC"
        };
        let mut stmt = self.conn.prepare(sql)?;
        let rows = if let Some(limit) = limit {
            stmt.query_map(params![session_id, limit], row_to_stored_event)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![session_id], row_to_stored_event)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    pub fn get_events_by_type(
        &self,
        session_id: &str,
        event_type: &str,
    ) -> Result<Vec<StoredEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM session_events WHERE session_id = ?1 AND type = ?2 ORDER BY id ASC",
        )?;
        Ok(stmt
            .query_map(params![session_id, event_type], row_to_stored_event)?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn get_events_by_priority(
        &self,
        session_id: &str,
        min_priority: i32,
    ) -> Result<Vec<StoredEvent>> {
        let mut stmt = self.conn.prepare("SELECT * FROM session_events WHERE session_id = ?1 AND priority >= ?2 ORDER BY priority DESC, id ASC")?;
        Ok(stmt
            .query_map(params![session_id, min_priority], row_to_stored_event)?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn get_event_count(&self, session_id: &str) -> Result<i64> {
        self.get_event_count_tx(&self.conn, session_id)
    }

    fn get_event_count_tx(&self, conn: &rusqlite::Connection, session_id: &str) -> Result<i64> {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM session_events WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )?)
    }

    pub fn get_latest_attributed_project_dir(&self, session_id: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row(
            "SELECT project_dir FROM session_events WHERE session_id = ?1 AND project_dir != '' ORDER BY id DESC LIMIT 1",
            params![session_id],
            |row| row.get(0),
        ).optional()?)
    }

    pub fn search_events(
        &self,
        query: &str,
        limit: i64,
        project_dir: Option<&str>,
        source_hook: Option<&str>,
    ) -> Result<Vec<StoredEvent>> {
        let like = format!("%{}%", query);
        let project = project_dir.unwrap_or("");
        let source = source_hook.unwrap_or("");
        let mut stmt = self.conn.prepare(
            "SELECT * FROM session_events
             WHERE data LIKE ?1
               AND (?2 = '' OR project_dir = ?2)
               AND (?3 = '' OR source_hook = ?3)
             ORDER BY id DESC LIMIT ?4",
        )?;
        Ok(stmt
            .query_map(params![like, project, source, limit], row_to_stored_event)?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn get_session_stats(&self, session_id: &str) -> Result<Option<SessionMeta>> {
        Ok(self.conn.query_row(
            "SELECT session_id, project_dir, started_at, last_event_at, event_count, compact_count FROM session_meta WHERE session_id = ?1",
            params![session_id],
            |row| Ok(SessionMeta {
                session_id: row.get(0)?,
                project_dir: row.get(1)?,
                started_at: row.get(2)?,
                last_event_at: row.get(3)?,
                event_count: row.get(4)?,
                compact_count: row.get(5)?,
            }),
        ).optional()?)
    }

    pub fn increment_compact_count(&self, session_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE session_meta SET compact_count = compact_count + 1 WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(())
    }

    pub fn upsert_resume(&self, session_id: &str, snapshot: &str, event_count: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO session_resume (session_id, snapshot, event_count, consumed)
             VALUES (?1, ?2, ?3, 0)
             ON CONFLICT(session_id) DO UPDATE SET
               snapshot = excluded.snapshot,
               event_count = excluded.event_count,
               created_at = datetime('now'),
               consumed = 0",
            params![session_id, snapshot, event_count],
        )?;
        Ok(())
    }

    pub fn get_resume(&self, session_id: &str) -> Result<Option<ResumeRow>> {
        Ok(self.conn.query_row(
            "SELECT id, session_id, snapshot, event_count, created_at, consumed FROM session_resume WHERE session_id = ?1",
            params![session_id], row_to_resume,
        ).optional()?)
    }

    pub fn mark_resume_consumed(&self, session_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE session_resume SET consumed = 1 WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(())
    }

    pub fn claim_latest_unconsumed_resume(
        &self,
        session_id: Option<&str>,
    ) -> Result<Option<ResumeRow>> {
        let row = if let Some(session_id) = session_id {
            self.conn.query_row(
                "SELECT id, session_id, snapshot, event_count, created_at, consumed FROM session_resume WHERE session_id = ?1 AND consumed = 0 ORDER BY created_at DESC LIMIT 1",
                params![session_id], row_to_resume,
            ).optional()?
        } else {
            self.conn.query_row(
                "SELECT id, session_id, snapshot, event_count, created_at, consumed FROM session_resume WHERE consumed = 0 ORDER BY created_at DESC LIMIT 1",
                [], row_to_resume,
            ).optional()?
        };
        if let Some(resume) = &row {
            self.mark_resume_consumed(&resume.session_id)?;
        }
        Ok(row)
    }

    pub fn get_latest_session_id(&self) -> Result<Option<String>> {
        Ok(self.conn.query_row(
            "SELECT session_id FROM session_meta ORDER BY COALESCE(last_event_at, started_at) DESC LIMIT 1",
            [], |row| row.get(0),
        ).optional()?)
    }

    pub fn increment_tool_call(
        &self,
        session_id: &str,
        tool: &str,
        bytes_returned: i64,
    ) -> Result<()> {
        let safe_bytes = bytes_returned.max(0);
        self.conn.execute(
            "INSERT INTO tool_calls (session_id, tool, calls, bytes_returned)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(session_id, tool) DO UPDATE SET
               calls = calls + 1,
               bytes_returned = bytes_returned + excluded.bytes_returned,
               updated_at = datetime('now')",
            params![session_id, tool, safe_bytes],
        )?;
        Ok(())
    }

    pub fn get_tool_call_stats(&self, session_id: &str) -> Result<ToolCallStats> {
        let mut stmt = self.conn.prepare("SELECT tool, calls, bytes_returned FROM tool_calls WHERE session_id = ?1 ORDER BY calls DESC, tool ASC")?;
        let by_tool: Vec<ToolCallRow> = stmt
            .query_map(params![session_id], |row| {
                Ok(ToolCallRow {
                    tool: row.get(0)?,
                    calls: row.get(1)?,
                    bytes_returned: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(ToolCallStats {
            total_calls: by_tool.iter().map(|r| r.calls).sum(),
            total_bytes_returned: by_tool.iter().map(|r| r.bytes_returned).sum(),
            by_tool,
        })
    }

    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM session_events WHERE session_id = ?1",
            params![session_id],
        )?;
        self.conn.execute(
            "DELETE FROM session_meta WHERE session_id = ?1",
            params![session_id],
        )?;
        self.conn.execute(
            "DELETE FROM session_resume WHERE session_id = ?1",
            params![session_id],
        )?;
        self.conn.execute(
            "DELETE FROM tool_calls WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(())
    }

    pub fn cleanup_old_sessions(&self, max_age_days: i64) -> Result<usize> {
        let changed = self.conn.execute(
            "DELETE FROM session_meta WHERE started_at < datetime('now', ?1)",
            params![format!("-{} days", max_age_days)],
        )?;
        Ok(changed)
    }
}

fn hash_data(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())[..16].to_uppercase()
}

fn category_to_string(category: &EventCategory) -> String {
    serde_json::to_value(category)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{:?}", category).to_lowercase())
}

fn category_from_string(s: &str) -> EventCategory {
    serde_json::from_value(serde_json::Value::String(s.to_string())).unwrap_or(EventCategory::Data)
}

fn row_to_stored_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredEvent> {
    Ok(StoredEvent {
        id: row.get("id")?,
        session_id: row.get("session_id")?,
        event_type: row.get("type")?,
        category: category_from_string(&row.get::<_, String>("category")?),
        priority: row.get("priority")?,
        data: row.get("data")?,
        project_dir: row.get("project_dir")?,
        attribution_source: row.get("attribution_source")?,
        attribution_confidence: row.get("attribution_confidence")?,
        source_hook: row.get("source_hook")?,
        created_at: row.get("created_at")?,
        data_hash: row.get("data_hash")?,
    })
}

fn row_to_resume(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResumeRow> {
    Ok(ResumeRow {
        id: row.get(0)?,
        session_id: row.get(1)?,
        snapshot: row.get(2)?,
        event_count: row.get(3)?,
        created_at: row.get(4)?,
        consumed: row.get::<_, i64>(5)? != 0,
    })
}
