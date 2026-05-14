use crate::types::{Chunk, MAX_CHUNK_BYTES};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlainTextChunk {
    pub title: String,
    pub content: String,
}

pub fn chunk_markdown(text: &str, max_chunk_bytes: Option<usize>) -> Vec<Chunk> {
    let max_chunk_bytes = max_chunk_bytes.unwrap_or(MAX_CHUNK_BYTES);
    let lines: Vec<&str> = text.split('\n').collect();
    let heading_re = Regex::new(r"^(#{1,4})\s+(.+)$").expect("valid heading regex");
    let hr_re = Regex::new(r"^[-_*]{3,}\s*$").expect("valid horizontal-rule regex");
    let fence_re = Regex::new(r"^(`{3,})(.*)?$").expect("valid fence regex");

    let mut chunks = Vec::new();
    let mut heading_stack: Vec<(usize, String)> = Vec::new();
    let mut current_content: Vec<String> = Vec::new();
    let mut current_heading = String::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if hr_re.is_match(line) {
            flush_markdown_chunk(
                &mut chunks,
                &mut current_content,
                &heading_stack,
                &current_heading,
                max_chunk_bytes,
            );
            i += 1;
            continue;
        }

        if let Some(caps) = heading_re.captures(line) {
            flush_markdown_chunk(
                &mut chunks,
                &mut current_content,
                &heading_stack,
                &current_heading,
                max_chunk_bytes,
            );
            let level = caps.get(1).map(|m| m.as_str().len()).unwrap_or(1);
            let heading = caps
                .get(2)
                .map(|m| m.as_str().trim())
                .unwrap_or_default()
                .to_string();
            while heading_stack
                .last()
                .is_some_and(|(existing_level, _)| *existing_level >= level)
            {
                heading_stack.pop();
            }
            heading_stack.push((level, heading.clone()));
            current_heading = heading;
            current_content.push(line.to_string());
            i += 1;
            continue;
        }

        if let Some(caps) = fence_re.captures(line) {
            let fence = caps.get(1).map(|m| m.as_str()).unwrap_or("```");
            current_content.push(line.to_string());
            i += 1;
            while i < lines.len() {
                current_content.push(lines[i].to_string());
                if lines[i].starts_with(fence) && lines[i].trim() == fence {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }

        current_content.push(line.to_string());
        i += 1;
    }

    flush_markdown_chunk(
        &mut chunks,
        &mut current_content,
        &heading_stack,
        &current_heading,
        max_chunk_bytes,
    );
    chunks
}

fn build_title(heading_stack: &[(usize, String)], current_heading: &str) -> String {
    if heading_stack.is_empty() {
        if current_heading.is_empty() {
            "Untitled".to_string()
        } else {
            current_heading.to_string()
        }
    } else {
        heading_stack
            .iter()
            .map(|(_, text)| text.as_str())
            .collect::<Vec<_>>()
            .join(" > ")
    }
}

fn flush_markdown_chunk(
    chunks: &mut Vec<Chunk>,
    current_content: &mut Vec<String>,
    heading_stack: &[(usize, String)],
    current_heading: &str,
    max_chunk_bytes: usize,
) {
    let joined = current_content.join("\n").trim().to_string();
    if joined.is_empty() {
        return;
    }

    let title = build_title(heading_stack, current_heading);
    let has_code = current_content.iter().any(|line| line.starts_with("```"));
    if joined.len() <= max_chunk_bytes {
        chunks.push(Chunk {
            title,
            content: joined,
            has_code,
        });
        current_content.clear();
        return;
    }

    let paragraphs: Vec<&str> = joined.split("\n\n").collect();
    let mut accumulator: Vec<&str> = Vec::new();
    let mut part_index = 1;
    for para in paragraphs.iter().copied() {
        accumulator.push(para);
        if accumulator.join("\n\n").len() > max_chunk_bytes && accumulator.len() > 1 {
            accumulator.pop();
            flush_accumulator(
                chunks,
                &mut accumulator,
                &title,
                paragraphs.len(),
                &mut part_index,
            );
            accumulator.push(para);
        }
    }
    flush_accumulator(
        chunks,
        &mut accumulator,
        &title,
        paragraphs.len(),
        &mut part_index,
    );
    current_content.clear();
}

fn flush_accumulator(
    chunks: &mut Vec<Chunk>,
    accumulator: &mut Vec<&str>,
    title: &str,
    paragraph_count: usize,
    part_index: &mut usize,
) {
    if accumulator.is_empty() {
        return;
    }
    let part = accumulator.join("\n\n").trim().to_string();
    if part.is_empty() {
        return;
    }
    let title = if paragraph_count > 1 {
        let title = format!("{} ({})", title, *part_index);
        *part_index += 1;
        title
    } else {
        title.to_string()
    };
    let has_code = part.contains("```");
    chunks.push(Chunk {
        title,
        content: part,
        has_code,
    });
    accumulator.clear();
}

pub fn chunk_plain_text(text: &str, lines_per_chunk: usize) -> Vec<PlainTextChunk> {
    let sections: Vec<&str> = text.split("\n\n").collect();
    if sections.len() >= 3 && sections.len() <= 200 && sections.iter().all(|s| s.len() < 5000) {
        return sections
            .iter()
            .enumerate()
            .filter_map(|(idx, section)| {
                let trimmed = section.trim();
                if trimmed.is_empty() {
                    return None;
                }
                let first_line = trimmed
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .chars()
                    .take(80)
                    .collect::<String>();
                Some(PlainTextChunk {
                    title: if first_line.is_empty() {
                        format!("Section {}", idx + 1)
                    } else {
                        first_line
                    },
                    content: trimmed.to_string(),
                })
            })
            .collect();
    }

    let lines: Vec<&str> = text.split('\n').collect();
    if lines.len() <= lines_per_chunk {
        return vec![PlainTextChunk {
            title: "Output".to_string(),
            content: text.to_string(),
        }];
    }

    let overlap = 2usize;
    let step = lines_per_chunk.saturating_sub(overlap).max(1);
    let mut chunks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let end = (i + lines_per_chunk).min(lines.len());
        let slice = &lines[i..end];
        if slice.is_empty() {
            break;
        }
        let start_line = i + 1;
        let end_line = i + slice.len();
        let first_line = slice[0].trim().chars().take(80).collect::<String>();
        chunks.push(PlainTextChunk {
            title: if first_line.is_empty() {
                format!("Lines {}-{}", start_line, end_line)
            } else {
                first_line
            },
            content: slice.join("\n"),
        });
        i += step;
    }
    chunks
}

pub fn walk_json(value: &Value, path: &[String], chunks: &mut Vec<Chunk>, max_chunk_bytes: usize) {
    let title = if path.is_empty() {
        "(root)".to_string()
    } else {
        path.join(" > ")
    };
    let serialized = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());

    if serialized.len() <= max_chunk_bytes {
        let should_recurse = value
            .as_object()
            .is_some_and(|obj| obj.values().any(|v| v.is_object() || v.is_array()));
        if !should_recurse {
            chunks.push(Chunk {
                title,
                content: serialized,
                has_code: true,
            });
            return;
        }
    }

    if let Some(obj) = value.as_object() {
        if obj.is_empty() {
            chunks.push(Chunk {
                title,
                content: serialized,
                has_code: true,
            });
            return;
        }
        for (key, val) in obj {
            let mut next_path = path.to_vec();
            next_path.push(key.clone());
            walk_json(val, &next_path, chunks, max_chunk_bytes);
        }
        return;
    }

    if let Some(arr) = value.as_array() {
        chunk_json_array(arr, path, chunks, max_chunk_bytes);
        return;
    }

    chunks.push(Chunk {
        title,
        content: serialized,
        has_code: false,
    });
}

fn find_identity_field(arr: &[Value]) -> Option<&'static str> {
    let first = arr.first()?.as_object()?;
    ["id", "name", "title", "path", "slug", "key", "label"]
        .into_iter()
        .find(|field| {
            first
                .get(*field)
                .is_some_and(|v| v.is_string() || v.is_number())
        })
}

fn identity_value(item: &Value, field: &str) -> String {
    item.as_object()
        .and_then(|obj| obj.get(field))
        .map(|value| {
            value
                .as_str()
                .map_or_else(|| value.to_string(), ToString::to_string)
        })
        .unwrap_or_default()
}

fn json_batch_title(
    prefix: &str,
    start_idx: usize,
    end_idx: usize,
    batch: &[Value],
    identity_field: Option<&str>,
) -> String {
    let sep = if prefix.is_empty() {
        String::new()
    } else {
        format!("{} > ", prefix)
    };
    let Some(identity_field) = identity_field else {
        return if start_idx == end_idx {
            format!("{}[{}]", sep, start_idx)
        } else {
            format!("{}[{}-{}]", sep, start_idx, end_idx)
        };
    };

    if batch.len() == 1 {
        return format!("{}{}", sep, identity_value(&batch[0], identity_field));
    }
    if batch.len() <= 3 {
        return format!(
            "{}{}",
            sep,
            batch
                .iter()
                .map(|v| identity_value(v, identity_field))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    format!(
        "{}{}…{}",
        sep,
        identity_value(&batch[0], identity_field),
        identity_value(&batch[batch.len() - 1], identity_field)
    )
}

fn chunk_json_array(
    arr: &[Value],
    path: &[String],
    chunks: &mut Vec<Chunk>,
    max_chunk_bytes: usize,
) {
    let prefix = if path.is_empty() {
        "(root)".to_string()
    } else {
        path.join(" > ")
    };
    let identity_field = find_identity_field(arr);
    let mut batch: Vec<Value> = Vec::new();
    let mut batch_start = 0usize;

    for (i, item) in arr.iter().enumerate() {
        batch.push(item.clone());
        let candidate = serde_json::to_string_pretty(&batch).unwrap_or_default();
        if candidate.len() > max_chunk_bytes && batch.len() > 1 {
            batch.pop();
            let title = json_batch_title(&prefix, batch_start, i - 1, &batch, identity_field);
            let content = serde_json::to_string_pretty(&batch).unwrap_or_else(|_| "[]".to_string());
            chunks.push(Chunk {
                title,
                content,
                has_code: true,
            });
            batch = vec![item.clone()];
            batch_start = i;
        }
    }

    if !batch.is_empty() {
        let title = json_batch_title(
            &prefix,
            batch_start,
            batch_start + batch.len() - 1,
            &batch,
            identity_field,
        );
        let content = serde_json::to_string_pretty(&batch).unwrap_or_else(|_| "[]".to_string());
        chunks.push(Chunk {
            title,
            content,
            has_code: true,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_preserves_heading_titles_and_code_blocks() {
        let chunks = chunk_markdown(
            "# API\nIntro\n```rust\nfn main() {}\n```\n## Usage\nRun it",
            None,
        );
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].title, "API");
        assert!(chunks[0].has_code);
        assert_eq!(chunks[1].title, "API > Usage");
    }

    #[test]
    fn plain_text_uses_overlapping_line_chunks() {
        let chunks = chunk_plain_text("a\nb\nc\nd\ne", 3);
        assert_eq!(chunks[0].content, "a\nb\nc");
        assert_eq!(chunks[1].content, "b\nc\nd");
    }

    #[test]
    fn json_array_titles_use_identity_fields() {
        let value: Value = serde_json::json!({"items":[{"id":1,"name":"a"},{"id":2,"name":"b"}]});
        let mut chunks = Vec::new();
        walk_json(&value, &[], &mut chunks, 64);
        assert!(
            chunks
                .iter()
                .any(|c| c.title.contains("items > 1") || c.title.contains("items > 1, 2"))
        );
    }
}
