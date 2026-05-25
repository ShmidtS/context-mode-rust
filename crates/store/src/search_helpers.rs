use crate::schema::build_search_sql;
use crate::types::{SearchMode, SearchResult, SearchRow, SourceMatchMode, is_stopword};
use context_mode_core::ContentType;
use regex::Regex;
use rusqlite::{Connection, params_from_iter, types::Value};
use std::collections::HashSet;

fn dedupe_tokens(tokens: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for token in tokens {
        let key = token.to_lowercase();
        if seen.insert(key) {
            out.push(token);
        }
    }
    out
}

fn format_search_tokens(tokens: Vec<String>, mode: SearchMode, empty_fallback: &str) -> String {
    if tokens.is_empty() {
        return empty_fallback.to_string();
    }
    let meaningful: Vec<String> = tokens
        .iter()
        .filter(|w| !is_stopword(&w.to_lowercase()))
        .cloned()
        .collect();
    let final_tokens = if meaningful.is_empty() {
        tokens
    } else {
        meaningful
    };
    final_tokens
        .into_iter()
        .map(|w| format!("\"{}\"", w))
        .collect::<Vec<_>>()
        .join(mode.as_fts_joiner())
}

pub fn sanitize_query(query: &str, mode: SearchMode) -> String {
    let re = Regex::new("['\\[\\]\"(){}*.:^~.]").expect("valid sanitize regex");
    let cleaned = re.replace_all(query, " ");
    let tokens = dedupe_tokens(
        cleaned
            .split_whitespace()
            .filter(|w| !matches!(w.to_uppercase().as_str(), "AND" | "OR" | "NOT" | "NEAR"))
            .map(ToString::to_string)
            .collect(),
    );
    format_search_tokens(tokens, mode, "\"\"")
}

pub fn sanitize_trigram_query(query: &str, mode: SearchMode) -> String {
    let re = Regex::new("[\"'\\[\\](){}*.:^~.]").expect("valid trigram sanitize regex");
    let cleaned = re.replace_all(query, "").trim().to_string();
    if cleaned.len() < 3 {
        return String::new();
    }
    let tokens = dedupe_tokens(
        cleaned
            .split_whitespace()
            .filter(|w| w.len() >= 3)
            .map(ToString::to_string)
            .collect(),
    );
    format_search_tokens(tokens, mode, "")
}

pub fn map_search_rows(rows: Vec<SearchRow>) -> Vec<SearchResult> {
    rows.into_iter()
        .map(|row| SearchResult {
            title: row.title,
            content: row.content,
            source: row.label,
            rank: row.rank,
            content_type: if row.content_type == "code" {
                ContentType::Code
            } else {
                ContentType::Prose
            },
            match_layer: None,
            highlighted: Some(row.highlighted),
            timestamp: row.timestamp,
            confidence: None,
            confidence_source: None,
        })
        .collect()
}

pub fn source_filter_param(source: &str, source_match_mode: SourceMatchMode) -> String {
    match source_match_mode {
        SourceMatchMode::Exact => source.to_string(),
        SourceMatchMode::Like => format!("%{}%", source),
    }
}

#[derive(Debug, Clone)]
pub struct SearchStmts {
    pub base: String,
    pub filtered: String,
    pub exact: String,
    pub content_type: String,
    pub filtered_content_type: String,
    pub exact_content_type: String,
}

impl SearchStmts {
    pub fn for_table(table: crate::schema::FtsTable) -> Self {
        Self {
            base: build_search_sql(table, None, false),
            filtered: build_search_sql(table, Some("like"), false),
            exact: build_search_sql(table, Some("exact"), false),
            content_type: build_search_sql(table, None, true),
            filtered_content_type: build_search_sql(table, Some("like"), true),
            exact_content_type: build_search_sql(table, Some("exact"), true),
        }
    }
}

pub fn select_search_sql_and_params(
    stmts: &SearchStmts,
    sanitized: &str,
    limit: usize,
    source: Option<&str>,
    content_type: Option<ContentType>,
    source_match_mode: SourceMatchMode,
) -> (String, Vec<Value>) {
    let mut params = vec![Value::Text(sanitized.to_string())];
    let sql = match (source, content_type) {
        (Some(source), Some(content_type)) => {
            params.push(Value::Text(source_filter_param(source, source_match_mode)));
            params.push(Value::Text(content_type_to_str(content_type).to_string()));
            if source_match_mode == SourceMatchMode::Exact {
                stmts.exact_content_type.clone()
            } else {
                stmts.filtered_content_type.clone()
            }
        }
        (Some(source), None) => {
            params.push(Value::Text(source_filter_param(source, source_match_mode)));
            if source_match_mode == SourceMatchMode::Exact {
                stmts.exact.clone()
            } else {
                stmts.filtered.clone()
            }
        }
        (None, Some(content_type)) => {
            params.push(Value::Text(content_type_to_str(content_type).to_string()));
            stmts.content_type.clone()
        }
        (None, None) => stmts.base.clone(),
    };
    params.push(Value::Integer(limit as i64));
    (sql, params)
}

pub fn content_type_to_str(content_type: ContentType) -> &'static str {
    match content_type {
        ContentType::Code => "code",
        ContentType::Prose => "prose",
    }
}

pub fn row_to_search_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SearchRow> {
    Ok(SearchRow {
        title: row.get(0)?,
        content: row.get(1)?,
        content_type: row.get(2)?,
        timestamp: row.get(3)?,
        label: row.get(4)?,
        rank: row.get(5)?,
        highlighted: row.get(6)?,
    })
}

#[derive(Debug, Clone)]
pub struct SearchCoreParams<'a> {
    pub conn: &'a Connection,
    pub query: &'a str,
    pub limit: usize,
    pub source: Option<&'a str>,
    pub mode: SearchMode,
    pub content_type: Option<ContentType>,
    pub source_match_mode: SourceMatchMode,
    pub sanitize: fn(&str, SearchMode) -> String,
    pub stmts: &'a SearchStmts,
    pub allow_empty: bool,
}

pub fn search_core(params: SearchCoreParams<'_>) -> rusqlite::Result<Vec<SearchResult>> {
    let sanitized = (params.sanitize)(params.query, params.mode);
    if !params.allow_empty && sanitized.is_empty() {
        return Ok(Vec::new());
    }
    let (sql, args) = select_search_sql_and_params(
        params.stmts,
        &sanitized,
        params.limit,
        params.source,
        params.content_type,
        params.source_match_mode,
    );
    let mut stmt = params.conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params_from_iter(args), row_to_search_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(map_search_rows(rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_query_quotes_non_stopword_tokens_and_dedupes() {
        assert_eq!(
            sanitize_query("Error AND error from API", SearchMode::Or),
            "\"Error\" OR \"API\""
        );
    }

    #[test]
    fn trigram_query_rejects_short_input() {
        assert_eq!(sanitize_trigram_query("ab", SearchMode::And), "");
    }
}
