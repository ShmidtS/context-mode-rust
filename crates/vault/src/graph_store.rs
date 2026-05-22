use crate::types::{
    VaultConfidence, VaultEdge, VaultEdgeInput, VaultFrontmatterKey, VaultNode, VaultNodeInput,
    VaultTag,
};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraphStoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, GraphStoreError>;

pub struct VaultGraphStore {
    conn: Connection,
    vault_path: String,
}

#[derive(Debug, Clone)]
pub struct UpsertNodeParams<'a> {
    pub vault_path: &'a str,
    pub note_path: &'a str,
    pub title: &'a str,
    pub frontmatter: Option<&'a str>,
    pub content_hash: &'a str,
    pub file_mtime: f64,
    pub source_id: Option<i64>,
    pub source_type: &'a str,
    pub connector_meta: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct InsertEdgeParams<'a> {
    pub source_id: i64,
    pub target_id: Option<i64>,
    pub target_name: &'a str,
    pub alias: Option<&'a str>,
    pub line_number: Option<i64>,
    pub context: Option<&'a str>,
    pub edge_type: &'a str,
    pub confidence: VaultConfidence,
}

impl VaultGraphStore {
    pub fn open(path: impl AsRef<Path>, vault_path: impl Into<String>) -> Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self {
            conn,
            vault_path: vault_path.into(),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn from_connection(conn: Connection, vault_path: impl Into<String>) -> Result<Self> {
        let store = Self {
            conn,
            vault_path: vault_path.into(),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS vault_nodes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                vault_path TEXT NOT NULL,
                note_path TEXT NOT NULL,
                title TEXT NOT NULL,
                frontmatter TEXT,
                content_hash TEXT NOT NULL,
                file_mtime REAL NOT NULL,
                out_degree INTEGER NOT NULL DEFAULT 0,
                in_degree INTEGER NOT NULL DEFAULT 0,
                source_id INTEGER,
                indexed_at TEXT NOT NULL DEFAULT (datetime('now')),
                source_type TEXT NOT NULL DEFAULT 'vault',
                connector_meta TEXT,
                UNIQUE(vault_path, note_path)
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_vault_nodes_path ON vault_nodes(vault_path, note_path);
            CREATE INDEX IF NOT EXISTS idx_vault_nodes_note_path ON vault_nodes(note_path);
            CREATE INDEX IF NOT EXISTS idx_vault_nodes_title ON vault_nodes(title);
            CREATE INDEX IF NOT EXISTS idx_vault_nodes_in_degree ON vault_nodes(in_degree DESC);

            CREATE TABLE IF NOT EXISTS vault_edges (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_id INTEGER NOT NULL,
                target_id INTEGER,
                target_name TEXT NOT NULL,
                alias TEXT,
                line_number INTEGER,
                context TEXT,
                edge_type TEXT NOT NULL DEFAULT 'wikilink',
                confidence TEXT NOT NULL DEFAULT 'EXTRACTED',
                FOREIGN KEY (source_id) REFERENCES vault_nodes(id)
            );
            CREATE INDEX IF NOT EXISTS idx_vault_edges_source ON vault_edges(source_id);
            CREATE INDEX IF NOT EXISTS idx_vault_edges_target ON vault_edges(target_id);

            CREATE TABLE IF NOT EXISTS vault_tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tag TEXT NOT NULL,
                node_id INTEGER NOT NULL,
                FOREIGN KEY (node_id) REFERENCES vault_nodes(id)
            );
            CREATE INDEX IF NOT EXISTS idx_vault_tags_tag ON vault_tags(tag);
            CREATE INDEX IF NOT EXISTS idx_vault_tags_node ON vault_tags(node_id);

            CREATE TABLE IF NOT EXISTS vault_frontmatter_keys (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                node_id INTEGER NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                UNIQUE(node_id, key),
                FOREIGN KEY (node_id) REFERENCES vault_nodes(id)
            );
            CREATE INDEX IF NOT EXISTS idx_vault_fmk_key_value ON vault_frontmatter_keys(key, value);",
        )?;
        Ok(())
    }

    pub fn get_node(&self, path: &str) -> Result<Option<VaultNodeInput>> {
        let node = self.get_node_by_path(&self.vault_path, path)?;
        Ok(node.map(|n| VaultNodeInput {
            path: n.note_path,
            title: n.title,
            frontmatter: n
                .frontmatter
                .and_then(|f| serde_json::from_str(&f).ok())
                .unwrap_or_default(),
            tags: self
                .get_tags_by_node(n.id)
                .unwrap_or_default()
                .into_iter()
                .map(|t| t.tag)
                .collect(),
            content_hash: n.content_hash,
            mtime_ms: n.file_mtime,
            in_degree: n.in_degree,
        }))
    }

    pub fn upsert_node_input(&self, node: &VaultNodeInput) -> Result<i64> {
        let frontmatter = serde_json::to_string(&node.frontmatter)?;
        let id: i64 = self.conn.query_row(
            "INSERT INTO vault_nodes (vault_path, note_path, title, frontmatter, content_hash, file_mtime, out_degree, in_degree, source_id, source_type, connector_meta)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, NULL, 'vault', NULL)
             ON CONFLICT(vault_path, note_path) DO UPDATE SET
                title = excluded.title,
                frontmatter = excluded.frontmatter,
                content_hash = excluded.content_hash,
                file_mtime = excluded.file_mtime,
                indexed_at = datetime('now')
             RETURNING id",
            params![self.vault_path, node.path, node.title, frontmatter, node.content_hash, node.mtime_ms, node.in_degree],
            |row| row.get(0),
        )?;
        self.delete_tags_by_node(id)?;
        self.delete_fmk_by_node(id)?;
        for tag in &node.tags {
            self.insert_tag(tag, id)?;
        }
        for (key, value) in &node.frontmatter {
            self.insert_frontmatter_key(id, key, &value.to_string())?;
        }
        Ok(id)
    }

    pub fn upsert_edge_input(&self, edge: &VaultEdgeInput) -> Result<()> {
        let source = self.get_node_by_path(&self.vault_path, &edge.source_path)?;
        let Some(source) = source else {
            return Ok(());
        };
        let target = match &edge.target_path {
            Some(path) => self.get_node_by_path(&self.vault_path, path)?.map(|n| n.id),
            None => None,
        };
        let target_name = edge
            .target_name
            .clone()
            .or_else(|| edge.target_path.clone())
            .unwrap_or_default();
        self.insert_edge(InsertEdgeParams {
            source_id: source.id,
            target_id: target,
            target_name: &target_name,
            alias: edge.alias.as_deref(),
            line_number: Some(edge.line_number),
            context: Some(&edge.context),
            edge_type: edge.link_type.as_str(),
            confidence: edge.confidence.clone().unwrap_or_default(),
        })?;
        Ok(())
    }

    pub fn remove_edges_from(&self, source_path: &str) -> Result<()> {
        if let Some(node) = self.get_node_by_path(&self.vault_path, source_path)? {
            self.delete_edges_by_source(node.id)?;
        }
        Ok(())
    }

    pub fn upsert_node(&self, params: UpsertNodeParams<'_>) -> Result<i64> {
        let id = self.conn.query_row(
            "INSERT INTO vault_nodes (vault_path, note_path, title, frontmatter, content_hash, file_mtime, out_degree, in_degree, source_id, source_type, connector_meta)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, ?7, ?8, ?9)
             ON CONFLICT(vault_path, note_path) DO UPDATE SET
                title = excluded.title,
                frontmatter = excluded.frontmatter,
                content_hash = excluded.content_hash,
                file_mtime = excluded.file_mtime,
                source_id = excluded.source_id,
                source_type = excluded.source_type,
                connector_meta = excluded.connector_meta,
                indexed_at = datetime('now')
             RETURNING id",
            params![
                params.vault_path,
                params.note_path,
                params.title,
                params.frontmatter,
                params.content_hash,
                params.file_mtime,
                params.source_id,
                params.source_type,
                params.connector_meta
            ],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn get_node_by_id(&self, id: i64) -> Result<Option<VaultNode>> {
        self.conn
            .query_row(
                "SELECT * FROM vault_nodes WHERE id = ?1",
                params![id],
                map_node,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn get_node_by_path(&self, vault_path: &str, note_path: &str) -> Result<Option<VaultNode>> {
        self.conn
            .query_row(
                "SELECT * FROM vault_nodes WHERE vault_path = ?1 AND note_path = ?2",
                params![vault_path, note_path],
                map_node,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn get_node_by_note_path(&self, note_path: &str) -> Result<Option<VaultNode>> {
        self.conn
            .query_row(
                "SELECT * FROM vault_nodes WHERE note_path = ?1 LIMIT 1",
                params![note_path],
                map_node,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn get_node_by_title(&self, title: &str) -> Result<Option<VaultNode>> {
        self.conn
            .query_row(
                "SELECT * FROM vault_nodes WHERE title = ?1 COLLATE NOCASE LIMIT 1",
                params![title],
                map_node,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn search_nodes(&self, query: &str) -> Result<Vec<VaultNode>> {
        let tokens: Vec<String> = query
            .split_whitespace()
            .map(|t| {
                t.chars()
                    .filter(|c| c.is_ascii_alphanumeric() || "_./-".contains(*c))
                    .collect::<String>()
            })
            .filter(|t| t.len() >= 3)
            .collect();
        if tokens.is_empty() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for token in tokens {
            let pattern = format!("%{token}%");
            let mut stmt = self
                .conn
                .prepare("SELECT * FROM vault_nodes WHERE note_path LIKE ?1 OR title LIKE ?1")?;
            let rows = stmt.query_map(params![pattern], map_node)?;
            for row in rows {
                out.push(row?);
            }
        }
        out.sort_by_key(|n| n.id);
        out.dedup_by_key(|n| n.id);
        Ok(out)
    }

    pub fn get_nodes_by_vault_path(&self, vault_path: &str) -> Result<Vec<VaultNode>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM vault_nodes WHERE vault_path = ?1")?;
        rows_to_vec(stmt.query_map(params![vault_path], map_node)?)
    }

    pub fn get_nodes_by_tag(&self, tag: &str) -> Result<Vec<VaultNode>> {
        let mut stmt = self.conn.prepare(
            "SELECT n.* FROM vault_nodes n JOIN vault_tags t ON t.node_id = n.id WHERE t.tag = ?1",
        )?;
        rows_to_vec(stmt.query_map(params![tag], map_node)?)
    }

    pub fn get_nodes_by_tag_hierarchy(&self, tag: &str) -> Result<Vec<VaultNode>> {
        let like = format!("{tag}/%");
        let mut stmt = self.conn.prepare("SELECT DISTINCT n.* FROM vault_nodes n JOIN vault_tags t ON t.node_id = n.id WHERE t.tag = ?1 OR t.tag LIKE ?2")?;
        rows_to_vec(stmt.query_map(params![tag, like], map_node)?)
    }

    pub fn insert_edge(&self, params: InsertEdgeParams<'_>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO vault_edges (source_id, target_id, target_name, alias, line_number, context, edge_type, confidence) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                params.source_id,
                params.target_id,
                params.target_name,
                params.alias,
                params.line_number,
                params.context,
                params.edge_type,
                params.confidence.to_string()
            ],
        )?;
        self.recalc_degrees(params.source_id)?;
        if let Some(target_id) = params.target_id {
            self.recalc_degrees(target_id)?;
        }
        Ok(())
    }

    pub fn delete_edges_by_source(&self, source_id: i64) -> Result<()> {
        let mut stmt = self
            .conn
            .prepare("SELECT target_id FROM vault_edges WHERE source_id = ?1")?;
        let target_ids: Vec<Option<i64>> = stmt
            .query_map(params![source_id], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        self.conn.execute(
            "DELETE FROM vault_edges WHERE source_id = ?1",
            params![source_id],
        )?;
        self.recalc_degrees(source_id)?;
        for target_id in target_ids.into_iter().flatten() {
            self.recalc_degrees(target_id)?;
        }
        Ok(())
    }

    pub fn get_edges_by_source(&self, source_id: i64) -> Result<Vec<VaultEdge>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM vault_edges WHERE source_id = ?1")?;
        rows_to_vec(stmt.query_map(params![source_id], map_edge)?)
    }

    pub fn get_edges_by_target(&self, target_id: i64) -> Result<Vec<VaultEdge>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM vault_edges WHERE target_id = ?1")?;
        rows_to_vec(stmt.query_map(params![target_id], map_edge)?)
    }

    pub fn get_all_edges(&self) -> Result<Vec<VaultEdge>> {
        let mut stmt = self.conn.prepare("SELECT * FROM vault_edges")?;
        rows_to_vec(stmt.query_map([], map_edge)?)
    }

    pub fn get_all_nodes(&self) -> Result<Vec<VaultNode>> {
        let mut stmt = self.conn.prepare("SELECT * FROM vault_nodes")?;
        rows_to_vec(stmt.query_map([], map_node)?)
    }

    pub fn get_all_node_ids(&self) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare("SELECT id FROM vault_nodes")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows_to_vec(rows)
    }

    pub fn get_node_tag_map(&self) -> Result<HashMap<i64, Vec<String>>> {
        let mut stmt = self.conn.prepare("SELECT node_id, tag FROM vault_tags")?;
        let mut rows = stmt.query([])?;
        let mut map = HashMap::new();
        while let Some(row) = rows.next()? {
            map.entry(row.get::<_, i64>(0)?)
                .or_insert_with(Vec::new)
                .push(row.get::<_, String>(1)?);
        }
        Ok(map)
    }

    pub fn insert_tag(&self, tag: &str, node_id: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO vault_tags (tag, node_id) VALUES (?1, ?2)",
            params![tag, node_id],
        )?;
        Ok(())
    }

    pub fn delete_tags_by_node(&self, node_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM vault_tags WHERE node_id = ?1",
            params![node_id],
        )?;
        Ok(())
    }

    pub fn get_tags_by_node(&self, node_id: i64) -> Result<Vec<VaultTag>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, tag, node_id FROM vault_tags WHERE node_id = ?1")?;
        rows_to_vec(stmt.query_map(params![node_id], |r| {
            Ok(VaultTag {
                id: r.get(0)?,
                tag: r.get(1)?,
                node_id: r.get(2)?,
            })
        })?)
    }

    pub fn insert_frontmatter_key(&self, node_id: i64, key: &str, value: &str) -> Result<()> {
        self.conn.execute("INSERT OR REPLACE INTO vault_frontmatter_keys (node_id, key, value) VALUES (?1, ?2, ?3)", params![node_id, key, value])?;
        Ok(())
    }

    pub fn delete_fmk_by_node(&self, node_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM vault_frontmatter_keys WHERE node_id = ?1",
            params![node_id],
        )?;
        Ok(())
    }

    pub fn get_fmk_by_node(&self, node_id: i64) -> Result<Vec<VaultFrontmatterKey>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, node_id, key, value FROM vault_frontmatter_keys WHERE node_id = ?1",
        )?;
        rows_to_vec(stmt.query_map(params![node_id], |r| {
            Ok(VaultFrontmatterKey {
                id: r.get(0)?,
                node_id: r.get(1)?,
                key: r.get(2)?,
                value: r.get(3)?,
            })
        })?)
    }

    pub fn get_node_count(&self) -> Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM vault_nodes", [], |r| r.get(0))
            .map_err(Into::into)
    }

    pub fn get_edge_count(&self) -> Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM vault_edges", [], |r| r.get(0))
            .map_err(Into::into)
    }

    pub fn find_node_by_title_like(&self, pattern: &str) -> Result<Option<(i64, i64)>> {
        self.conn
            .query_row(
                "SELECT id, in_degree FROM vault_nodes WHERE note_path LIKE ?1 LIMIT 1",
                params![pattern],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn find_node_by_path_like(&self, pattern: &str) -> Result<Option<VaultNode>> {
        self.conn
            .query_row(
                "SELECT * FROM vault_nodes WHERE note_path LIKE ?1 LIMIT 1",
                params![pattern],
                map_node,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn recalc_degrees(&self, node_id: i64) -> Result<()> {
        self.conn.execute("UPDATE vault_nodes SET out_degree = (SELECT COUNT(*) FROM vault_edges WHERE source_id = ?1) WHERE id = ?1", params![node_id])?;
        self.conn.execute("UPDATE vault_nodes SET in_degree = (SELECT COUNT(*) FROM vault_edges WHERE target_id = ?1) WHERE id = ?1", params![node_id])?;
        Ok(())
    }
}

fn rows_to_vec<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>> {
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn map_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<VaultNode> {
    Ok(VaultNode {
        id: row.get("id")?,
        vault_path: row.get("vault_path")?,
        note_path: row.get("note_path")?,
        title: row.get("title")?,
        frontmatter: row.get("frontmatter")?,
        content_hash: row.get("content_hash")?,
        file_mtime: row.get("file_mtime")?,
        out_degree: row.get("out_degree")?,
        in_degree: row.get("in_degree")?,
        source_id: row.get("source_id")?,
        indexed_at: row.get("indexed_at")?,
        source_type: row.get("source_type")?,
        connector_meta: row.get("connector_meta")?,
    })
}

fn map_edge(row: &rusqlite::Row<'_>) -> rusqlite::Result<VaultEdge> {
    let confidence: String = row.get("confidence")?;
    Ok(VaultEdge {
        id: row.get("id")?,
        source_id: row.get("source_id")?,
        target_id: row.get("target_id")?,
        target_name: row.get("target_name")?,
        alias: row.get("alias")?,
        line_number: row.get("line_number")?,
        context: row.get("context")?,
        edge_type: row.get("edge_type")?,
        confidence: VaultConfidence::from(confidence.as_str()),
    })
}
