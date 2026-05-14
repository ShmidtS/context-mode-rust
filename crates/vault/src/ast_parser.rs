use regex::Regex;
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{Language, Node, Parser};

macro_rules! lazy_static_re {
    ($name:ident, $pattern:expr) => {
        static $name: std::sync::LazyLock<Regex> =
            std::sync::LazyLock::new(|| Regex::new($pattern).expect("valid regex"));
    };
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub line: usize,
    pub end_line: Option<usize>,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Trait,
    Interface,
    Type,
    Module,
    Variable,
    Field,
    Unknown,
}

#[derive(Debug, Clone, Default)]
pub struct AstResult {
    pub symbols: Vec<Symbol>,
    pub imports: Vec<String>,
    pub language: String,
}

pub fn parse_file(path: &Path, content: &str) -> AstResult {
    try_parse_file_tree_sitter(path, content).unwrap_or_else(|| parse_file_regex(path, content))
}

pub fn parse_file_tree_sitter(path: &Path, content: &str) -> AstResult {
    try_parse_file_tree_sitter(path, content).unwrap_or_else(|| parse_file_regex(path, content))
}

fn parse_file_regex(path: &Path, content: &str) -> AstResult {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let language = ext_to_language(ext);
    let mut result = AstResult {
        language: language.clone(),
        ..Default::default()
    };

    match language.as_str() {
        "rust" => parse_rust(content, &mut result),
        "javascript" | "typescript" => parse_js_ts(content, &mut result),
        "python" => parse_python(content, &mut result),
        "go" => parse_go(content, &mut result),
        _ => parse_generic(content, &mut result),
    }

    result
}

fn ext_to_language(ext: &str) -> String {
    match ext {
        "rs" => "rust".into(),
        "js" | "mjs" | "cjs" => "javascript".into(),
        "ts" | "tsx" | "mts" | "cts" => "typescript".into(),
        "py" => "python".into(),
        "go" => "go".into(),
        "java" => "java".into(),
        "rb" => "ruby".into(),
        "php" => "php".into(),
        _ => "generic".into(),
    }
}

fn try_parse_file_tree_sitter(path: &Path, content: &str) -> Option<AstResult> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let language_name = ext_to_language(ext);
    let language = tree_sitter_language(language_name.as_str())?;
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    let tree = parser.parse(content, None)?;
    if tree.root_node().has_error() {
        return None;
    }

    let mut result = AstResult {
        language: language_name.clone(),
        ..Default::default()
    };
    collect_tree_sitter_nodes(
        tree.root_node(),
        content,
        language_name.as_str(),
        &mut result,
    );
    Some(result)
}

fn tree_sitter_language(language: &str) -> Option<Language> {
    match language {
        "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
        "javascript" | "typescript" => Some(tree_sitter_javascript::LANGUAGE.into()),
        "python" => Some(tree_sitter_python::LANGUAGE.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        _ => None,
    }
}

fn collect_tree_sitter_nodes(node: Node, content: &str, language: &str, result: &mut AstResult) {
    match language {
        "rust" => collect_rust_node(node, content, result),
        "javascript" | "typescript" => collect_js_ts_node(node, content, result),
        "python" => collect_python_node(node, content, result),
        "go" => collect_go_node(node, content, result),
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_tree_sitter_nodes(child, content, language, result);
    }
}

fn collect_rust_node(node: Node, content: &str, result: &mut AstResult) {
    let kind = match node.kind() {
        "function_item" => SymbolKind::Function,
        "struct_item" => SymbolKind::Struct,
        "enum_item" => SymbolKind::Enum,
        "trait_item" => SymbolKind::Trait,
        "impl_item" => SymbolKind::Class,
        "use_declaration" => {
            if let Some(import) = node_text(node, content).strip_prefix("use ") {
                result
                    .imports
                    .push(import.trim_end_matches(';').trim().to_string());
            }
            return;
        }
        _ => return,
    };

    if let Some(name) = symbol_name(node, content) {
        result.symbols.push(Symbol {
            name,
            kind,
            line: node.start_position().row + 1,
            end_line: Some(node.end_position().row + 1),
            signature: symbol_signature(node, content),
        });
    }
}

fn collect_js_ts_node(node: Node, content: &str, result: &mut AstResult) {
    let kind = match node.kind() {
        "function_declaration" => SymbolKind::Function,
        "class_declaration" => SymbolKind::Class,
        "method_definition" => SymbolKind::Method,
        "import_statement" => {
            if let Some(import) = import_source(node, content) {
                result.imports.push(import);
            }
            return;
        }
        _ => return,
    };

    if let Some(name) = symbol_name(node, content) {
        result.symbols.push(Symbol {
            name,
            kind,
            line: node.start_position().row + 1,
            end_line: Some(node.end_position().row + 1),
            signature: symbol_signature(node, content),
        });
    }
}

fn collect_python_node(node: Node, content: &str, result: &mut AstResult) {
    let kind = match node.kind() {
        "function_definition" => SymbolKind::Function,
        "class_definition" => SymbolKind::Class,
        "import_statement" | "import_from_statement" => {
            result
                .imports
                .push(node_text(node, content).trim().to_string());
            return;
        }
        _ => return,
    };

    if let Some(name) = symbol_name(node, content) {
        result.symbols.push(Symbol {
            name,
            kind,
            line: node.start_position().row + 1,
            end_line: Some(node.end_position().row + 1),
            signature: symbol_signature(node, content),
        });
    }
}

fn collect_go_node(node: Node, content: &str, result: &mut AstResult) {
    let kind = match node.kind() {
        "function_declaration" => SymbolKind::Function,
        "method_declaration" => SymbolKind::Method,
        "type_spec" => SymbolKind::Type,
        "import_spec" => {
            if let Some(import) = import_source(node, content) {
                result.imports.push(import);
            }
            return;
        }
        _ => return,
    };

    if let Some(name) = symbol_name(node, content) {
        result.symbols.push(Symbol {
            name,
            kind,
            line: node.start_position().row + 1,
            end_line: Some(node.end_position().row + 1),
            signature: symbol_signature(node, content),
        });
    }
}

fn symbol_name(node: Node, content: &str) -> Option<String> {
    node.child_by_field_name("name")
        .or_else(|| node.child_by_field_name("type"))
        .or_else(|| first_named_child_of_kind(node, "identifier"))
        .or_else(|| first_named_child_of_kind(node, "type_identifier"))
        .map(|name| node_text(name, content).trim().to_string())
        .filter(|name| !name.is_empty())
}

fn first_named_child_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn import_source(node: Node, content: &str) -> Option<String> {
    node.child_by_field_name("source")
        .or_else(|| node.child_by_field_name("path"))
        .or_else(|| first_string_child(node))
        .map(|source| trim_quotes(node_text(source, content).trim()).to_string())
        .filter(|source| !source.is_empty())
}

fn first_string_child(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind().contains("string") || child.kind().contains("literal"))
}

fn trim_quotes(value: &str) -> &str {
    value.trim_matches('"').trim_matches('\'').trim_matches('`')
}

fn symbol_signature(node: Node, content: &str) -> Option<String> {
    let text = node_text(node, content);
    text.lines()
        .next()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
}

fn node_text<'a>(node: Node, content: &'a str) -> &'a str {
    node.utf8_text(content.as_bytes()).unwrap_or("")
}

fn parse_rust(content: &str, result: &mut AstResult) {
    lazy_static_re!(
        FN_RE,
        r"^\s*(?:pub\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+(\w+)"
    );
    lazy_static_re!(STRUCT_RE, r"^\s*pub\s+struct\s+(\w+)");
    lazy_static_re!(ENUM_RE, r"^\s*pub\s+enum\s+(\w+)");
    lazy_static_re!(TRAIT_RE, r"^\s*pub\s+trait\s+(\w+)");
    lazy_static_re!(IMPL_RE, r"^\s*impl(?:<[^>]+>)?\s+(?:\w+::)*(\w+)");
    lazy_static_re!(TYPE_RE, r"^\s*pub\s+type\s+(\w+)");
    lazy_static_re!(MOD_RE, r"^\s*pub\s+mod\s+(\w+)");
    lazy_static_re!(USE_RE, r"^\s*use\s+([^;]+);");

    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;
        if let Some(cap) = FN_RE.captures(line) {
            result.symbols.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Function,
                line: line_num,
                end_line: None,
                signature: Some(line.trim().to_string()),
            });
        } else if let Some(cap) = STRUCT_RE.captures(line) {
            result.symbols.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Struct,
                line: line_num,
                end_line: None,
                signature: None,
            });
        } else if let Some(cap) = ENUM_RE.captures(line) {
            result.symbols.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Enum,
                line: line_num,
                end_line: None,
                signature: None,
            });
        } else if let Some(cap) = TRAIT_RE.captures(line) {
            result.symbols.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Trait,
                line: line_num,
                end_line: None,
                signature: None,
            });
        } else if let Some(cap) = IMPL_RE.captures(line) {
            result.symbols.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Class,
                line: line_num,
                end_line: None,
                signature: None,
            });
        } else if let Some(cap) = TYPE_RE.captures(line) {
            result.symbols.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Type,
                line: line_num,
                end_line: None,
                signature: None,
            });
        } else if let Some(cap) = MOD_RE.captures(line) {
            result.symbols.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Module,
                line: line_num,
                end_line: None,
                signature: None,
            });
        }
        if let Some(cap) = USE_RE.captures(line) {
            result.imports.push(cap[1].trim().to_string());
        }
    }
}

fn parse_js_ts(content: &str, result: &mut AstResult) {
    lazy_static_re!(FN_RE, r"^\s*(?:export\s+)?(?:async\s+)?function\s+(\w+)");
    lazy_static_re!(
        ARROW_RE,
        r"^\s*(?:export\s+)?(?:const|let|var)\s+(\w+)\s*[=:]\s*(?:async\s*)?\("
    );
    lazy_static_re!(CLASS_RE, r"^\s*(?:export\s+)?class\s+(\w+)");
    lazy_static_re!(METHOD_RE, r"^\s*(?:async\s+)?(\w+)\s*\([^)]*\)\s*\{");
    lazy_static_re!(
        IMPORT_RE,
        r#"^\s*import\s+.*?\s+from\s+['\"]([^'\"]+)['\"]"#
    );
    lazy_static_re!(
        REQ_RE,
        r#"^\s*(?:const|let|var)\s+.*?=\s*require\s*\(\s*['\"]([^'\"]+)['\"]\s*\)"#
    );

    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;
        if let Some(cap) = FN_RE.captures(line) {
            result.symbols.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Function,
                line: line_num,
                end_line: None,
                signature: Some(line.trim().to_string()),
            });
        } else if let Some(cap) = ARROW_RE.captures(line) {
            result.symbols.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Function,
                line: line_num,
                end_line: None,
                signature: Some(line.trim().to_string()),
            });
        } else if let Some(cap) = CLASS_RE.captures(line) {
            result.symbols.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Class,
                line: line_num,
                end_line: None,
                signature: None,
            });
        } else if let Some(cap) = METHOD_RE.captures(line) {
            let name = &cap[1];
            if !["if", "while", "for", "switch", "catch"].contains(&name) {
                result.symbols.push(Symbol {
                    name: name.to_string(),
                    kind: SymbolKind::Method,
                    line: line_num,
                    end_line: None,
                    signature: Some(line.trim().to_string()),
                });
            }
        }
        if let Some(cap) = IMPORT_RE.captures(line) {
            result.imports.push(cap[1].to_string());
        }
        if let Some(cap) = REQ_RE.captures(line) {
            result.imports.push(cap[1].to_string());
        }
    }
}

fn parse_python(content: &str, result: &mut AstResult) {
    lazy_static_re!(DEF_RE, r"^\s*(?:async\s+)?def\s+(\w+)");
    lazy_static_re!(CLASS_RE, r"^\s*class\s+(\w+)");
    lazy_static_re!(IMPORT_RE, r"^\s*import\s+([^#\n]+)");
    lazy_static_re!(FROM_RE, r"^\s*from\s+(\S+)\s+import");

    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;
        if let Some(cap) = DEF_RE.captures(line) {
            result.symbols.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Function,
                line: line_num,
                end_line: None,
                signature: Some(line.trim().to_string()),
            });
        } else if let Some(cap) = CLASS_RE.captures(line) {
            result.symbols.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Class,
                line: line_num,
                end_line: None,
                signature: None,
            });
        }
        if let Some(cap) = IMPORT_RE.captures(line) {
            result.imports.push(cap[1].trim().to_string());
        }
        if let Some(cap) = FROM_RE.captures(line) {
            result.imports.push(cap[1].trim().to_string());
        }
    }
}

fn parse_go(content: &str, result: &mut AstResult) {
    lazy_static_re!(FUNC_RE, r"^\s*func\s+(?:\([^)]*\)\s*)?(\w+)");
    lazy_static_re!(TYPE_RE, r"^\s*type\s+(\w+)");
    lazy_static_re!(IMPORT_RE, r#"^\s*import\s+['\"]([^'\"]+)['\"]"#);

    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;
        if let Some(cap) = FUNC_RE.captures(line) {
            result.symbols.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Function,
                line: line_num,
                end_line: None,
                signature: Some(line.trim().to_string()),
            });
        } else if let Some(cap) = TYPE_RE.captures(line) {
            result.symbols.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Type,
                line: line_num,
                end_line: None,
                signature: None,
            });
        }
        if let Some(cap) = IMPORT_RE.captures(line) {
            result.imports.push(cap[1].to_string());
        }
    }
}

fn parse_generic(content: &str, result: &mut AstResult) {
    lazy_static_re!(FN_RE, r"^\s*(?:function|func|def|fn)\s+(\w+)");

    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;
        if let Some(cap) = FN_RE.captures(line) {
            result.symbols.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Function,
                line: line_num,
                end_line: None,
                signature: Some(line.trim().to_string()),
            });
        }
    }
}

pub fn language_from_path(path: &Path) -> String {
    ext_to_language(path.extension().and_then(|e| e.to_str()).unwrap_or(""))
}

pub fn find_symbol_at_line(result: &AstResult, line: usize) -> Option<&Symbol> {
    result.symbols.iter().find(|s| s.line == line)
}

pub fn symbols_by_kind(result: &AstResult, kind: SymbolKind) -> Vec<&Symbol> {
    result.symbols.iter().filter(|s| s.kind == kind).collect()
}

pub fn all_function_names(result: &AstResult) -> Vec<String> {
    result
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function || s.kind == SymbolKind::Method)
        .map(|s| s.name.clone())
        .collect()
}

pub fn cross_reference_map(results: &HashMap<String, AstResult>) -> HashMap<String, Vec<String>> {
    let mut refs = HashMap::new();
    for (path, result) in results {
        let mut refs_for_path = Vec::new();
        for other_path in results.keys() {
            if other_path != path {
                if let Some(other) = results.get(other_path) {
                    for import in &result.imports {
                        let file_stem = std::path::Path::new(other_path)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("");
                        if import.contains(file_stem) || file_stem.contains(import) {
                            refs_for_path.push(other_path.clone());
                            break;
                        }
                    }
                    for sym in &result.symbols {
                        if other
                            .symbols
                            .iter()
                            .any(|o| o.name == sym.name && o.kind == sym.kind)
                            && !refs_for_path.contains(other_path)
                        {
                            refs_for_path.push(other_path.clone());
                        }
                    }
                }
            }
        }
        refs.insert(path.clone(), refs_for_path);
    }
    refs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rust_functions() {
        let code = r#"
pub fn foo() {}
fn bar() {}
async fn baz() {}
"#;
        let result = parse_file(Path::new("test.rs"), code);
        assert_eq!(result.symbols.len(), 3);
        assert_eq!(result.symbols[0].name, "foo");
        assert_eq!(result.symbols[1].name, "bar");
        assert_eq!(result.symbols[2].name, "baz");
    }

    #[test]
    fn parses_rust_structs_and_traits() {
        let code = r#"
pub struct Foo { a: i32 }
pub enum Bar { A, B }
pub trait Baz {}
"#;
        let result = parse_file(Path::new("test.rs"), code);
        let kinds: Vec<_> = result.symbols.iter().map(|s| &s.kind).collect();
        assert!(kinds.contains(&&SymbolKind::Struct));
        assert!(kinds.contains(&&SymbolKind::Enum));
        assert!(kinds.contains(&&SymbolKind::Trait));
    }

    #[test]
    fn parses_python_functions() {
        let code = "def foo():\n    pass\n\nclass Bar:\n    def method(self): pass\n";
        let result = parse_file(Path::new("test.py"), code);
        let names: Vec<_> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"foo"));
        assert!(names.contains(&"Bar"));
    }

    #[test]
    fn parses_js_functions() {
        let code = "function foo() {}\nconst bar = () => {}\nclass Baz {}\n";
        let result = parse_file(Path::new("test.js"), code);
        assert!(result.symbols.iter().any(|s| s.name == "foo"));
        assert!(result.symbols.iter().any(|s| s.name == "Baz"));
    }
}
