use crate::db_schema::{self, FtsChunk};
use crate::types::{ConfidenceSource, ContentType, MatchLayer, SearchResult};
use anyhow::Result;
use rusqlite::Connection;

fn fts_chunk_to_result(chunk: FtsChunk) -> SearchResult {
    SearchResult {
        title: if chunk.symbol_name.is_empty() {
            chunk.file_path.clone()
        } else {
            chunk.symbol_name.clone()
        },
        content: chunk.content.clone(),
        source: chunk.file_path.clone(),
        rank: chunk.rank,
        content_type: if chunk.symbol_type.is_empty() {
            ContentType::Prose
        } else {
            ContentType::Code
        },
        match_layer: Some(MatchLayer::Porter),
        highlighted: None,
        timestamp: None,
        confidence: Some(1.0),
        confidence_source: Some(ConfidenceSource::EXTRACTED),
    }
}

/// Search across all indexed chunks using FTS5 BM25.
pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let chunks = db_schema::search_fts(conn, query, limit)?;
    Ok(chunks.into_iter().map(fts_chunk_to_result).collect())
}

/// Search within a specific repo.
pub fn search_repo(
    conn: &Connection,
    repo_id: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>> {
    let chunks = db_schema::search_fts_repo(conn, repo_id, query, limit)?;
    Ok(chunks.into_iter().map(fts_chunk_to_result).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_schema;
    use rusqlite::Connection;

    fn in_memory() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        db_schema::init_local_schema(&mut conn).unwrap();
        conn
    }

    #[test]
    fn test_search_fts_basic() {
        let mut conn = in_memory();
        db_schema::insert_chunk(
            &mut conn,
            "hello world",
            "main",
            "function",
            "src/main.rs",
            "repo",
            1,
            2,
        )
        .unwrap();

        let results = search(&conn, "hello", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "main");
        assert_eq!(results[0].source, "src/main.rs");
        assert_eq!(results[0].content, "hello world");
        assert_eq!(results[0].match_layer, Some(MatchLayer::Porter));
    }

    #[test]
    fn test_search_repo_filter() {
        let mut conn = in_memory();
        db_schema::insert_chunk(
            &mut conn,
            "fn foo() {}",
            "foo",
            "function",
            "a.rs",
            "repo-a",
            1,
            1,
        )
        .unwrap();
        db_schema::insert_chunk(
            &mut conn,
            "fn bar() {}",
            "bar",
            "function",
            "b.rs",
            "repo-b",
            1,
            1,
        )
        .unwrap();

        let results = search_repo(&conn, "repo-a", "foo", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "foo");
    }
}
