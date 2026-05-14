use crate::path_utils::{extension, join_normalized, normalize_path, parent_dir, path_stem};
use crate::types::{ImportEntry, ImportKind, ParsedCodeFile};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::Path;

const CODE_EXTS: &[&str] = &[".ts", ".js", ".mjs", ".cjs"];

fn sha256_hex(content: &str) -> String {
    hex::encode(Sha256::digest(content.as_bytes()))
}

fn derive_tags_from_path(file_path: &str) -> Vec<String> {
    file_path
        .replace('\\', "/")
        .split('/')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .skip(1)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .filter(|p| !p.is_empty() && !p.starts_with('.'))
        .map(ToString::to_string)
        .collect()
}

fn derive_title(file_path: &str) -> String {
    let stem = path_stem(file_path);
    if matches!(
        extension(file_path).as_str(),
        ".ts" | ".js" | ".mjs" | ".cjs"
    ) {
        stem
    } else {
        Path::new(file_path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| file_path.to_string())
    }
}

fn try_resolve_base(base: &str, all_paths: &HashSet<String>) -> Option<String> {
    let base = normalize_path(base);
    if all_paths.contains(&base) {
        return Some(base);
    }

    if CODE_EXTS.iter().any(|ext| base.ends_with(ext)) {
        if let Some(dot) = base.rfind('.') {
            let stem = &base[..dot];
            for ext in CODE_EXTS {
                let candidate = format!("{stem}{ext}");
                if all_paths.contains(&candidate) {
                    return Some(candidate);
                }
            }
        }
    }

    for ext in CODE_EXTS {
        let candidate = format!("{base}{ext}");
        if all_paths.contains(&candidate) {
            return Some(candidate);
        }
    }

    for ext in CODE_EXTS {
        let candidate = join_normalized(&base, &format!("index{ext}"));
        if all_paths.contains(&candidate) {
            return Some(candidate);
        }
    }

    None
}

fn resolve_import_specifier(
    specifier: &str,
    source_dir: &str,
    all_paths: Option<&HashSet<String>>,
    vault_root: Option<&str>,
) -> Option<String> {
    let all_paths = all_paths?;
    if specifier.starts_with('.') {
        return try_resolve_base(&join_normalized(source_dir, specifier), all_paths);
    }
    if let Some(vault_root) = vault_root {
        let abs_base = Path::new(vault_root).join(specifier);
        if let Ok(rel) = abs_base.strip_prefix(vault_root) {
            return try_resolve_base(&normalize_path(rel), all_paths);
        }
    }
    None
}

fn context_slice(line: &str, start: usize, len: usize) -> String {
    let ctx_start = start.saturating_sub(60);
    let ctx_end = (start + len + 60).min(line.len());
    line.get(ctx_start..ctx_end).unwrap_or(line).to_string()
}

struct Handler {
    regex: Regex,
    extensions: &'static [&'static str],
    multi_capture: bool,
    kind: ImportKind,
    resolve_path: bool,
    external_mode: ExternalMode,
}

#[derive(Clone, Copy)]
enum ExternalMode {
    Auto,
    Always,
    Never,
}

fn handlers() -> Vec<Handler> {
    vec![
        Handler { regex: Regex::new(r#"import\s+.*?\s+from\s+['\"]([^'\"]+)['\"];?"#).unwrap(), extensions: &[".ts", ".js", ".mjs", ".cjs"], multi_capture: false, kind: ImportKind::Static, resolve_path: true, external_mode: ExternalMode::Auto },
        Handler { regex: Regex::new(r#"import\s*\(\s*['\"]([^'\"]+)['\"]\s*\)"#).unwrap(), extensions: &[".ts", ".js", ".mjs", ".cjs"], multi_capture: false, kind: ImportKind::Dynamic, resolve_path: true, external_mode: ExternalMode::Auto },
        Handler { regex: Regex::new(r#"require\s*\(\s*['\"]([^'\"]+)['\"]\s*\)"#).unwrap(), extensions: &[".ts", ".js", ".mjs", ".cjs"], multi_capture: false, kind: ImportKind::Require, resolve_path: true, external_mode: ExternalMode::Auto },
        Handler { regex: Regex::new(r#"export\s+(?:\{[^}]*\}|(?:type\s+)?\*)\s+from\s+['\"]([^'\"]+)['\"];?"#).unwrap(), extensions: &[".ts", ".js", ".mjs", ".cjs"], multi_capture: false, kind: ImportKind::ExportFrom, resolve_path: true, external_mode: ExternalMode::Auto },
        Handler { regex: Regex::new(r"\b(?:from\s+([a-zA-Z_][a-zA-Z0-9_.]*)\s+import|import\s+([a-zA-Z_][a-zA-Z0-9_.]*(?:\s*,\s*[a-zA-Z_][a-zA-Z0-9_.]*)*))").unwrap(), extensions: &[".py", ".pyi", ".pyw"], multi_capture: true, kind: ImportKind::Static, resolve_path: true, external_mode: ExternalMode::Auto },
        Handler { regex: Regex::new(r#"import\s+(?:\(\s*(?:[_a-zA-Z][a-zA-Z0-9_]*\s+)?\"([^\"]+)\"[^)]*\)|(?:[_a-zA-Z][a-zA-Z0-9_]*\s+)?\"([^\"]+)\")"#).unwrap(), extensions: &[".go"], multi_capture: true, kind: ImportKind::Static, resolve_path: true, external_mode: ExternalMode::Always },
        Handler { regex: Regex::new(r"\buse\s+([a-zA-Z_][a-zA-Z0-9_:]*)\s*;").unwrap(), extensions: &[".rs"], multi_capture: false, kind: ImportKind::Static, resolve_path: false, external_mode: ExternalMode::Always },
        Handler { regex: Regex::new(r"\bextern\s+crate\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*;").unwrap(), extensions: &[".rs"], multi_capture: false, kind: ImportKind::Static, resolve_path: false, external_mode: ExternalMode::Always },
        Handler { regex: Regex::new(r"\bmod\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*;").unwrap(), extensions: &[".rs"], multi_capture: false, kind: ImportKind::Static, resolve_path: true, external_mode: ExternalMode::Never },
        Handler { regex: Regex::new(r"\bimport\s+([a-zA-Z_][a-zA-Z0-9_.*]*)\s*;").unwrap(), extensions: &[".java", ".kt", ".kts", ".scala", ".sc", ".cs"], multi_capture: false, kind: ImportKind::Static, resolve_path: false, external_mode: ExternalMode::Always },
        Handler { regex: Regex::new(r#"#include\s+[<\"]([^>\"]+)[>\"]"#).unwrap(), extensions: &[".c", ".h", ".cpp", ".cxx", ".cc", ".hpp", ".hxx"], multi_capture: false, kind: ImportKind::Static, resolve_path: true, external_mode: ExternalMode::Auto },
        Handler { regex: Regex::new(r#"\b(?:include|require|include_once|require_once)\s*\(?\s*['\"]([^'\"]+)['\"]\s*\)?"#).unwrap(), extensions: &[".php", ".phtml", ".php3", ".php4", ".php5", ".phps"], multi_capture: false, kind: ImportKind::Require, resolve_path: true, external_mode: ExternalMode::Auto },
        Handler { regex: Regex::new(r"\buse\s+([a-zA-Z_][a-zA-Z0-9_\\]*)\s*;").unwrap(), extensions: &[".php", ".phtml", ".php3", ".php4", ".php5", ".phps"], multi_capture: false, kind: ImportKind::Static, resolve_path: false, external_mode: ExternalMode::Always },
        Handler { regex: Regex::new(r#"\b(?:require|require_relative|load)\s*\(?\s*['\"]([^'\"]+)['\"]\s*\)?"#).unwrap(), extensions: &[".rb", ".rbx", ".ru"], multi_capture: false, kind: ImportKind::Require, resolve_path: true, external_mode: ExternalMode::Auto },
        Handler { regex: Regex::new(r#"\b(?:require|dofile|loadfile)\s*\(\s*['\"]([^'\"]+)['\"]\s*\)"#).unwrap(), extensions: &[".lua"], multi_capture: false, kind: ImportKind::Require, resolve_path: true, external_mode: ExternalMode::Auto },
        Handler { regex: Regex::new(r"(?:^|\n)\s*(?:source|\.\s)\s+([^;\s&|><]+)").unwrap(), extensions: &[".sh", ".bash", ".zsh", ".fish", ".ksh", ".csh", ".tcsh"], multi_capture: false, kind: ImportKind::Require, resolve_path: true, external_mode: ExternalMode::Auto },
        Handler { regex: Regex::new(r#"<(?:script|img|video|audio|source|iframe)\s+[^>]*\bsrc\s*=\s*['\"]([^'\"]+)['\"][^>]*>"#).unwrap(), extensions: &[".html", ".htm", ".xhtml"], multi_capture: false, kind: ImportKind::Static, resolve_path: true, external_mode: ExternalMode::Never },
        Handler { regex: Regex::new(r#"<(?:link|a|area)\s+[^>]*\b(?:href|xlink:href)\s*=\s*['\"]([^'\"]+)['\"][^>]*>"#).unwrap(), extensions: &[".html", ".htm", ".xhtml"], multi_capture: false, kind: ImportKind::Static, resolve_path: true, external_mode: ExternalMode::Never },
        Handler { regex: Regex::new(r#"@import\s+(?:url\s*\(\s*)?['\"]([^'\"]+)['\"]\s*\)?"#).unwrap(), extensions: &[".css", ".scss", ".sass", ".less", ".styl", ".stylus"], multi_capture: false, kind: ImportKind::Static, resolve_path: true, external_mode: ExternalMode::Auto },
        Handler { regex: Regex::new(r#"@(?:use|forward)\s+['\"]([^'\"]+)['\"]"#).unwrap(), extensions: &[".css", ".scss", ".sass", ".less", ".styl", ".stylus"], multi_capture: false, kind: ImportKind::Static, resolve_path: true, external_mode: ExternalMode::Auto },
    ]
}

fn compute_is_external(
    mode: ExternalMode,
    specifier: &str,
    resolved_path: &Option<String>,
) -> bool {
    match mode {
        ExternalMode::Auto => !specifier.starts_with('.') && resolved_path.is_none(),
        ExternalMode::Always => true,
        ExternalMode::Never => false,
    }
}

pub fn parse_code_file(
    file_path: impl Into<String>,
    content: &str,
    all_paths: Option<&HashSet<String>>,
    vault_root: Option<&str>,
) -> ParsedCodeFile {
    let file_path = file_path.into();
    let content_hash = sha256_hex(content);
    let title = derive_title(&file_path);
    let tags = derive_tags_from_path(&file_path);
    let source_dir = parent_dir(&file_path);
    let ext = extension(&file_path);
    let mut imports = Vec::new();

    for (idx, line) in content.split('\n').enumerate() {
        for handler in handlers()
            .into_iter()
            .filter(|h| h.extensions.contains(&ext.as_str()))
        {
            for caps in handler.regex.captures_iter(line) {
                let specifier = if handler.multi_capture {
                    caps.get(1).or_else(|| caps.get(2))
                } else {
                    caps.get(1)
                };
                let Some(specifier) = specifier.map(|m| m.as_str()) else {
                    continue;
                };
                let resolved_path = if handler.resolve_path {
                    resolve_import_specifier(specifier, &source_dir, all_paths, vault_root)
                } else {
                    None
                };
                let whole = caps.get(0).expect("whole match");
                imports.push(ImportEntry {
                    specifier: specifier.to_string(),
                    resolved_path: resolved_path.clone(),
                    line_number: idx + 1,
                    context: context_slice(line, whole.start(), whole.len()),
                    kind: handler.kind.clone(),
                    is_external: compute_is_external(
                        handler.external_mode,
                        specifier,
                        &resolved_path,
                    ),
                });
            }
        }
    }

    ParsedCodeFile {
        path: file_path,
        title,
        tags,
        imports,
        content_hash,
    }
}
