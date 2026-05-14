use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::chunker::chunk_by_tokens;
use crate::db_schema;
use crate::db_schema::FileRecord;
use crate::embedding::EmbeddingProvider;
use crate::types::IndexResult;

/// Metadata collected from a file on disk before indexing.
#[derive(Debug, Clone, PartialEq)]
pub struct FileMeta {
    pub path: PathBuf,
    pub content_hash: String,
    pub size: u64,
    pub mtime: u64,
}

/// Walks a repository root and collects metadata for every non-ignorable file.
pub fn collect_file_metas(root: impl AsRef<Path>) -> Vec<FileMeta> {
    let mut metas = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        if crate::watcher::is_ignorable(path) {
            continue;
        }
        if let Ok(meta) = std::fs::metadata(path) {
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let size = meta.len();
            let content_hash = compute_file_hash(path).unwrap_or_default();
            metas.push(FileMeta {
                path: path.to_path_buf(),
                content_hash,
                size,
                mtime,
            });
        }
    }
    metas
}

/// Compute SHA-256 hash of file contents, returned as lowercase hex.
pub fn compute_file_hash(path: impl AsRef<Path>) -> anyhow::Result<String> {
    let data = std::fs::read(path)?;
    let hash = Sha256::digest(&data);
    Ok(hex::encode(hash))
}

/// Compare on-disk file list with DB state and return (to_add, to_remove).
pub fn diff_with_db(
    conn: &Connection,
    repo: &str,
    disk: &[FileMeta],
) -> anyhow::Result<(Vec<FileMeta>, Vec<String>)> {
    let db_files = db_schema::list_files_by_repo(conn, repo)?;
    let db_map: HashMap<String, String> =
        db_files.into_iter().map(|r| (r.path, r.sha256)).collect();

    let mut to_add = Vec::new();
    let mut to_remove = Vec::new();

    for meta in disk {
        let path_str = meta.path.to_string_lossy().into_owned();
        match db_map.get(&path_str) {
            Some(hash) if hash == &meta.content_hash => {
                // unchanged
            }
            Some(_) => {
                // changed: remove old chunks, re-index
                to_remove.push(path_str);
                to_add.push(meta.clone());
            }
            None => {
                to_add.push(meta.clone());
            }
        }
    }

    for path in db_map.keys() {
        if !disk.iter().any(|m| m.path.to_string_lossy() == *path) {
            to_remove.push(path.clone());
        }
    }

    Ok((to_add, to_remove))
}

fn block_on_future<F: std::future::Future>(f: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(f),
        Err(_) => tokio::runtime::Runtime::new().unwrap().block_on(f),
    }
}

/// Index a single file: chunk, insert into DB, store embeddings via `provider`.
pub fn index_file(
    conn: &mut Connection,
    repo: &str,
    file: &FileMeta,
    provider: &impl EmbeddingProvider,
) -> anyhow::Result<IndexResult> {
    let content = std::fs::read_to_string(&file.path).unwrap_or_default();
    let path_str = file.path.to_string_lossy().into_owned();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    let record = FileRecord {
        path: path_str.clone(),
        repo_id: repo.to_string(),
        mtime: file.mtime as f64,
        size: file.size as i64,
        sha256: file.content_hash.clone(),
        indexed_at: now,
    };
    db_schema::insert_file(conn, &record)?;

    let chunks = chunk_by_tokens(&content, 512);
    let mut total = 0usize;
    let mut code = 0usize;

    for chunk in chunks.iter() {
        let chunk_id =
            db_schema::insert_chunk(conn, &chunk.content, "", "", &path_str, repo, 0, 0)?;
        total += 1;
        code += 1;

        let texts = vec![chunk.content.clone()];
        let embeddings = block_on_future(provider.embed(&texts));
        let vec = embeddings.map_err(|e| anyhow::anyhow!("embed error: {}", e))?;
        let bytes: Vec<u8> = vec[0].iter().flat_map(|f| f.to_le_bytes()).collect();
        let _ = db_schema::insert_vector(conn, chunk_id, &bytes);
    }

    Ok(IndexResult {
        source_id: 0,
        label: path_str,
        total_chunks: total,
        code_chunks: code,
    })
}

/// Full repository indexing pipeline.
pub fn index_repository(
    conn: &mut Connection,
    repo: &str,
    root: impl AsRef<Path>,
    provider: &impl EmbeddingProvider,
) -> anyhow::Result<Vec<IndexResult>> {
    let metas = collect_file_metas(root);
    let (to_add, to_remove) = diff_with_db(conn, repo, &metas)?;

    for path in to_remove {
        let _ = db_schema::delete_file(conn, &path);
        let _ = db_schema::delete_chunks_by_file(conn, &path);
    }

    let mut results = Vec::new();
    for meta in to_add {
        match index_file(conn, repo, &meta, provider) {
            Ok(r) => results.push(r),
            Err(e) => tracing::warn!("failed to index {:?}: {}", meta.path, e),
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_conn() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        db_schema::init_local_schema(&mut conn).unwrap();
        conn
    }

    #[test]
    fn test_compute_file_hash() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"hello world").unwrap();
        let hash = compute_file_hash(tmp.path()).unwrap();
        assert_eq!(hash.len(), 64);
        assert!(!hash.contains(" "));
    }

    #[test]
    fn test_collect_and_diff() {
        let mut conn = make_conn();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let file_path = root.join("test.rs");
        std::fs::write(&file_path, b"fn main() {}").unwrap();

        let metas = collect_file_metas(&root);
        assert!(
            !metas.is_empty(),
            "metas should not be empty, root={:?}",
            root
        );

        let (to_add, to_remove) = diff_with_db(&conn, "test-repo", &metas).unwrap();
        assert!(!to_add.is_empty());
        assert!(to_remove.is_empty());

        // Simulate DB insert manually so diff sees it next time.
        for meta in &metas {
            let record = FileRecord {
                path: meta.path.to_string_lossy().into_owned(),
                repo_id: "test-repo".into(),
                mtime: meta.mtime as f64,
                size: meta.size as i64,
                sha256: meta.content_hash.clone(),
                indexed_at: 0.0,
            };
            db_schema::insert_file(&mut conn, &record).unwrap();
        }

        let (to_add2, to_remove2) = diff_with_db(&conn, "test-repo", &metas).unwrap();
        assert!(
            to_add2.is_empty(),
            "to_add2 should be empty but got {:?}",
            to_add2
        );
        assert!(
            to_remove2.is_empty(),
            "to_remove2 should be empty but got {:?}",
            to_remove2
        );
    }

    #[test]
    fn test_index_file_roundtrip() {
        let mut conn = make_conn();
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test.rs");
        std::fs::write(&file_path, b"fn foo() -> i32 { 42 }\nfn bar() {}").unwrap();

        let meta = FileMeta {
            path: file_path.clone(),
            content_hash: compute_file_hash(&file_path).unwrap(),
            size: 35,
            mtime: 0,
        };

        let provider = crate::embedding::ZeroEmbeddingProvider::new();
        let result = index_file(&mut conn, "repo", &meta, &provider).unwrap();
        assert_eq!(result.label, meta.path.to_string_lossy());
        assert!(result.total_chunks > 0);
    }

    #[test]
    fn test_index_repository() {
        let mut conn = make_conn();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let file_path = root.join("main.rs");
        std::fs::write(&file_path, b"fn main() {}").unwrap();

        let provider = crate::embedding::ZeroEmbeddingProvider::new();
        let results = index_repository(&mut conn, "repo", &root, &provider).unwrap();
        assert!(!results.is_empty(), "results should not be empty");
    }
}
