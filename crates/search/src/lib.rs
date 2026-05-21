pub mod auto_memory;
pub mod context_packer;
pub mod hybrid;
pub mod query;
pub mod ranking;
pub mod rrf;
pub mod unified;
pub mod vector_store;

pub use auto_memory::{
    AutoMemoryExtraction, AutoMemoryOptions, AutoMemoryResult, MemoryPattern, extract_auto_memory,
    extract_auto_memory_with_options, results_to_auto_memory,
};
pub use context_packer::{ContextPacker, PackOptions, PackResult, PackedItem, pack_search_results};
pub use hybrid::{HybridRankedId, HybridScore, hybrid_rrf, hybrid_rrf_pairs, rrf_score};
pub use query::{
    QueryClassification, classify_query, is_symbol_like, split_identifier, split_query_identifiers,
};
pub use ranking::{
    EXACT_IDENTIFIER_BOOST, GENERATED_FILE_PENALTY, PATH_MATCH_BOOST, RankedSearchResult,
    TEST_FILE_PENALTY, apply_rank_modifiers, deduplicate_file_chunks, has_exact_identifier_match,
    has_path_match, is_generated_file, is_test_file, rank_results,
};
pub use rrf::{DEFAULT_RRF_K, FusedId, RankedId, RrfScored, fuse_ranked_ids, reciprocal_rank_fuse};
pub use unified::{
    SearchOrigin, UnifiedSearchOptions, UnifiedSearchResult, estimate_tokens, fuse_search_results,
    fuse_unified_results,
};
pub use vector_store::{
    VectorRecord, VectorSearchResult, VectorStore, VectorStoreError, cosine_similarity,
};
