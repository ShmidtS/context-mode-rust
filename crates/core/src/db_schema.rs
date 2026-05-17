use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};

pub const SCHEMA_VERSION: i32 = 1;

/// Initialize the local schema with migrations.
pub fn init_local_schema(conn: &Connection) -> Result<()> {
    let current_version: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap_or(0);

    if current_version < 1 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                repo_id TEXT NOT NULL,
                mtime REAL NOT NULL,
                size INTEGER NOT NULL,
                sha256 TEXT NOT NULL,
                indexed_at REAL NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_files_repo ON files(repo_id);
            CREATE INDEX IF NOT EXISTS idx_files_sha ON files(sha256);

            CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                content,
                symbol_name,
                symbol_type,
                file_path UNINDEXED,
                repo_id UNINDEXED,
                start_line UNINDEXED,
                end_line UNINDEXED,
                tokenize='porter unicode61'
            );

            CREATE TABLE IF NOT EXISTS vectors (
                chunk_id INTEGER PRIMARY KEY,
                vec BLOB NOT NULL
            );

            CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                repo_id TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at REAL NOT NULL,
                completed_at REAL,
                error TEXT,
                nodes_indexed INTEGER,
                edges_indexed INTEGER
            );

            CREATE INDEX IF NOT EXISTS idx_jobs_repo ON jobs(repo_id);
            CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);

            PRAGMA user_version = 1;
            ",
        )?;
    }

    Ok(())
}

/// A single file record.
#[derive(Debug, Clone)]
pub struct FileRecord {
    pub path: String,
    pub repo_id: String,
    pub mtime: f64,
    pub size: i64,
    pub sha256: String,
    pub indexed_at: f64,
}

/// A single FTS chunk result.
#[derive(Debug, Clone)]
pub struct FtsChunk {
    pub rowid: i64,
    pub rank: f64,
    pub content: String,
    pub symbol_name: String,
    pub symbol_type: String,
    pub file_path: String,
    pub repo_id: String,
    pub start_line: i64,
    pub end_line: i64,
}

/// Insert or replace a file record.
pub fn insert_file(conn: &Connection, file: &FileRecord) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO files (path, repo_id, mtime, size, sha256, indexed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![file.path, file.repo_id, file.mtime, file.size, file.sha256, file.indexed_at],
    )?;
    Ok(())
}

/// Delete a file by path.
pub fn delete_file(conn: &Connection, path: &str) -> Result<usize> {
    let count = conn.execute("DELETE FROM files WHERE path = ?1", [path])?;
    Ok(count)
}

/// Get a file by path.
pub fn get_file_by_path(conn: &Connection, path: &str) -> Result<Option<FileRecord>> {
    conn.query_row(
        "SELECT path, repo_id, mtime, size, sha256, indexed_at FROM files WHERE path = ?1",
        [path],
        |row| {
            Ok(FileRecord {
                path: row.get(0)?,
                repo_id: row.get(1)?,
                mtime: row.get(2)?,
                size: row.get(3)?,
                sha256: row.get(4)?,
                indexed_at: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

/// List files by repo_id.
pub fn list_files_by_repo(conn: &Connection, repo_id: &str) -> Result<Vec<FileRecord>> {
    let mut stmt = conn.prepare(
        "SELECT path, repo_id, mtime, size, sha256, indexed_at FROM files WHERE repo_id = ?1",
    )?;
    let rows = stmt.query_map([repo_id], |row| {
        Ok(FileRecord {
            path: row.get(0)?,
            repo_id: row.get(1)?,
            mtime: row.get(2)?,
            size: row.get(3)?,
            sha256: row.get(4)?,
            indexed_at: row.get(5)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

/// Insert an FTS chunk and return its rowid.
#[allow(clippy::too_many_arguments)]
pub fn insert_chunk(
    conn: &Connection,
    content: &str,
    symbol_name: &str,
    symbol_type: &str,
    file_path: &str,
    repo_id: &str,
    start_line: i64,
    end_line: i64,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO chunks_fts (content, symbol_name, symbol_type, file_path, repo_id, start_line, end_line) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![content, symbol_name, symbol_type, file_path, repo_id, start_line, end_line],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Delete chunks by file path.
pub fn delete_chunks_by_file(conn: &Connection, file_path: &str) -> Result<usize> {
    let count = conn.execute("DELETE FROM chunks_fts WHERE file_path = ?1", [file_path])?;
    Ok(count)
}

/// Search FTS chunks.
pub fn search_fts(conn: &Connection, query: &str, limit: usize) -> Result<Vec<FtsChunk>> {
    let mut stmt = conn.prepare(
        "SELECT rowid, rank, content, symbol_name, symbol_type, file_path, repo_id, start_line, end_line FROM chunks_fts WHERE chunks_fts MATCH ?1 ORDER BY rank LIMIT ?2"
    )?;
    let rows = stmt.query_map(rusqlite::params![query, limit as i64], |row| {
        Ok(FtsChunk {
            rowid: row.get(0)?,
            rank: row.get(1)?,
            content: row.get(2)?,
            symbol_name: row.get(3)?,
            symbol_type: row.get(4)?,
            file_path: row.get(5)?,
            repo_id: row.get(6)?,
            start_line: row.get(7)?,
            end_line: row.get(8)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

/// Search FTS chunks within a repo.
pub fn search_fts_repo(
    conn: &Connection,
    repo_id: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<FtsChunk>> {
    let mut stmt = conn.prepare(
        "SELECT rowid, rank, content, symbol_name, symbol_type, file_path, repo_id, start_line, end_line FROM chunks_fts WHERE repo_id = ?1 AND chunks_fts MATCH ?2 ORDER BY rank LIMIT ?3"
    )?;
    let rows = stmt.query_map(rusqlite::params![repo_id, query, limit as i64], |row| {
        Ok(FtsChunk {
            rowid: row.get(0)?,
            rank: row.get(1)?,
            content: row.get(2)?,
            symbol_name: row.get(3)?,
            symbol_type: row.get(4)?,
            file_path: row.get(5)?,
            repo_id: row.get(6)?,
            start_line: row.get(7)?,
            end_line: row.get(8)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

/// Insert or replace a vector embedding.
pub fn insert_vector(conn: &Connection, chunk_id: i64, vec: &[u8]) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO vectors (chunk_id, vec) VALUES (?1, ?2)",
        rusqlite::params![chunk_id, vec.to_vec()],
    )?;
    Ok(())
}

/// A job record tracking indexing progress.
#[derive(Debug, Clone)]
pub struct JobRecord {
    pub id: String,
    pub repo_id: String,
    pub status: String,
    pub created_at: f64,
    pub completed_at: Option<f64>,
    pub error: Option<String>,
    pub nodes_indexed: Option<i64>,
    pub edges_indexed: Option<i64>,
}

/// Insert a new job record.
pub fn insert_job(conn: &Connection, job: &JobRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO jobs (id, repo_id, status, created_at, completed_at, error, nodes_indexed, edges_indexed) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![job.id, job.repo_id, job.status, job.created_at, job.completed_at, job.error, job.nodes_indexed, job.edges_indexed],
    )?;
    Ok(())
}

/// Update a job's status and completion fields.
pub fn update_job_status(
    conn: &Connection,
    id: &str,
    status: &str,
    completed_at: Option<f64>,
    error: Option<&str>,
    nodes_indexed: Option<i64>,
) -> Result<()> {
    conn.execute(
        "UPDATE jobs SET status = ?1, completed_at = ?2, error = ?3, nodes_indexed = ?4 WHERE id = ?5",
        rusqlite::params![status, completed_at, error, nodes_indexed, id],
    )?;
    Ok(())
}

/// Get a job by ID.
pub fn get_job(conn: &Connection, id: &str) -> Result<Option<JobRecord>> {
    conn.query_row(
        "SELECT id, repo_id, status, created_at, completed_at, error, nodes_indexed, edges_indexed FROM jobs WHERE id = ?1",
        [id],
        |row| {
            Ok(JobRecord {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                status: row.get(2)?,
                created_at: row.get(3)?,
                completed_at: row.get(4)?,
                error: row.get(5)?,
                nodes_indexed: row.get(6)?,
                edges_indexed: row.get(7)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

/// List all repos with their file counts.
pub fn list_repos(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare("SELECT repo_id, COUNT(*) as files FROM files GROUP BY repo_id")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

/// Delete vectors by chunk id.
pub fn delete_vector_by_chunk(conn: &Connection, chunk_id: i64) -> Result<usize> {
    let count = conn.execute("DELETE FROM vectors WHERE chunk_id = ?1", [chunk_id])?;
    Ok(count)
}

/// Get vector by chunk id.
pub fn get_vector_by_chunk(conn: &Connection, chunk_id: i64) -> Result<Option<Vec<u8>>> {
    conn.query_row(
        "SELECT vec FROM vectors WHERE chunk_id = ?1",
        [chunk_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn test_init_schema() {
        let conn = in_memory();
        init_local_schema(&conn).unwrap();
        let version: i32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn test_insert_and_get_file() {
        let conn = in_memory();
        init_local_schema(&conn).unwrap();
        let file = FileRecord {
            path: "src/main.rs".to_string(),
            repo_id: "repo1".to_string(),
            mtime: 1234.0,
            size: 42,
            sha256: "abc".to_string(),
            indexed_at: 5678.0,
        };
        insert_file(&conn, &file).unwrap();
        let got = get_file_by_path(&conn, "src/main.rs").unwrap().unwrap();
        assert_eq!(got.path, "src/main.rs");
        assert_eq!(got.size, 42);
    }

    #[test]
    fn test_delete_file() {
        let conn = in_memory();
        init_local_schema(&conn).unwrap();
        let file = FileRecord {
            path: "a.rs".to_string(),
            repo_id: "repo1".to_string(),
            mtime: 0.0,
            size: 0,
            sha256: "x".to_string(),
            indexed_at: 0.0,
        };
        insert_file(&conn, &file).unwrap();
        let count = delete_file(&conn, "a.rs").unwrap();
        assert_eq!(count, 1);
        assert!(get_file_by_path(&conn, "a.rs").unwrap().is_none());
    }

    #[test]
    fn test_list_files_by_repo() {
        let conn = in_memory();
        init_local_schema(&conn).unwrap();
        insert_file(
            &conn,
            &FileRecord {
                path: "a.rs".to_string(),
                repo_id: "repo1".to_string(),
                mtime: 0.0,
                size: 1,
                sha256: "a".to_string(),
                indexed_at: 0.0,
            },
        )
        .unwrap();
        insert_file(
            &conn,
            &FileRecord {
                path: "b.rs".to_string(),
                repo_id: "repo1".to_string(),
                mtime: 0.0,
                size: 2,
                sha256: "b".to_string(),
                indexed_at: 0.0,
            },
        )
        .unwrap();
        insert_file(
            &conn,
            &FileRecord {
                path: "c.rs".to_string(),
                repo_id: "repo2".to_string(),
                mtime: 0.0,
                size: 3,
                sha256: "c".to_string(),
                indexed_at: 0.0,
            },
        )
        .unwrap();
        let files = list_files_by_repo(&conn, "repo1").unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_insert_and_search_chunks() {
        let conn = in_memory();
        init_local_schema(&conn).unwrap();
        insert_chunk(
            &conn,
            "hello world",
            "main",
            "fn",
            "src/main.rs",
            "repo1",
            1,
            10,
        )
        .unwrap();
        insert_chunk(&conn, "foo bar", "foo", "fn", "src/foo.rs", "repo1", 1, 5).unwrap();
        let results = search_fts(&conn, "hello", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "hello world");
    }

    #[test]
    fn test_search_repo_filter() {
        let conn = in_memory();
        init_local_schema(&conn).unwrap();
        insert_chunk(
            &conn,
            "hello world",
            "main",
            "fn",
            "src/main.rs",
            "repo1",
            1,
            10,
        )
        .unwrap();
        insert_chunk(
            &conn,
            "hello moon",
            "main",
            "fn",
            "src/main.rs",
            "repo2",
            1,
            10,
        )
        .unwrap();
        let results = search_fts_repo(&conn, "repo1", "hello", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].repo_id, "repo1");
    }

    #[test]
    fn test_insert_and_get_vector() {
        let conn = in_memory();
        init_local_schema(&conn).unwrap();
        let vec = vec![1u8, 2, 3, 4];
        insert_vector(&conn, 1, &vec).unwrap();
        let got = get_vector_by_chunk(&conn, 1).unwrap().unwrap();
        assert_eq!(got, vec);
    }

    #[test]
    fn test_delete_vector() {
        let conn = in_memory();
        init_local_schema(&conn).unwrap();
        insert_vector(&conn, 1, &[1, 2]).unwrap();
        let count = delete_vector_by_chunk(&conn, 1).unwrap();
        assert_eq!(count, 1);
        assert!(get_vector_by_chunk(&conn, 1).unwrap().is_none());
    }

    #[test]
    fn test_list_repos() {
        let conn = in_memory();
        init_local_schema(&conn).unwrap();
        insert_file(
            &conn,
            &FileRecord {
                path: "a.rs".to_string(),
                repo_id: "repo1".to_string(),
                mtime: 0.0,
                size: 1,
                sha256: "a".to_string(),
                indexed_at: 0.0,
            },
        )
        .unwrap();
        insert_file(
            &conn,
            &FileRecord {
                path: "b.rs".to_string(),
                repo_id: "repo1".to_string(),
                mtime: 0.0,
                size: 2,
                sha256: "b".to_string(),
                indexed_at: 0.0,
            },
        )
        .unwrap();
        insert_file(
            &conn,
            &FileRecord {
                path: "c.rs".to_string(),
                repo_id: "repo2".to_string(),
                mtime: 0.0,
                size: 3,
                sha256: "c".to_string(),
                indexed_at: 0.0,
            },
        )
        .unwrap();
        let repos = list_repos(&conn).unwrap();
        assert_eq!(repos.len(), 2);
        let repo1 = repos.iter().find(|(id, _)| id == "repo1").unwrap();
        assert_eq!(repo1.1, 2);
        let repo2 = repos.iter().find(|(id, _)| id == "repo2").unwrap();
        assert_eq!(repo2.1, 1);
    }
}
