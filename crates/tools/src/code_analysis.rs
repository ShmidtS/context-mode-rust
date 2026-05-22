use anyhow::{Context, anyhow};
use context_mode_vault::ast_parser::{Symbol, SymbolKind, parse_file};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub async fn ctx_dead_code(params: Value) -> anyhow::Result<Value> {
    let path = path_param(&params)?;
    let content = read_file(&path)?;
    let ast = parse_file(&path, &content);

    let dead_symbols: Vec<Value> = ast
        .symbols
        .iter()
        .filter(|symbol| is_function_or_method(symbol))
        .filter(|symbol| !name_appears_outside_declaration(&content, symbol))
        .map(|symbol| {
            json!({
                "name": symbol.name,
                "line": symbol.line,
                "kind": symbol_kind_name(&symbol.kind),
            })
        })
        .collect();

    Ok(json!({
        "content": [{
            "type": "text",
            "text": if dead_symbols.is_empty() {
                "No dead functions found.".to_string()
            } else {
                serde_json::to_string_pretty(&dead_symbols)?
            }
        }],
        "isError": false
    }))
}

pub async fn ctx_complexity(params: Value) -> anyhow::Result<Value> {
    let path = path_param(&params)?;
    let content = read_file(&path)?;
    let ast = parse_file(&path, &content);
    let lines: Vec<&str> = content.lines().collect();
    let mut functions: Vec<&Symbol> = ast
        .symbols
        .iter()
        .filter(|symbol| is_function_or_method(symbol))
        .collect();
    functions.sort_by_key(|symbol| symbol.line);

    let results: Vec<Value> = functions
        .iter()
        .enumerate()
        .map(|(index, symbol)| {
            let end_line = functions
                .get(index + 1)
                .map(|next| next.line.saturating_sub(1))
                .unwrap_or(lines.len());
            let start = symbol.line.saturating_sub(1);
            let end = end_line.min(lines.len());
            let body = if start < end {
                lines[start..end].join("\n")
            } else {
                String::new()
            };
            let complexity = estimate_complexity(&body);

            json!({
                "name": symbol.name,
                "line": symbol.line,
                "complexity": complexity,
                "risk": complexity_risk(complexity),
            })
        })
        .collect();

    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&results)?
        }],
        "isError": false
    }))
}

pub async fn ctx_dep_graph(params: Value) -> anyhow::Result<Value> {
    let paths = paths_param(&params)?;
    let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();

    for path in paths {
        let content = read_file(&path)?;
        let ast = parse_file(&path, &content);
        dependencies.insert(path.to_string_lossy().to_string(), ast.imports);
    }

    let mut nodes: Vec<String> = dependencies.keys().cloned().collect();
    nodes.sort();

    let mut edges = Vec::new();
    for from in &nodes {
        if let Some(imports) = dependencies.get(from) {
            for to in imports {
                edges.push(json!({ "from": from, "to": to }));
            }
        }
    }

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!(
                "Dependency graph: {} nodes, {} edges\n{}",
                nodes.len(),
                edges.len(),
                serde_json::to_string_pretty(&json!({"nodes": nodes, "edges": edges}))?
            )
        }],
        "isError": false
    }))
}

fn path_param(params: &Value) -> anyhow::Result<PathBuf> {
    params
        .get("path")
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("missing required string parameter: path"))
}

fn paths_param(params: &Value) -> anyhow::Result<Vec<PathBuf>> {
    if let Some(paths) = params.get("paths").and_then(Value::as_array) {
        let parsed: Vec<PathBuf> = paths
            .iter()
            .filter_map(Value::as_str)
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .collect();
        if !parsed.is_empty() {
            return Ok(parsed);
        }
    }

    path_param(params).map(|path| vec![path])
}

fn read_file(path: &Path) -> anyhow::Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

fn is_function_or_method(symbol: &Symbol) -> bool {
    symbol.kind == SymbolKind::Function || symbol.kind == SymbolKind::Method
}

fn name_appears_outside_declaration(content: &str, symbol: &Symbol) -> bool {
    content
        .lines()
        .enumerate()
        .filter(|(index, _)| index + 1 != symbol.line)
        .any(|(_, line)| line.contains(&symbol.name))
}

fn estimate_complexity(body: &str) -> usize {
    1 + count_word(body, "if")
        + count_word(body, "else")
        + count_word(body, "for")
        + count_word(body, "while")
        + count_word(body, "match")
        + body.matches("&&").count()
        + body.matches("||").count()
        + body.matches('?').count()
}

fn count_word(text: &str, word: &str) -> usize {
    text.split(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .filter(|part| *part == word)
        .count()
}

fn complexity_risk(complexity: usize) -> &'static str {
    match complexity {
        0..=4 => "low",
        5..=10 => "medium",
        _ => "high",
    }
}

fn symbol_kind_name(kind: &SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Method => "method",
        SymbolKind::Class => "class",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::Trait => "trait",
        SymbolKind::Interface => "interface",
        SymbolKind::Type => "type",
        SymbolKind::Module => "module",
        SymbolKind::Variable => "variable",
        SymbolKind::Field => "field",
        SymbolKind::Unknown => "unknown",
    }
}
