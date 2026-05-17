use std::path::Path;

use rusqlite::Connection;

use crate::db_schema;
use crate::search;
use crate::types::SearchResult;

/// High-level local searcher that manages its own DB connection.
pub struct LocalSearcher {
    conn: Connection,
}

fn default_db_path() -> std::path::PathBuf {
    let mut db_dir = context_mode_utils::paths::home_or_current();
    db_dir.push(".context-mode");
    db_dir.push("code-index.db");
    db_dir
}

impl LocalSearcher {
    /// Open a local search DB. If `db_path` is `None`, opens the default on-disk DB
    /// at `~/.context-mode/code-index.db`.
    pub fn open(db_path: Option<&Path>) -> anyhow::Result<Self> {
        let path = db_path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(default_db_path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(&path)?;
        db_schema::init_local_schema(&conn)?;
        Ok(Self { conn })
    }

    /// Search across all indexed chunks, optionally filtered by `repo_id`.
    pub fn search(
        &self,
        query: &str,
        repo_id: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let results = match repo_id {
            Some(repo) => search::search_repo(&self.conn, repo, query, limit)?,
            None => search::search(&self.conn, query, limit)?,
        };
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_schema;

    #[test]
    fn test_local_searcher_basic() {
        let mut conn = Connection::open_in_memory().unwrap();
        db_schema::init_local_schema(&mut conn).unwrap();
        db_schema::insert_chunk(
            &mut conn,
            "hello world",
            "main",
            "fn",
            "a.rs",
            "repo1",
            1,
            5,
        )
        .unwrap();

        // Test via in-memory path by opening directly
        let searcher = LocalSearcher { conn };
        let results = searcher.search("hello", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "hello world");
    }

    #[test]
    fn test_local_searcher_repo_filter() {
        let mut conn = Connection::open_in_memory().unwrap();
        db_schema::init_local_schema(&mut conn).unwrap();
        db_schema::insert_chunk(
            &mut conn,
            "hello world",
            "main",
            "fn",
            "a.rs",
            "repo1",
            1,
            5,
        )
        .unwrap();
        db_schema::insert_chunk(&mut conn, "hello moon", "main", "fn", "b.rs", "repo2", 1, 5)
            .unwrap();

        let searcher = LocalSearcher { conn };
        let results = searcher.search("hello", Some("repo1"), 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "a.rs");
    }
}
