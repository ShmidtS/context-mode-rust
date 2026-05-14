pub mod chunking;
pub mod content_store;
pub mod reranking;
pub mod schema;
pub mod search_helpers;
pub mod types;
pub mod vocabulary;

pub use chunking::{PlainTextChunk, chunk_markdown, chunk_plain_text, walk_json};
pub use content_store::{
    ContentStore, StoreError, StoreResult, cleanup_stale_content_dbs, cleanup_stale_dbs,
};
pub use reranking::{
    apply_proximity_reranking, count_adjacent_pairs, find_all_positions, find_min_span,
};
pub use schema::{
    PREPARED_STATEMENTS, PreparedStatements, build_search_sql, init_schema, prepare_statements,
};
pub use search_helpers::{
    SearchStmts, map_search_rows, sanitize_query, sanitize_trigram_query, search_core,
    source_filter_param,
};
pub use types::*;
pub use vocabulary::{
    FuzzyCache, extract_and_store_vocabulary, extract_words, fuzzy_correct, levenshtein,
    max_edit_distance,
};
