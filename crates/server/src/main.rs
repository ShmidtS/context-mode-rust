use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use context_mode_core::db_schema;
use context_mode_core::local_indexer;
use context_mode_core::search;
use context_mode_core::types::{IndexResult, SearchResult, VaultNode};
use context_mode_core::vault::VaultGraph;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Clone)]
struct AppState {
    db_path: PathBuf,
    graph: Arc<Mutex<VaultGraph>>,
}

impl AppState {
    fn with_conn<F, T>(&self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(&Connection) -> anyhow::Result<T>,
    {
        let conn = Connection::open(&self.db_path)?;
        f(&conn)
    }

    fn with_conn_mut<F, T>(&self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(&mut Connection) -> anyhow::Result<T>,
    {
        let mut conn = Connection::open(&self.db_path)?;
        db_schema::init_local_schema(&mut conn)?;
        f(&mut conn)
    }
}

async fn health_check() -> &'static str {
    "OK"
}

#[derive(Deserialize)]
struct SearchRequest {
    query: String,
    repo: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    10
}

#[derive(Serialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
}

async fn search_handler(
    State(state): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, String> {
    let results = tokio::task::block_in_place(|| {
        match req.repo {
            Some(repo) => state.with_conn(|conn| search::search_repo(conn, &repo, &req.query, req.limit)),
            None => state.with_conn(|conn| search::search(conn, &req.query, req.limit)),
        }
    })
    .map_err(|e| e.to_string())?;
    Ok(Json(SearchResponse { results }))
}

#[derive(Deserialize)]
struct IndexRequest {
    repo: String,
    path: PathBuf,
}

#[derive(Serialize)]
struct IndexResponse {
    results: Vec<IndexResult>,
}

async fn index_handler(
    State(state): State<AppState>,
    Json(req): Json<IndexRequest>,
) -> Result<Json<IndexResponse>, String> {
    let results = tokio::task::block_in_place(|| {
        state.with_conn_mut(|conn| local_indexer::index_repository(conn, &req.repo, &req.path))
    })
    .map_err(|e| e.to_string())?;
    Ok(Json(IndexResponse { results }))
}

async fn vault_nodes_handler(State(state): State<AppState>) -> Result<Json<Vec<VaultNode>>, String> {
    let graph = state.graph.lock().map_err(|e| e.to_string())?;
    let nodes = graph.all_nodes().into_iter().cloned().collect();
    Ok(Json(nodes))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let db_path = std::env::var("CM_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("context-mode.db"));

    {
        let mut conn = Connection::open(&db_path)?;
        db_schema::init_local_schema(&mut conn)?;
    }

    let state = AppState {
        db_path,
        graph: Arc::new(Mutex::new(VaultGraph::new())),
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/search", post(search_handler))
        .route("/index", post(index_handler))
        .route("/vault/nodes", get(vault_nodes_handler))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("Server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    fn test_state() -> AppState {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        AppState {
            db_path: tmp.path().into(),
            graph: Arc::new(Mutex::new(VaultGraph::new())),
        }
    }

    #[tokio::test]
    async fn test_health() {
        let state = test_state();
        let app = Router::new()
            .route("/health", get(health_check))
            .with_state(state);

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
