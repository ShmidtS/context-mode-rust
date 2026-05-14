use anyhow::{Result, anyhow};
use context_mode_store::ContentStore;
use once_cell::sync::Lazy;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const CACHE_TTL: Duration = Duration::from_secs(5 * 60);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

static FETCH_CACHE: Lazy<Mutex<HashMap<String, (String, Instant)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, serde::Deserialize)]
pub struct FetchAndIndexParams {
    pub url: String,
    pub timeout: Option<u64>,
    pub source: Option<String>,
}

pub async fn ctx_fetch_and_index(params: serde_json::Value) -> Result<serde_json::Value> {
    let params: FetchAndIndexParams = serde_json::from_value(params)?;
    let (markdown, status, cached) = fetch_markdown(&params).await?;
    let content_length = markdown.len();
    let mut store = open_store()?;
    store.index_content(
        markdown,
        params.source.unwrap_or_else(|| params.url.clone()),
    )?;

    Ok(json!({
        "url": params.url,
        "status": status,
        "content_length": content_length,
        "cached": cached,
    }))
}

async fn fetch_markdown(params: &FetchAndIndexParams) -> Result<(String, u16, bool)> {
    if let Some(cached) = get_cached(&params.url) {
        return Ok((cached, 200, true));
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(
            params.timeout.unwrap_or(DEFAULT_TIMEOUT.as_secs()),
        ))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;
    let response = client.get(&params.url).send().await?;
    let status = response.status();
    if !status.is_success() {
        return Err(anyhow!(
            "fetch failed with status {} for {}",
            status,
            params.url
        ));
    }

    let html = response.text().await?;
    let markdown = html2md::parse_html(&html);
    set_cached(params.url.clone(), markdown.clone());
    Ok((markdown, status.as_u16(), false))
}

fn get_cached(url: &str) -> Option<String> {
    let mut cache = FETCH_CACHE.lock().ok()?;
    let now = Instant::now();
    cache.retain(|_, (_, fetched_at)| now.duration_since(*fetched_at) < CACHE_TTL);
    cache.get(url).map(|(content, _)| content.clone())
}

fn set_cached(url: String, content: String) {
    if let Ok(mut cache) = FETCH_CACHE.lock() {
        let now = Instant::now();
        cache.retain(|_, (_, fetched_at)| now.duration_since(*fetched_at) < CACHE_TTL);
        cache.insert(url, (content, now));
    }
}

fn open_store() -> Result<ContentStore> {
    let path = Path::new(".context-mode/store.db");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    ContentStore::open(path)
        .or_else(|_| ContentStore::in_memory())
        .map_err(Into::into)
}
