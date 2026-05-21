pub mod analytics;
pub mod ast_chunker;
pub mod ast_parser;
pub mod chunking;
pub mod code_parser;
pub mod config;
pub mod graph_store;
pub mod indexer;
pub mod markdown_parser;
pub mod path_utils;
pub mod resolver;
pub mod search;
pub mod symbol_graph;
pub mod types;

pub use analytics::{
    AnalyzeOpts, CommunityHint, GodNode, GraphAnalysisResult, SuggestedQuestion,
    SurprisingConnection, TokenEstimate, analyze_graph,
};
pub use code_parser::parse_code_file;
pub use config::{
    add_vault_config, get_vault_config, list_vault_configs, load_configs, load_vault_config,
    remove_vault_config, save_vault_config, vault_config_dir, vault_config_path,
};
pub use graph_store::VaultGraphStore;
pub use indexer::{IndexOpts, IndexResult, index_vault};
pub use markdown_parser::parse_vault_note;
pub use path_utils::*;
pub use resolver::resolve_link;
pub use search::{FusionSearchOpts, VaultGraphSearch};
pub use types::*;
