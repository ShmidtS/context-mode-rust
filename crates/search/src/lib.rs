pub mod auto_memory;
pub mod context_packer;
pub mod rrf;
pub mod unified;
pub mod vector_store;

pub use auto_memory::{
    AutoMemoryExtraction, AutoMemoryOptions, AutoMemoryResult, MemoryPattern, extract_auto_memory,
    extract_auto_memory_with_options, results_to_auto_memory,
};
pub use context_packer::{ContextPacker, PackOptions, PackResult, PackedItem, pack_search_results};
pub use rrf::{DEFAULT_RRF_K, FusedId, RankedId, RrfScored, fuse_ranked_ids, reciprocal_rank_fuse};
pub use unified::{
    SearchOrigin, UnifiedSearchOptions, UnifiedSearchResult, estimate_tokens, fuse_search_results,
    fuse_unified_results,
};
pub use vector_store::{
    VectorRecord, VectorSearchResult, VectorStore, VectorStoreError, cosine_similarity,
};
