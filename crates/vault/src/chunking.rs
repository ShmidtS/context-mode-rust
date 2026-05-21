use crate::ast_parser::parse_file;
use context_mode_core::Chunk;
use std::path::Path;

pub const TARGET_CHUNK_SIZE: usize = 1500;
pub const MIN_CHUNK_SIZE: usize = 50;
pub const RECURSION_DEPTH: usize = 500;
pub const MAX_INDEXED_FILE_SIZE: usize = 1_000_000;

pub fn chunk_source(source: impl Into<String>, path: &Path, content: &str) -> Vec<Chunk> {
    let source = source.into();
    if content.len() > MAX_INDEXED_FILE_SIZE {
        return Vec::new();
    }

    let ast = parse_file(path, content);
    let line_offsets = line_offsets(content);
    let mut chunks = Vec::new();
    let mut symbols = ast.symbols;
    symbols.sort_by_key(|symbol| symbol.line);

    if symbols.is_empty() {
        chunks = chunk_lines(
            &source,
            content,
            1,
            line_offsets.len().saturating_sub(1),
            &line_offsets,
        );
    } else {
        for (index, symbol) in symbols.iter().enumerate() {
            let start_line = symbol.line.max(1);
            let next_start = symbols
                .get(index + 1)
                .map(|next| next.line.saturating_sub(1));
            let end_line = symbol
                .end_line
                .or(next_start)
                .unwrap_or_else(|| line_offsets.len().saturating_sub(1))
                .max(start_line);
            chunks.extend(chunk_lines(
                &source,
                content,
                start_line,
                end_line,
                &line_offsets,
            ));
        }
    }

    merge_small_adjacent(chunks)
}

pub fn chunk_text(source: impl Into<String>, content: &str) -> Vec<Chunk> {
    let source = source.into();
    if content.len() > MAX_INDEXED_FILE_SIZE {
        return Vec::new();
    }
    let line_offsets = line_offsets(content);
    chunk_lines(
        &source,
        content,
        1,
        line_offsets.len().saturating_sub(1),
        &line_offsets,
    )
}

fn line_offsets(content: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (index, byte) in content.bytes().enumerate() {
        if byte == b'\n' {
            offsets.push(index + 1);
        }
    }
    if *offsets.last().unwrap_or(&0) != content.len() {
        offsets.push(content.len());
    }
    offsets
}

fn chunk_lines(
    source: &str,
    content: &str,
    start_line: usize,
    end_line: usize,
    line_offsets: &[usize],
) -> Vec<Chunk> {
    if content.is_empty() || start_line > end_line || line_offsets.len() < 2 {
        return Vec::new();
    }

    let max_line = line_offsets.len() - 1;
    let mut chunks = Vec::new();
    let mut chunk_start_line = start_line.min(max_line).max(1);
    let end_line = end_line.min(max_line).max(chunk_start_line);

    while chunk_start_line <= end_line {
        let mut chunk_end_line = chunk_start_line;
        while chunk_end_line < end_line {
            let byte_start = line_offsets[chunk_start_line - 1];
            let byte_end = line_offsets[chunk_end_line];
            if byte_end.saturating_sub(byte_start) >= TARGET_CHUNK_SIZE {
                break;
            }
            chunk_end_line += 1;
        }

        let byte_start = line_offsets[chunk_start_line - 1];
        let byte_end = line_offsets[chunk_end_line];
        let chunk_content = content[byte_start..byte_end].to_string();
        if !chunk_content.trim().is_empty() {
            chunks.push(Chunk {
                source: source.to_string(),
                content: chunk_content,
                start_line: chunk_start_line,
                end_line: chunk_end_line,
                byte_start,
                byte_end,
            });
        }
        chunk_start_line = chunk_end_line + 1;
    }

    chunks
}

fn merge_small_adjacent(chunks: Vec<Chunk>) -> Vec<Chunk> {
    let mut merged: Vec<Chunk> = Vec::new();

    for chunk in chunks {
        if let Some(previous) = merged.last_mut() {
            let combined_len = previous.content.len() + chunk.content.len();
            if (previous.content.len() < MIN_CHUNK_SIZE || chunk.content.len() < MIN_CHUNK_SIZE)
                && previous.source == chunk.source
                && previous.end_line + 1 >= chunk.start_line
                && combined_len <= TARGET_CHUNK_SIZE * 2
            {
                previous.content.push_str(&chunk.content);
                previous.end_line = chunk.end_line;
                previous.byte_end = chunk.byte_end;
                continue;
            }
        }
        merged.push(chunk);
    }

    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_line_chunks() {
        let content = (0..100)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = chunk_source("notes.txt", Path::new("notes.txt"), &content);

        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].source, "notes.txt");
        assert_eq!(chunks[0].start_line, 1);
    }

    #[test]
    fn skips_large_files() {
        let content = "a".repeat(MAX_INDEXED_FILE_SIZE + 1);
        assert!(chunk_text("large.txt", &content).is_empty());
    }

    #[test]
    fn merges_small_symbol_chunks() {
        let content = "fn a() {}\nfn b() {}\n";
        let chunks = chunk_source("test.rs", Path::new("test.rs"), content);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 2);
    }
}
