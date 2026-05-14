use crate::ast_parser::{AstResult, Symbol, SymbolKind};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Chunk {
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
}

pub fn chunk_by_symbols(content: &str, ast: &AstResult) -> Vec<Chunk> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut symbols: Vec<&Symbol> = ast.symbols.iter().collect();
    symbols.sort_by_key(|s| s.line);
    for (i, sym) in symbols.iter().enumerate() {
        let start = sym.line.saturating_sub(1);
        let end = if i + 1 < symbols.len() {
            symbols[i + 1].line.saturating_sub(1)
        } else {
            lines.len()
        };
        let text = lines[start..end].join("\n");
        chunks.push(Chunk {
            name: sym.name.clone(),
            kind: sym.kind.clone(),
            start_line: sym.line,
            end_line: if end == lines.len() { end } else { end + 1 },
            text,
        });
    }
    chunks
}

pub fn chunk_plain(content: &str, max_lines: usize) -> Vec<Chunk> {
    let lines: Vec<&str> = content.lines().collect();
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < lines.len() {
        let end = (start + max_lines).min(lines.len());
        let text = lines[start..end].join("\n");
        chunks.push(Chunk {
            name: format!("chunk_{}", chunks.len()),
            kind: SymbolKind::Unknown,
            start_line: start + 1,
            end_line: end,
            text,
        });
        start = end;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast_parser::parse_file;
    use std::path::Path;

    #[test]
    fn chunks_rust_file() {
        let code = "fn foo() {}\nfn bar() {}\n";
        let ast = parse_file(Path::new("t.rs"), code);
        let chunks = chunk_by_symbols(code, &ast);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].name, "foo");
        assert_eq!(chunks[1].name, "bar");
    }

    #[test]
    fn plain_chunking() {
        let code = "a\nb\nc\nd\n";
        let chunks = chunk_plain(code, 2);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "a\nb");
        assert_eq!(chunks[1].text, "c\nd");
    }
}
