use anyhow::Result;

#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
}

pub fn register_all_tools() -> Result<()> {
    Ok(())
}

pub fn get_all_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "ctx_execute".into(),
            description: "Execute code in sandboxed subprocess (11 languages)".into(),
        },
        ToolDef {
            name: "ctx_execute_file".into(),
            description: "Read a file and process it in sandbox".into(),
        },
        ToolDef {
            name: "ctx_batch_execute".into(),
            description: "Execute multiple commands in parallel and auto-index".into(),
        },
        ToolDef {
            name: "ctx_index".into(),
            description: "Index content into searchable knowledge base".into(),
        },
        ToolDef {
            name: "ctx_search".into(),
            description: "Search indexed content with BM25 ranking".into(),
        },
        ToolDef {
            name: "ctx_fetch_and_index".into(),
            description: "Fetch URL and index content".into(),
        },
        ToolDef {
            name: "ctx_stats".into(),
            description: "Show session statistics".into(),
        },
        ToolDef {
            name: "ctx_doctor".into(),
            description: "Diagnose context-mode health".into(),
        },
        ToolDef {
            name: "ctx_upgrade".into(),
            description: "Check for upgrades".into(),
        },
        ToolDef {
            name: "ctx_purge".into(),
            description: "Purge knowledge base".into(),
        },
        ToolDef {
            name: "ctx_insight".into(),
            description: "Show insight dashboard".into(),
        },
        ToolDef {
            name: "ctx_semantic_search".into(),
            description: "Semantic search over embeddings".into(),
        },
        ToolDef {
            name: "ctx_index_embeddings".into(),
            description: "Index content for semantic search".into(),
        },
        ToolDef {
            name: "ctx_context_pack".into(),
            description: "Pack context into token budget".into(),
        },
        ToolDef {
            name: "ctx_vault_index".into(),
            description: "Index vault markdown notes".into(),
        },
        ToolDef {
            name: "ctx_vault_graph".into(),
            description: "Query vault graph relationships".into(),
        },
        ToolDef {
            name: "ctx_dead_code".into(),
            description: "Find dead code".into(),
        },
        ToolDef {
            name: "ctx_complexity".into(),
            description: "Analyze code complexity".into(),
        },
        ToolDef {
            name: "ctx_dep_graph".into(),
            description: "Build dependency graph".into(),
        },
    ]
}
