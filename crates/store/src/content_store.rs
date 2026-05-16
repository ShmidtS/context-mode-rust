use crate::chunking::{chunk_markdown, chunk_plain_text, walk_json};
use crate::reranking::apply_proximity_reranking;
use crate::schema::{PREPARED_STATEMENTS, init_schema};
use crate::search_helpers::{
    SearchCoreParams, SearchStmts, sanitize_query, sanitize_trigram_query,
    search_core,
};
use crate::types::{
    AstChunk, Chunk, IndexOptions, IndexResult, MatchLayer, SearchMode, SearchResult,
    SourceListItem, SourceMatchMode, SourceMeta, StoreStats,
};
use crate::vocabulary::{FuzzyCache, extract_and_store_vocabulary, fuzzy_correct};
use chrono::Utc;
use context_mode_core::ContentType;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("either content or path must be provided")]
    MissingContent,
}

pub type StoreResult<T> = Result<T, StoreError>;

#[derive(Debug)]
pub struct ContentStore {
    conn: Connection,
    db_path: Option<PathBuf>,
    insert_count: usize,
    fuzzy_cache: FuzzyCache,
    pub last_refresh_count: usize,
}

impl ContentStore {
    pub const OPTIMIZE_EVERY: usize = 50;
    pub const FUZZY_CACHE_SIZE: usize = 256;

    pub fn new(conn: Connection) -> StoreResult<Self> {
        init_schema(&conn)?;
        Ok(Self {
            conn,
            db_path: None,
            insert_count: 0,
            fuzzy_cache: FuzzyCache::new(),
            last_refresh_count: 0,
        })
    }

    pub fn open<P: AsRef<Path>>(db_path: P) -> StoreResult<Self> {
        let path = db_path.as_ref().to_path_buf();
        let conn = Connection::open(&path)?;
        conn.busy_timeout(std::time::Duration::from_secs(30))?;
        apply_wal_pragmas(&conn)?;
        init_schema(&conn)?;
        Ok(Self {
            conn,
            db_path: Some(path),
            insert_count: 0,
            fuzzy_cache: FuzzyCache::new(),
            last_refresh_count: 0,
        })
    }

    pub fn in_memory() -> StoreResult<Self> {
        Self::new(Connection::open_in_memory()?)
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn index(&mut self, options: IndexOptions) -> StoreResult<IndexResult> {
        let has_content = options
            .content
            .as_ref()
            .is_some_and(|content| !content.is_empty());
        if !has_content && options.path.is_none() {
            return Err(StoreError::MissingContent);
        }

        let text = if has_content {
            options.content.unwrap_or_default()
        } else {
            let path = options.path.as_ref().ok_or(StoreError::MissingContent)?;
            fs::read_to_string(path)?
        };
        let label = options
            .source
            .or_else(|| options.path.clone())
            .unwrap_or_else(|| "untitled".to_string());
        let chunks = chunk_markdown(&text, None);
        let content_hash = options.path.as_ref().map(|_| hash_content(&text));
        self.insert_chunks(
            &chunks,
            &label,
            &text,
            options.path.as_deref(),
            content_hash.as_deref(),
        )
    }

    pub fn index_content(
        &mut self,
        content: impl Into<String>,
        source: impl Into<String>,
    ) -> StoreResult<IndexResult> {
        self.index(IndexOptions {
            content: Some(content.into()),
            path: None,
            source: Some(source.into()),
        })
    }

    pub fn index_path<P: AsRef<Path>>(
        &mut self,
        path: P,
        source: Option<String>,
    ) -> StoreResult<IndexResult> {
        self.index(IndexOptions {
            content: None,
            path: Some(path.as_ref().to_string_lossy().to_string()),
            source,
        })
    }

    pub fn index_plain_text(
        &mut self,
        content: &str,
        source: &str,
        lines_per_chunk: usize,
    ) -> StoreResult<IndexResult> {
        if content.trim().is_empty() {
            return self.insert_chunks(&[], source, "", None, None);
        }
        let chunks: Vec<Chunk> = chunk_plain_text(content, lines_per_chunk)
            .into_iter()
            .map(|chunk| Chunk {
                title: chunk.title,
                content: chunk.content,
                has_code: false,
            })
            .collect();
        self.insert_chunks(&chunks, source, content, None, None)
    }

    pub fn index_json(
        &mut self,
        content: &str,
        source: &str,
        max_chunk_bytes: usize,
    ) -> StoreResult<IndexResult> {
        if content.trim().is_empty() {
            return self.index_plain_text("", source, 20);
        }
        let parsed: serde_json::Value = match serde_json::from_str(content) {
            Ok(parsed) => parsed,
            Err(_) => return self.index_plain_text(content, source, 20),
        };
        let mut chunks = Vec::new();
        walk_json(&parsed, &[], &mut chunks, max_chunk_bytes);
        if chunks.is_empty() {
            return self.index_plain_text(content, source, 20);
        }
        self.insert_chunks(&chunks, source, content, None, None)
    }

    pub fn index_code_chunks(
        &mut self,
        chunks: &[AstChunk],
        source_path: &str,
        source_type: &str,
    ) -> StoreResult<IndexResult> {
        if chunks.is_empty() {
            return Ok(IndexResult {
                source_id: 0,
                label: source_path.to_string(),
                total_chunks: 0,
                code_chunks: 0,
            });
        }

        let label = format!("code:{source_path}");
        let tx = self.conn.transaction()?;
        tx.execute(
            PREPARED_STATEMENTS.stmt_delete_chunks_by_label,
            params![label],
        )?;
        tx.execute(
            PREPARED_STATEMENTS.stmt_delete_chunks_trigram_by_label,
            params![label],
        )?;
        tx.execute(
            PREPARED_STATEMENTS.stmt_delete_sources_by_label,
            params![label],
        )?;
        tx.execute(
            PREPARED_STATEMENTS.stmt_insert_source,
            params![
                label,
                chunks.len() as i64,
                chunks.len() as i64,
                source_path,
                Option::<String>::None
            ],
        )?;
        let source_id = tx.last_insert_rowid();
        let now = Utc::now().to_rfc3339();
        for chunk in chunks {
            let content = format!(
                "{}\n// symbol:{} kind:{} lines:{}-{}",
                chunk.content,
                chunk.symbol_name.clone().unwrap_or_default(),
                chunk.symbol_kind.clone().unwrap_or_default(),
                chunk.line_start.map(|v| v.to_string()).unwrap_or_default(),
                chunk.line_end.map(|v| v.to_string()).unwrap_or_default()
            );
            tx.execute(
                PREPARED_STATEMENTS.stmt_insert_chunk,
                params![
                    chunk.title,
                    content,
                    source_id,
                    "code",
                    source_type,
                    Option::<String>::None,
                    Option::<String>::None,
                    now
                ],
            )?;
            tx.execute(
                PREPARED_STATEMENTS.stmt_insert_chunk_trigram,
                params![
                    chunk.title,
                    content,
                    source_id,
                    "code",
                    source_type,
                    Option::<String>::None,
                    Option::<String>::None,
                    now
                ],
            )?;
        }
        tx.commit()?;
        self.after_insert()?;
        Ok(IndexResult {
            source_id,
            label,
            total_chunks: chunks.len(),
            code_chunks: chunks.len(),
        })
    }

    fn insert_chunks(
        &mut self,
        chunks: &[Chunk],
        label: &str,
        text: &str,
        file_path: Option<&str>,
        content_hash: Option<&str>,
    ) -> StoreResult<IndexResult> {
        let code_chunks = chunks.iter().filter(|chunk| chunk.has_code).count();
        let tx = self.conn.transaction()?;
        tx.execute(
            PREPARED_STATEMENTS.stmt_delete_chunks_by_label,
            params![label],
        )?;
        tx.execute(
            PREPARED_STATEMENTS.stmt_delete_chunks_trigram_by_label,
            params![label],
        )?;
        tx.execute(
            PREPARED_STATEMENTS.stmt_delete_sources_by_label,
            params![label],
        )?;
        tx.execute(
            PREPARED_STATEMENTS.stmt_insert_source,
            params![
                label,
                chunks.len() as i64,
                code_chunks as i64,
                file_path,
                content_hash
            ],
        )?;
        let source_id = tx.last_insert_rowid();
        let now = Utc::now().to_rfc3339();
        for chunk in chunks {
            let content_type = if chunk.has_code { "code" } else { "prose" };
            tx.execute(
                PREPARED_STATEMENTS.stmt_insert_chunk,
                params![
                    chunk.title,
                    chunk.content,
                    source_id,
                    content_type,
                    Option::<String>::None,
                    Option::<String>::None,
                    Option::<String>::None,
                    now
                ],
            )?;
            tx.execute(
                PREPARED_STATEMENTS.stmt_insert_chunk_trigram,
                params![
                    chunk.title,
                    chunk.content,
                    source_id,
                    content_type,
                    Option::<String>::None,
                    Option::<String>::None,
                    Option::<String>::None,
                    now
                ],
            )?;
        }
        tx.commit()?;

        if !text.is_empty() {
            extract_and_store_vocabulary(&self.conn, text, &mut self.fuzzy_cache)?;
        }
        self.after_insert()?;

        Ok(IndexResult {
            source_id,
            label: label.to_string(),
            total_chunks: chunks.len(),
            code_chunks,
        })
    }

    fn after_insert(&mut self) -> StoreResult<()> {
        self.insert_count += 1;
        if self.insert_count % Self::OPTIMIZE_EVERY == 0 {
            self.optimize_fts()?;
        }
        Ok(())
    }

    pub fn search(
        &self,
        query: &str,
        limit: usize,
        source: Option<&str>,
        mode: SearchMode,
        content_type: Option<ContentType>,
        source_match_mode: SourceMatchMode,
    ) -> StoreResult<Vec<SearchResult>> {
        Ok(search_core(SearchCoreParams {
            conn: &self.conn,
            query,
            limit,
            source,
            mode,
            content_type,
            source_match_mode,
            sanitize: sanitize_query,
            stmts: &SearchStmts::for_table(crate::schema::FtsTable::Chunks),
            allow_empty: true,
        })?)
    }

    pub fn search_trigram(
        &self,
        query: &str,
        limit: usize,
        source: Option<&str>,
        mode: SearchMode,
        content_type: Option<ContentType>,
        source_match_mode: SourceMatchMode,
    ) -> StoreResult<Vec<SearchResult>> {
        Ok(search_core(SearchCoreParams {
            conn: &self.conn,
            query,
            limit,
            source,
            mode,
            content_type,
            source_match_mode,
            sanitize: sanitize_trigram_query,
            stmts: &SearchStmts::for_table(crate::schema::FtsTable::ChunksTrigram),
            allow_empty: false,
        })?)
    }

    pub fn fuzzy_correct(&mut self, query: &str) -> StoreResult<Option<String>> {
        Ok(fuzzy_correct(
            &self.conn,
            &mut self.fuzzy_cache,
            Self::FUZZY_CACHE_SIZE,
            query,
        )?)
    }

    pub fn search_with_fallback(
        &mut self,
        query: &str,
        limit: usize,
        source: Option<&str>,
        content_type: Option<ContentType>,
        source_match_mode: SourceMatchMode,
    ) -> StoreResult<Vec<SearchResult>> {
        self.refresh_stale_sources()?;
        let rrf_results = self.rrf_search(
            query,
            limit,
            source,
            content_type.clone(),
            source_match_mode,
        )?;
        if !rrf_results.is_empty() {
            return Ok(apply_proximity_reranking(&rrf_results, query)
                .into_iter()
                .map(|mut result| {
                    result.match_layer = Some(MatchLayer::Rrf);
                    result
                })
                .collect());
        }

        let words: Vec<String> = query
            .to_lowercase()
            .split_whitespace()
            .filter(|word| word.len() >= 3 && !crate::types::is_stopword(word))
            .map(ToString::to_string)
            .collect();
        let original = words.join(" ");
        let mut corrected_words = Vec::new();
        for word in words {
            corrected_words.push(self.fuzzy_correct(&word)?.unwrap_or(word));
        }
        let corrected_query = corrected_words.join(" ");
        if corrected_query != original {
            let fuzzy_results = self.rrf_search(
                &corrected_query,
                limit,
                source,
                content_type,
                source_match_mode,
            )?;
            if !fuzzy_results.is_empty() {
                return Ok(apply_proximity_reranking(&fuzzy_results, &corrected_query)
                    .into_iter()
                    .map(|mut result| {
                        result.match_layer = Some(MatchLayer::RrfFuzzy);
                        result
                    })
                    .collect());
            }
        }
        Ok(Vec::new())
    }

    fn rrf_search(
        &self,
        query: &str,
        limit: usize,
        source: Option<&str>,
        content_type: Option<ContentType>,
        source_match_mode: SourceMatchMode,
    ) -> StoreResult<Vec<SearchResult>> {
        let fetch_limit = (limit * 2).max(10);
        let porter = self.search(
            query,
            fetch_limit,
            source,
            SearchMode::Or,
            content_type.clone(),
            source_match_mode,
        )?;
        let trigram = self.search_trigram(
            query,
            fetch_limit,
            source,
            SearchMode::Or,
            content_type,
            source_match_mode,
        )?;
        let fused = reciprocal_rank_fuse(&[porter, trigram]);
        Ok(fused.into_iter().take(limit).collect())
    }

    fn refresh_stale_sources(&mut self) -> StoreResult<()> {
        self.last_refresh_count = 0;
        let sources: Vec<(String, String, Option<String>, String)> = {
            let mut stmt = self.conn.prepare("SELECT label, file_path, content_hash, indexed_at FROM sources WHERE file_path IS NOT NULL")?;
            stmt.query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<rusqlite::Result<_>>()?
        };

        for (label, file_path, content_hash, indexed_at) in sources {
            let path = Path::new(&file_path);
            if !path.exists() {
                continue;
            }
            let metadata = fs::metadata(path)?;
            let Ok(modified) = metadata.modified() else {
                continue;
            };
            let indexed_at = chrono::DateTime::parse_from_rfc3339(&indexed_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            if chrono::DateTime::<Utc>::from(modified) <= indexed_at {
                continue;
            }
            let text = fs::read_to_string(path)?;
            let new_hash = hash_content(&text);
            if content_hash.as_deref() == Some(new_hash.as_str()) {
                continue;
            }
            self.index(IndexOptions {
                content: Some(text),
                path: Some(file_path),
                source: Some(label),
            })?;
            self.last_refresh_count += 1;
        }
        Ok(())
    }

    pub fn get_source_meta(&self, label: &str) -> StoreResult<Option<SourceMeta>> {
        Ok(self
            .conn
            .query_row(
                PREPARED_STATEMENTS.stmt_source_meta,
                params![label],
                |row| {
                    Ok(SourceMeta {
                        label: row.get(0)?,
                        chunk_count: row.get::<_, i64>(1)? as usize,
                        code_chunk_count: row.get::<_, i64>(2)? as usize,
                        indexed_at: row.get(3)?,
                        file_path: row.get(4)?,
                        content_hash: row.get(5)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn list_sources(&self) -> StoreResult<Vec<SourceListItem>> {
        let mut stmt = self.conn.prepare(PREPARED_STATEMENTS.stmt_list_sources)?;
        Ok(stmt
            .query_map([], |row| {
                Ok(SourceListItem {
                    label: row.get(0)?,
                    chunk_count: row.get::<_, i64>(1)? as usize,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn get_chunks_by_source(&self, source_id: i64) -> StoreResult<Vec<SearchResult>> {
        let mut stmt = self
            .conn
            .prepare(PREPARED_STATEMENTS.stmt_chunks_by_source)?;
        Ok(stmt
            .query_map(params![source_id], |row| {
                let content_type: String = row.get(2)?;
                Ok(SearchResult {
                    title: row.get(0)?,
                    content: row.get(1)?,
                    source: row.get(3)?,
                    rank: 0.0,
                    content_type: if content_type == "code" {
                        ContentType::Code
                    } else {
                        ContentType::Prose
                    },
                    match_layer: None,
                    highlighted: None,
                    timestamp: None,
                    confidence: None,
                    confidence_source: None,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn get_distinctive_terms(
        &self,
        source_id: i64,
        max_terms: usize,
    ) -> StoreResult<Vec<String>> {
        let total_chunks: Option<i64> = self
            .conn
            .query_row(
                PREPARED_STATEMENTS.stmt_source_chunk_count,
                params![source_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(total_chunks) = total_chunks else {
            return Ok(Vec::new());
        };
        if total_chunks < 3 {
            return Ok(Vec::new());
        }

        let mut doc_freq: HashMap<String, usize> = HashMap::new();
        let token_re = regex::Regex::new(r"[^\p{L}\p{N}_-]+").expect("valid token regex");
        let mut stmt = self.conn.prepare(PREPARED_STATEMENTS.stmt_chunk_content)?;
        let rows = stmt.query_map(params![source_id], |row| row.get::<_, String>(0))?;
        for row in rows {
            let content = row?;
            let mut words: Vec<String> = token_re
                .split(&content.to_lowercase())
                .filter(|word| word.len() >= 3 && !crate::types::is_stopword(word))
                .map(ToString::to_string)
                .collect();
            words.sort();
            words.dedup();
            for word in words {
                *doc_freq.entry(word).or_default() += 1;
            }
        }

        let total = total_chunks as f64;
        let max_appearances = 3usize.max((total_chunks as f64 * 0.4).ceil() as usize);
        let mut scored: Vec<(String, f64)> = doc_freq
            .into_iter()
            .filter(|(_, count)| *count >= 2 && *count <= max_appearances)
            .map(|(word, count)| {
                let idf = (total / count as f64).ln();
                let len_bonus = (word.len() as f64 / 20.0).min(0.5);
                let identifier_bonus = if word.contains('_') {
                    1.5
                } else if word.len() >= 12 {
                    0.8
                } else {
                    0.0
                };
                let score = idf + len_bonus + identifier_bonus;
                (word, score)
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scored
            .into_iter()
            .take(max_terms)
            .map(|(word, _)| word)
            .collect())
    }

    pub fn get_stats(&self) -> StoreResult<StoreStats> {
        Ok(self
            .conn
            .query_row(PREPARED_STATEMENTS.stmt_stats, [], |row| {
                Ok(StoreStats {
                    sources: row.get::<_, i64>(0)? as usize,
                    chunks: row.get::<_, i64>(1)? as usize,
                    code_chunks: row.get::<_, i64>(2)? as usize,
                })
            })?)
    }

    pub fn cleanup_stale_sources(&mut self, max_age_days: u64) -> StoreResult<usize> {
        let tx = self.conn.transaction()?;
        tx.execute(
            PREPARED_STATEMENTS.stmt_cleanup_chunks,
            params![max_age_days as i64],
        )?;
        tx.execute(
            PREPARED_STATEMENTS.stmt_cleanup_chunks_trigram,
            params![max_age_days as i64],
        )?;
        let deleted = tx.execute(
            PREPARED_STATEMENTS.stmt_cleanup_sources,
            params![max_age_days as i64],
        )?;
        tx.commit()?;
        Ok(deleted)
    }

    pub fn get_db_size_bytes(&self) -> u64 {
        self.db_path
            .as_ref()
            .and_then(|path| fs::metadata(path).ok())
            .map_or(0, |metadata| metadata.len())
    }

    pub fn optimize_fts(&self) -> StoreResult<()> {
        self.conn.execute_batch("INSERT INTO chunks(chunks) VALUES('optimize'); INSERT INTO chunks_trigram(chunks_trigram) VALUES('optimize');")?;
        Ok(())
    }

    pub fn close(self) -> StoreResult<()> {
        self.optimize_fts()?;
        Ok(())
    }

    pub fn cleanup(self) -> StoreResult<()> {
        let db_path = self.db_path.clone();
        drop(self);
        if let Some(path) = db_path {
            delete_db_files(path)?;
        }
        Ok(())
    }
}

fn reciprocal_rank_fuse(result_sets: &[Vec<SearchResult>]) -> Vec<SearchResult> {
    let mut by_key: HashMap<String, (SearchResult, f64)> = HashMap::new();
    for results in result_sets {
        for (idx, result) in results.iter().enumerate() {
            let score = 1.0 / (60.0 + idx as f64 + 1.0);
            let key = format!("{}::{}", result.source, result.title);
            by_key
                .entry(key)
                .and_modify(|(_, existing)| *existing += score)
                .or_insert_with(|| (result.clone(), score));
        }
    }
    let mut fused: Vec<(SearchResult, f64)> = by_key.into_values().collect();
    fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    fused
        .into_iter()
        .map(|(mut result, score)| {
            result.rank = -score;
            result
        })
        .collect()
}

fn hash_content(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

fn apply_wal_pragmas(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "busy_timeout", 30_000i64)?;
    Ok(())
}

fn delete_db_files<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    let path = path.as_ref();
    for suffix in ["", "-wal", "-shm"] {
        let candidate = PathBuf::from(format!("{}{}", path.display(), suffix));
        match fs::remove_file(candidate) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

pub fn cleanup_stale_dbs() -> usize {
    let Ok(entries) = fs::read_dir(std::env::temp_dir()) else {
        return 0;
    };
    let current_pid = std::process::id();
    let re = regex::Regex::new(r"^context-mode-(\d+)\.db$").expect("valid stale db regex");
    let mut cleaned = 0;
    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().to_string();
        let Some(caps) = re.captures(&file_name) else {
            continue;
        };
        let pid = caps
            .get(1)
            .and_then(|m| m.as_str().parse::<u32>().ok())
            .unwrap_or(current_pid);
        if pid == current_pid {
            continue;
        }
        if delete_db_files(entry.path()).is_ok() {
            cleaned += 1;
        }
    }
    cleaned
}

pub fn cleanup_stale_content_dbs<P: AsRef<Path>>(content_dir: P, max_age_days: u64) -> usize {
    let Ok(entries) = fs::read_dir(content_dir) else {
        return 0;
    };
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(max_age_days * 24 * 60 * 60))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    let mut cleaned = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("db") {
            continue;
        }
        let should_clean = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .is_ok_and(|modified| modified < cutoff);
        let wal_stale = fs::metadata(format!("{}-wal", path.display()))
            .and_then(|metadata| {
                metadata
                    .modified()
                    .map(|modified| (metadata.len(), modified))
            })
            .is_ok_and(|(len, modified)| {
                len > 0
                    && modified
                        .elapsed()
                        .is_ok_and(|elapsed| elapsed.as_secs() > 3600)
            });
        if (should_clean || wal_stale) && delete_db_files(&path).is_ok() {
            cleaned += 1;
        }
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_store_indexes_and_searches_markdown() {
        let mut store = ContentStore::in_memory().unwrap();
        let indexed = store
            .index_content("# Rust API\nDatabase connection pooling", "docs")
            .unwrap();
        assert_eq!(indexed.total_chunks, 1);
        let results = store
            .search(
                "database",
                3,
                None,
                SearchMode::And,
                None,
                SourceMatchMode::Like,
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "docs");
    }

    #[test]
    fn content_store_replaces_existing_source_label() {
        let mut store = ContentStore::in_memory().unwrap();
        store.index_content("# Old\nalpha", "docs").unwrap();
        store.index_content("# New\nbeta", "docs").unwrap();
        assert!(
            store
                .search(
                    "alpha",
                    3,
                    None,
                    SearchMode::And,
                    None,
                    SourceMatchMode::Like
                )
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store
                .search(
                    "beta",
                    3,
                    None,
                    SearchMode::And,
                    None,
                    SourceMatchMode::Like
                )
                .unwrap()
                .len(),
            1
        );
    }
}
