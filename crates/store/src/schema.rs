use crate::types::FTS5_COLUMNS;
use rusqlite::Connection;

pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS sources (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          label TEXT NOT NULL,
          chunk_count INTEGER NOT NULL DEFAULT 0,
          code_chunk_count INTEGER NOT NULL DEFAULT 0,
          indexed_at TEXT NOT NULL DEFAULT (datetime('now')),
          file_path TEXT,
          content_hash TEXT
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS chunks USING fts5({}, tokenize='porter unicode61');
        CREATE VIRTUAL TABLE IF NOT EXISTS chunks_trigram USING fts5({}, tokenize='trigram');

        CREATE TABLE IF NOT EXISTS vocabulary (
          word TEXT PRIMARY KEY
        );

        CREATE INDEX IF NOT EXISTS idx_sources_label ON sources(label);
        "#,
        FTS5_COLUMNS, FTS5_COLUMNS
    ))?;

    migrate_fts_schema(conn)?;
    add_column_if_missing(conn, "sources", "file_path", "TEXT")?;
    add_column_if_missing(conn, "sources", "content_hash", "TEXT")?;
    Ok(())
}

fn migrate_fts_schema(conn: &Connection) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare("SELECT name FROM pragma_table_xinfo('chunks')")?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if !columns.is_empty() && !columns.iter().any(|name| name == "source_category") {
        conn.execute_batch(&format!(
            r#"
            DROP TABLE IF EXISTS chunks;
            DROP TABLE IF EXISTS chunks_trigram;
            CREATE VIRTUAL TABLE chunks USING fts5({}, tokenize='porter unicode61');
            CREATE VIRTUAL TABLE chunks_trigram USING fts5({}, tokenize='trigram');
            "#,
            FTS5_COLUMNS, FTS5_COLUMNS
        ))?;
    }
    Ok(())
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(&format!("SELECT name FROM pragma_table_xinfo('{}')", table))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !columns.iter().any(|name| name == column) {
        conn.execute_batch(&format!(
            "ALTER TABLE {} ADD COLUMN {} {}",
            table, column, definition
        ))?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct PreparedStatements {
    pub stmt_insert_source: &'static str,
    pub stmt_insert_chunk: &'static str,
    pub stmt_insert_chunk_trigram: &'static str,
    pub stmt_insert_vocab: &'static str,
    pub stmt_delete_chunks_by_label: &'static str,
    pub stmt_delete_chunks_trigram_by_label: &'static str,
    pub stmt_delete_sources_by_label: &'static str,
    pub stmt_fuzzy_vocab: &'static str,
    pub stmt_list_sources: &'static str,
    pub stmt_chunks_by_source: &'static str,
    pub stmt_source_chunk_count: &'static str,
    pub stmt_chunk_content: &'static str,
    pub stmt_stats: &'static str,
    pub stmt_source_meta: &'static str,
    pub stmt_cleanup_chunks: &'static str,
    pub stmt_cleanup_chunks_trigram: &'static str,
    pub stmt_cleanup_sources: &'static str,
}

pub const PREPARED_STATEMENTS: PreparedStatements = PreparedStatements {
    stmt_insert_source: "INSERT INTO sources (label, chunk_count, code_chunk_count, file_path, content_hash) VALUES (?1, ?2, ?3, ?4, ?5)",
    stmt_insert_chunk: "INSERT INTO chunks (title, content, source_id, content_type, source_category, session_id, event_id, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    stmt_insert_chunk_trigram: "INSERT INTO chunks_trigram (title, content, source_id, content_type, source_category, session_id, event_id, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    stmt_insert_vocab: "INSERT OR IGNORE INTO vocabulary (word) VALUES (?1)",
    stmt_delete_chunks_by_label: "DELETE FROM chunks WHERE source_id IN (SELECT id FROM sources WHERE label = ?1)",
    stmt_delete_chunks_trigram_by_label: "DELETE FROM chunks_trigram WHERE source_id IN (SELECT id FROM sources WHERE label = ?1)",
    stmt_delete_sources_by_label: "DELETE FROM sources WHERE label = ?1",
    stmt_fuzzy_vocab: "SELECT word FROM vocabulary WHERE length(word) BETWEEN ?1 AND ?2",
    stmt_list_sources: "SELECT label, chunk_count FROM sources ORDER BY id DESC",
    stmt_chunks_by_source: "SELECT c.title, c.content, c.content_type, s.label FROM chunks c JOIN sources s ON s.id = c.source_id WHERE c.source_id = ?1 ORDER BY c.rowid",
    stmt_source_chunk_count: "SELECT chunk_count FROM sources WHERE id = ?1",
    stmt_chunk_content: "SELECT content FROM chunks WHERE source_id = ?1",
    stmt_source_meta: "SELECT label, chunk_count, code_chunk_count, indexed_at, file_path, content_hash FROM sources WHERE label = ?1",
    stmt_stats: "SELECT (SELECT COUNT(*) FROM sources) AS sources, (SELECT COUNT(*) FROM chunks) AS chunks, (SELECT COUNT(*) FROM chunks WHERE content_type = 'code') AS code_chunks",
    stmt_cleanup_chunks: "DELETE FROM chunks WHERE source_id IN (SELECT id FROM sources WHERE datetime(indexed_at) < datetime('now', '-' || ?1 || ' days'))",
    stmt_cleanup_chunks_trigram: "DELETE FROM chunks_trigram WHERE source_id IN (SELECT id FROM sources WHERE datetime(indexed_at) < datetime('now', '-' || ?1 || ' days'))",
    stmt_cleanup_sources: "DELETE FROM sources WHERE datetime(indexed_at) < datetime('now', '-' || ?1 || ' days')",
};

pub fn prepare_statements() -> PreparedStatements {
    PREPARED_STATEMENTS
}

#[derive(Debug, Clone, Copy)]
pub enum FtsTable {
    Chunks,
    ChunksTrigram,
}

impl FtsTable {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chunks => "chunks",
            Self::ChunksTrigram => "chunks_trigram",
        }
    }
}

pub fn build_search_sql(
    table: FtsTable,
    source_filter: Option<&str>,
    content_type_filter: bool,
) -> String {
    let table = table.as_str();
    let cols = format!(
        "{table}.title, {table}.content, {table}.content_type, {table}.timestamp, sources.label, bm25({table}, 5.0, 1.0) AS rank, highlight({table}, 1, char(2), char(3)) AS highlighted"
    );
    let from = format!("FROM {table} JOIN sources ON sources.id = {table}.source_id");
    let mut conditions = vec![format!("{table} MATCH ?1")];
    if source_filter == Some("like") {
        conditions.push("sources.label LIKE ?2".to_string());
    }
    if source_filter == Some("exact") {
        conditions.push("sources.label = ?2".to_string());
    }
    if content_type_filter {
        let idx = if source_filter.is_some() { 3 } else { 2 };
        conditions.push(format!("{table}.content_type = ?{idx}"));
    }
    let limit_idx = 2 + usize::from(source_filter.is_some()) + usize::from(content_type_filter);
    format!(
        "SELECT {cols} {from} WHERE {} ORDER BY rank LIMIT ?{limit_idx}",
        conditions.join(" AND ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_creates_fts5_tables_with_expected_columns() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let columns: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_xinfo('chunks')")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(columns.contains(&"source_category".to_string()));
        assert!(columns.contains(&"timestamp".to_string()));
    }
}
