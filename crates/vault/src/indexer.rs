use crate::code_parser::parse_code_file;
use crate::graph_store::{GraphStoreError, VaultGraphStore};
use crate::markdown_parser::parse_vault_note;
use crate::path_utils::{
    extension, file_name, join_normalized, normalize_relative_path, parent_dir,
};
use crate::resolver::resolve_link;
use crate::types::{LinkType, VaultConfidence, VaultEdgeInput, VaultNodeInput};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Error)]
pub enum IndexerError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("graph store error: {0}")]
    Store(#[from] GraphStoreError),
    #[error("walkdir error: {0}")]
    Walkdir(#[from] walkdir::Error),
}

pub type Result<T> = std::result::Result<T, IndexerError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct IndexOpts {
    pub exclude_patterns: Option<Vec<String>>,
    pub reindex: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct IndexResult {
    pub indexed: usize,
    pub updated: usize,
    pub skipped: usize,
    pub broken_links: usize,
}

const DEFAULT_EXCLUDE_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    ".omc",
    "dist",
    "build",
    "coverage",
    ".claude",
    ".obsidian",
];
const CODE_EXTENSIONS: &[&str] = &[
    ".ts", ".js", ".mjs", ".cjs", ".py", ".pyi", ".pyw", ".go", ".rs", ".java", ".kt", ".kts",
    ".scala", ".sc", ".swift", ".c", ".h", ".cpp", ".cxx", ".cc", ".hpp", ".hxx", ".cs", ".rb",
    ".rbx", ".ru", ".php", ".phtml", ".php3", ".php4", ".php5", ".phps", ".lua", ".sh", ".bash",
    ".zsh", ".fish", ".ksh", ".csh", ".tcsh", ".ps1", ".psm1", ".psd1", ".html", ".htm", ".xhtml",
    ".css", ".scss", ".sass", ".less", ".styl", ".stylus", ".sql",
];
const BINARY_EXTENSIONS: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".ico", ".webp", ".svg", ".mp3", ".mp4", ".avi",
    ".mov", ".wav", ".ogg", ".flac", ".zip", ".tar", ".gz", ".rar", ".7z", ".bz2", ".xz", ".tgz",
    ".exe", ".dll", ".so", ".dylib", ".woff", ".woff2", ".ttf", ".eot", ".otf", ".pdf", ".doc",
    ".docx", ".xls", ".xlsx", ".ppt", ".pptx", ".sqlite", ".db", ".bin", ".dat", ".class", ".jar",
    ".war", ".ear", ".o", ".a", ".obj", ".lib", ".pyc", ".pyo", ".pyd", ".egg", ".whl", ".gem",
    ".deb", ".rpm", ".msi", ".dmg", ".iso", ".img",
];

pub fn is_code_extension(ext: &str) -> bool {
    CODE_EXTENSIONS.contains(&ext)
}

fn is_binary_extension(ext: &str) -> bool {
    BINARY_EXTENSIONS.contains(&ext)
}

fn should_skip(entry: &DirEntry, exclude_dirs: &HashSet<String>) -> bool {
    entry.file_type().is_dir()
        && exclude_dirs.contains(&entry.file_name().to_string_lossy().to_string())
}

fn collect_source_files(vault_root: &Path, opts: &IndexOpts) -> Result<Vec<String>> {
    let exclude_dirs: HashSet<String> = opts
        .exclude_patterns
        .clone()
        .unwrap_or_else(|| DEFAULT_EXCLUDE_DIRS.iter().map(|s| s.to_string()).collect())
        .into_iter()
        .collect();
    let walker = WalkDir::new(vault_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !should_skip(e, &exclude_dirs));
    let mut files = Vec::new();
    for entry in walker {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = normalize_relative_path(vault_root, entry.path());
        if !is_binary_extension(&extension(&rel)) {
            files.push(rel);
        }
    }
    files.sort();
    Ok(files)
}

fn read_file_text(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    let utf8 = String::from_utf8_lossy(bytes).to_string();
    let replacement_count = utf8.matches('�').count();
    if replacement_count > utf8.len() / 100 {
        Ok(bytes.iter().map(|b| *b as char).collect())
    } else {
        Ok(utf8)
    }
}

fn sha256_hex(content: &str) -> String {
    hex::encode(Sha256::digest(content.as_bytes()))
}

fn mtime_ms(meta: &fs::Metadata) -> f64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

pub fn index_vault(
    vault_root: impl AsRef<Path>,
    store: &VaultGraphStore,
    opts: Option<IndexOpts>,
) -> Result<IndexResult> {
    let opts = opts.unwrap_or_default();
    let vault_root = vault_root.as_ref();
    let mut result = IndexResult::default();
    let all_files = collect_source_files(vault_root, &opts)?;
    let all_paths: HashSet<String> = all_files.iter().cloned().collect();

    let md_files: Vec<String> = all_files
        .iter()
        .filter(|f| f.ends_with(".md"))
        .cloned()
        .collect();
    let code_files: Vec<String> = all_files
        .iter()
        .filter(|f| is_code_extension(&extension(f)))
        .cloned()
        .collect();

    let mut notes = Vec::new();
    for rel_path in &md_files {
        let abs_path = vault_root.join(rel_path);
        let meta = match fs::metadata(&abs_path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mtime = mtime_ms(&meta);
        let existing = store.get_node(rel_path)?;
        if !opts.reindex
            && existing
                .as_ref()
                .is_some_and(|n| (n.mtime_ms - mtime).abs() < f64::EPSILON)
        {
            result.skipped += 1;
            continue;
        }
        let content = match read_file_text(&abs_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let parsed = parse_vault_note(rel_path.clone(), &content);
        let had_existing = existing.is_some();
        if !opts.reindex {
            if let Some(mut node) = existing.filter(|n| n.content_hash == parsed.content_hash) {
                node.mtime_ms = mtime;
                store.upsert_node_input(&node)?;
                result.skipped += 1;
                continue;
            }
        }
        notes.push((rel_path.clone(), mtime, parsed, had_existing));
    }

    for (_, mtime, parsed, existed) in &notes {
        store.upsert_node_input(&VaultNodeInput {
            path: parsed.path.clone(),
            title: parsed.title.clone(),
            frontmatter: parsed.frontmatter.clone(),
            tags: parsed.tags.clone(),
            content_hash: parsed.content_hash.clone(),
            mtime_ms: *mtime,
            in_degree: 0,
        })?;
        if *existed {
            result.updated += 1;
        } else {
            result.indexed += 1;
        }
    }

    let mut codes = Vec::new();
    for rel_path in &code_files {
        let abs_path = vault_root.join(rel_path);
        let meta = match fs::metadata(&abs_path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mtime = mtime_ms(&meta);
        let existing = store.get_node(rel_path)?;
        if !opts.reindex
            && existing
                .as_ref()
                .is_some_and(|n| (n.mtime_ms - mtime).abs() < f64::EPSILON)
        {
            result.skipped += 1;
            continue;
        }
        let content = match read_file_text(&abs_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let parsed = parse_code_file(
            rel_path.clone(),
            &content,
            Some(&all_paths),
            vault_root.to_str(),
        );
        let had_existing = existing.is_some();
        if !opts.reindex {
            if let Some(mut node) = existing.filter(|n| n.content_hash == parsed.content_hash) {
                node.mtime_ms = mtime;
                store.upsert_node_input(&node)?;
                result.skipped += 1;
                continue;
            }
        }
        codes.push((rel_path.clone(), mtime, parsed, had_existing));
    }

    for (_, mtime, parsed, existed) in &codes {
        store.upsert_node_input(&VaultNodeInput {
            path: parsed.path.clone(),
            title: parsed.title.clone(),
            frontmatter: Default::default(),
            tags: parsed.tags.clone(),
            content_hash: parsed.content_hash.clone(),
            mtime_ms: *mtime,
            in_degree: 0,
        })?;
        if *existed {
            result.updated += 1;
        } else {
            result.indexed += 1;
        }
    }

    for rel_path in all_files
        .iter()
        .filter(|f| !f.ends_with(".md") && !is_code_extension(&extension(f)))
    {
        let abs_path = vault_root.join(rel_path);
        let meta = match fs::metadata(&abs_path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mtime = mtime_ms(&meta);
        let existing = store.get_node(rel_path)?;
        if !opts.reindex
            && existing
                .as_ref()
                .is_some_and(|n| (n.mtime_ms - mtime).abs() < f64::EPSILON)
        {
            result.skipped += 1;
            continue;
        }
        let content_hash = sha256_hex(&format!("{rel_path}{mtime}"));
        if !opts.reindex
            && existing
                .as_ref()
                .is_some_and(|n| n.content_hash == content_hash)
        {
            let mut node = existing.unwrap();
            node.mtime_ms = mtime;
            store.upsert_node_input(&node)?;
            result.skipped += 1;
            continue;
        }
        let tags: Vec<String> = rel_path
            .replace('\\', "/")
            .split('/')
            .rev()
            .skip(1)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .filter(|p| !p.is_empty() && !p.starts_with('.'))
            .map(ToString::to_string)
            .collect();
        store.upsert_node_input(&VaultNodeInput {
            path: rel_path.clone(),
            title: file_name(rel_path),
            frontmatter: Default::default(),
            tags,
            content_hash,
            mtime_ms: mtime,
            in_degree: 0,
        })?;
        if existing.is_some() {
            result.updated += 1;
        } else {
            result.indexed += 1;
        }
    }

    for (rel_path, _, parsed, _) in &notes {
        store.remove_edges_from(rel_path)?;
        let source_dir = parent_dir(rel_path);
        for wl in &parsed.wiki_links {
            let target_path = resolve_link(
                &wl.target,
                &source_dir,
                vault_root.to_string_lossy().as_ref(),
                &all_paths,
            );
            if target_path.is_none() {
                result.broken_links += 1;
            }
            store.upsert_edge_input(&VaultEdgeInput {
                source_path: rel_path.clone(),
                target_path,
                link_type: wl.link_type.clone(),
                alias: wl.alias.clone(),
                target_name: Some(wl.target.clone()),
                context: wl.context.clone(),
                line_number: wl.line_number as i64,
                confidence: Some(VaultConfidence::EXTRACTED),
            })?;
        }
        for ml in &parsed.markdown_links {
            let md_target = join_normalized(&source_dir, &ml.target);
            let resolved = all_paths.contains(&md_target).then_some(md_target);
            if resolved.is_none() {
                result.broken_links += 1;
            }
            let is_code_link = is_code_extension(&extension(&ml.target));
            store.upsert_edge_input(&VaultEdgeInput {
                source_path: rel_path.clone(),
                target_path: resolved,
                link_type: if is_code_link {
                    LinkType::Reference
                } else {
                    LinkType::Markdown
                },
                alias: None,
                target_name: Some(ml.target.clone()),
                context: ml.context.clone(),
                line_number: ml.line_number as i64,
                confidence: Some(VaultConfidence::EXTRACTED),
            })?;
        }
    }

    for (rel_path, _, parsed, _) in &codes {
        store.remove_edges_from(rel_path)?;
        for imp in &parsed.imports {
            let (link_type, target_path, target_name) = if let Some(path) = &imp.resolved_path {
                (LinkType::Import, Some(path.clone()), Some(path.clone()))
            } else if imp.is_external {
                (LinkType::External, None, Some(imp.specifier.clone()))
            } else {
                (LinkType::Import, None, Some(imp.specifier.clone()))
            };
            store.upsert_edge_input(&VaultEdgeInput {
                source_path: rel_path.clone(),
                target_path,
                link_type,
                alias: None,
                target_name,
                context: imp.context.clone(),
                line_number: imp.line_number as i64,
                confidence: Some(VaultConfidence::EXTRACTED),
            })?;
        }
    }

    Ok(result)
}


