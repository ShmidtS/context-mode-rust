use crate::types::{LinkType, MarkdownLink, ParsedNote, WikiLink};
use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

fn sha256_hex(content: &str) -> String {
    hex::encode(Sha256::digest(content.as_bytes()))
}

fn extract_frontmatter(content: &str) -> (HashMap<String, Value>, usize) {
    let lines: Vec<&str> = content.split('\n').collect();
    if lines.first().copied() != Some("---") {
        return (HashMap::new(), 0);
    }
    let Some(close_idx) = lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(i, line)| (*line == "---").then_some(i))
    else {
        return (HashMap::new(), 0);
    };

    let mut frontmatter = HashMap::new();
    for raw in &lines[1..close_idx] {
        let Some((key, value)) = raw.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() {
            continue;
        }
        frontmatter.insert(key.to_string(), parse_yaml_scalar(value));
    }
    (frontmatter, close_idx + 1)
}

fn parse_yaml_scalar(value: &str) -> Value {
    if value.starts_with('[') && value.ends_with(']') {
        return Value::Array(
            value[1..value.len() - 1]
                .split(',')
                .map(|s| Value::String(strip_quotes(s.trim()).to_string()))
                .collect(),
        );
    }
    match value {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => value
            .parse::<f64>()
            .map(|n| {
                serde_json::Number::from_f64(n)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            })
            .unwrap_or_else(|_| Value::String(strip_quotes(value).to_string())),
    }
}

fn strip_quotes(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(value)
}

fn derive_title(path: &str, frontmatter: &HashMap<String, Value>, content: &str) -> String {
    if let Some(title) = frontmatter.get("title").and_then(Value::as_str) {
        if !title.is_empty() {
            return title.to_string();
        }
    }
    for line in content.lines() {
        if let Some(title) = line.strip_prefix("# ") {
            return title.trim().to_string();
        }
    }
    crate::path_utils::path_stem(path)
}

fn context_slice(line: &str, start: usize, len: usize) -> String {
    let ctx_start = start.saturating_sub(60);
    let ctx_end = (start + len + 60).min(line.len());
    line.get(ctx_start..ctx_end).unwrap_or(line).to_string()
}

fn non_code_lines(content: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for (idx, line) in content.split('\n').enumerate() {
        if !in_fence && line.starts_with("```") {
            in_fence = true;
            continue;
        }
        if in_fence && line.starts_with("```") {
            in_fence = false;
            continue;
        }
        if in_fence {
            continue;
        }
        let is_indented_code = line.starts_with("    ") && !line.trim().is_empty();
        if !is_indented_code {
            out.push((idx + 1, line));
        }
    }
    out
}

pub fn parse_vault_note(file_path: impl Into<String>, content: &str) -> ParsedNote {
    let file_path = file_path.into();
    let content_hash = sha256_hex(content);
    let (frontmatter, _) = extract_frontmatter(content);
    let title = derive_title(&file_path, &frontmatter, content);
    let wiki_re =
        Regex::new(r"(!?)\[\[([^\]#|]+?)(?:#[^|]*)?(?:\|([^\]]+))?\]\]").expect("valid wiki regex");
    let md_re = Regex::new(r"\[([^\]]*)\]\(([^)\s]+)\)").expect("valid markdown link regex");
    let tag_re = Regex::new(r"(?:^|[\s(>])#([a-zA-Z][\w/-]*)").expect("valid tag regex");
    let url_re = Regex::new(r"^(?:https?|ftp|mailto):").expect("valid url regex");

    let mut tags = HashSet::new();
    if let Some(Value::Array(values)) = frontmatter.get("tags") {
        for value in values {
            if let Some(tag) = value.as_str() {
                tags.insert(tag.to_string());
            }
        }
    }

    let mut wiki_links = Vec::new();
    let mut markdown_links = Vec::new();
    for (line_number, line) in non_code_lines(content) {
        for caps in tag_re.captures_iter(line) {
            if let Some(tag) = caps.get(1).map(|m| m.as_str()) {
                tags.insert(tag.to_string());
                if let Some((parent, _)) = tag.split_once('/') {
                    tags.insert(parent.to_string());
                }
            }
        }

        for caps in wiki_re.captures_iter(line) {
            let whole = caps.get(0).expect("whole match");
            let target = caps.get(2).map(|m| m.as_str().trim()).unwrap_or_default();
            if target.is_empty() {
                continue;
            }
            wiki_links.push(WikiLink {
                target: target.to_string(),
                alias: caps
                    .get(3)
                    .map(|m| m.as_str().trim().to_string())
                    .filter(|s| !s.is_empty()),
                line_number,
                context: context_slice(line, whole.start(), whole.len()),
                link_type: if caps.get(1).map(|m| m.as_str()) == Some("!") {
                    LinkType::Embed
                } else {
                    LinkType::Wikilink
                },
            });
        }

        for caps in md_re.captures_iter(line) {
            let target = caps.get(2).map(|m| m.as_str().trim()).unwrap_or_default();
            if target.is_empty() || url_re.is_match(target) {
                continue;
            }
            let whole = caps.get(0).expect("whole match");
            markdown_links.push(MarkdownLink {
                text: caps
                    .get(1)
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_default(),
                target: target.to_string(),
                line_number,
                context: context_slice(line, whole.start(), whole.len()),
            });
        }
    }

    let mut tags: Vec<String> = tags.into_iter().collect();
    tags.sort();

    ParsedNote {
        path: file_path,
        title,
        frontmatter,
        tags,
        wiki_links,
        markdown_links,
        content_hash,
    }
}
