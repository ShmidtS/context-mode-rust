use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::Html,
    routing::{get, post},
};
use context_mode_core::db_schema;
use context_mode_core::local_indexer;
use context_mode_core::search;
use context_mode_core::types::{IndexResult, SearchResult, VaultNode};
use context_mode_core::vault::VaultGraph;
use context_mode_session::{SessionDB, get_lifetime_stats};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::info;

#[derive(Clone)]
struct AppState {
    db_path: PathBuf,
    session_db_path: PathBuf,
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
        db_schema::init_local_schema(&conn)?;
        f(&mut conn)
    }
}

async fn health_check() -> &'static str {
    "OK"
}

const DASHBOARD_HTML: &str = include_str!("dashboard.html");

async fn dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
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
    let results = tokio::task::block_in_place(|| match req.repo {
        Some(repo) => {
            state.with_conn(|conn| search::search_repo(conn, &repo, &req.query, req.limit))
        }
        None => state.with_conn(|conn| search::search(conn, &req.query, req.limit)),
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
    let provider = context_mode_core::embedding::OllamaEmbeddingProvider::new();
    let results = tokio::task::block_in_place(|| {
        state.with_conn_mut(|conn| {
            local_indexer::index_repository(conn, &req.repo, &req.path, &provider)
        })
    })
    .map_err(|e| e.to_string())?;
    Ok(Json(IndexResponse { results }))
}

async fn vault_nodes_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<VaultNode>>, String> {
    let graph = state.graph.lock().map_err(|e| e.to_string())?;
    let nodes = graph.all_nodes().into_iter().cloned().collect();
    Ok(Json(nodes))
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

fn api_error(err: impl ToString) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: err.to_string(),
        }),
    )
}

#[derive(Serialize)]
struct StatsResponse {
    total_events: i64,
    tool_calls: i64,
    errors: i64,
    sessions: i64,
}

async fn stats_handler(State(state): State<AppState>) -> ApiResult<StatsResponse> {
    let stats = tokio::task::block_in_place(|| {
        let db = SessionDB::open(&state.session_db_path)?;
        let lifetime = get_lifetime_stats(&db)?;
        let errors = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM session_events WHERE category = 'error' OR type = 'error'",
                [],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0);
        anyhow::Ok(StatsResponse {
            total_events: lifetime.events,
            tool_calls: lifetime.tool_calls,
            errors,
            sessions: lifetime.sessions,
        })
    })
    .map_err(api_error)?;

    Ok(Json(stats))
}

#[derive(Serialize)]
struct SessionSummary {
    id: String,
    start_time: String,
    duration_seconds: i64,
    tools_used: Vec<String>,
}

async fn stats_sessions_handler(State(state): State<AppState>) -> ApiResult<Vec<SessionSummary>> {
    let sessions = tokio::task::block_in_place(|| {
        let db = SessionDB::open(&state.session_db_path)?;
        let mut stmt = db.connection().prepare(
            "SELECT m.session_id,
                    m.started_at,
                    CAST(strftime('%s', COALESCE(m.last_event_at, m.started_at)) - strftime('%s', m.started_at) AS INTEGER),
                    COALESCE(group_concat(t.tool), '')
             FROM session_meta m
             LEFT JOIN tool_calls t ON t.session_id = m.session_id
             GROUP BY m.session_id, m.started_at, m.last_event_at
             ORDER BY m.started_at DESC
             LIMIT 20",
        )?;
        let rows = stmt.query_map([], |row| {
            let tools: String = row.get(3)?;
            Ok(SessionSummary {
                id: row.get(0)?,
                start_time: row.get(1)?,
                duration_seconds: row.get(2)?,
                tools_used: tools
                    .split(',')
                    .filter(|tool| !tool.is_empty())
                    .map(ToString::to_string)
                    .collect(),
            })
        })?;
        anyhow::Ok(rows.collect::<Result<Vec<_>, _>>()?)
    })
    .map_err(api_error)?;

    Ok(Json(sessions))
}

#[derive(Serialize)]
struct TagCount {
    tag: String,
    count: usize,
}

#[derive(Serialize)]
struct VaultGodNode {
    id: i64,
    title: String,
    path: String,
    degree: i64,
}

#[derive(Serialize)]
struct VaultAnalyticsResponse {
    node_count: usize,
    edge_count: usize,
    top_tags: Vec<TagCount>,
    god_nodes: Vec<VaultGodNode>,
}

async fn vault_analytics_handler(
    State(state): State<AppState>,
) -> ApiResult<VaultAnalyticsResponse> {
    let graph = state.graph.lock().map_err(api_error)?;
    let nodes = graph.all_nodes();
    let mut tag_counts = std::collections::HashMap::<String, usize>::new();
    for node in &nodes {
        for tag in graph.tags_for(node.id) {
            *tag_counts.entry(tag.tag.clone()).or_default() += 1;
        }
    }
    let mut top_tags: Vec<_> = tag_counts
        .into_iter()
        .map(|(tag, count)| TagCount { tag, count })
        .collect();
    top_tags.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.tag.cmp(&b.tag)));
    top_tags.truncate(10);

    let mut god_nodes: Vec<_> = nodes
        .iter()
        .map(|node| VaultGodNode {
            id: node.id,
            title: node.title.clone(),
            path: node.note_path.clone(),
            degree: node.in_degree + node.out_degree,
        })
        .collect();
    god_nodes.sort_by(|a, b| b.degree.cmp(&a.degree).then_with(|| a.path.cmp(&b.path)));
    god_nodes.truncate(10);

    Ok(Json(VaultAnalyticsResponse {
        node_count: nodes.len(),
        edge_count: graph.all_edges().len(),
        top_tags,
        god_nodes,
    }))
}

#[derive(Deserialize)]
struct AnalysisRequest {
    path: String,
}

async fn dead_code_handler(Json(req): Json<AnalysisRequest>) -> ApiResult<Value> {
    let result = context_mode_tools::code_analysis::ctx_dead_code(json!({ "path": req.path }))
        .await
        .map_err(api_error)?;
    Ok(Json(result))
}

async fn complexity_handler(Json(req): Json<AnalysisRequest>) -> ApiResult<Value> {
    let result = context_mode_tools::code_analysis::ctx_complexity(json!({ "path": req.path }))
        .await
        .map_err(api_error)?;
    Ok(Json(result))
}

async fn connectors_handler() -> ApiResult<Vec<Value>> {
    let connectors = tokio::task::block_in_place(|| {
        let path = connectors_path();
        if !path.exists() {
            return anyhow::Ok(Vec::new());
        }
        let data = std::fs::read_to_string(path)?;
        if data.trim().is_empty() {
            return anyhow::Ok(Vec::new());
        }
        anyhow::Ok(serde_json::from_str(&data)?)
    })
    .map_err(api_error)?;

    Ok(Json(connectors))
}

fn connectors_path() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".context-mode")
        .join("connectors.json")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let db_path = std::env::var("CM_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("context-mode.db"));

    {
        let conn = Connection::open(&db_path)?;
        db_schema::init_local_schema(&conn)?;
    }

    let state = AppState {
        db_path,
        session_db_path: SessionDB::default_path(),
        graph: Arc::new(Mutex::new(VaultGraph::new())),
    };

    let app = Router::new()
        .route("/", get(dashboard))
        .route("/health", get(health_check))
        .route("/search", post(search_handler))
        .route("/index", post(index_handler))
        .route("/vault/nodes", get(vault_nodes_handler))
        .route("/stats", get(stats_handler))
        .route("/stats/sessions", get(stats_sessions_handler))
        .route("/vault/analytics", get(vault_analytics_handler))
        .route("/analysis/dead-code", post(dead_code_handler))
        .route("/analysis/complexity", post(complexity_handler))
        .route("/connectors", get(connectors_handler))
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
        let db = tempfile::NamedTempFile::new().unwrap();
        let session_db = tempfile::NamedTempFile::new().unwrap();
        AppState {
            db_path: db.path().into(),
            session_db_path: session_db.path().into(),
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
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_stats() {
        let state = test_state();
        let app = Router::new()
            .route("/stats", get(stats_handler))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total_events"], 0);
        assert_eq!(json["tool_calls"], 0);
        assert_eq!(json["errors"], 0);
        assert_eq!(json["sessions"], 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_complexity_analysis() {
        let app = Router::new().route("/analysis/complexity", post(complexity_handler));
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs");
        let body = serde_json::to_vec(&serde_json::json!({ "path": path })).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/analysis/complexity")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
