use std::path::{Component, Path, PathBuf};

pub fn normalize_path<P: AsRef<Path>>(path: P) -> String {
    let raw = path.as_ref().to_string_lossy().replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    let mut prefix = String::new();

    for part in raw.split('/') {
        if part.is_empty() {
            if prefix.is_empty() && raw.starts_with('/') {
                prefix.push('/');
            }
            continue;
        }
        match part {
            "." => {}
            ".." => {
                if let Some(last) = parts.last() {
                    if *last != ".." {
                        parts.pop();
                        continue;
                    }
                }
                parts.push(part);
            }
            _ => parts.push(part),
        }
    }

    let joined = parts.join("/");
    if prefix == "/" && !joined.starts_with('/') {
        format!("/{joined}")
    } else {
        joined
    }
}

pub fn normalize_relative_path(root: impl AsRef<Path>, path: impl AsRef<Path>) -> String {
    match path.as_ref().strip_prefix(root.as_ref()) {
        Ok(rel) => normalize_path(rel),
        Err(_) => normalize_path(path),
    }
}

pub fn path_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

pub fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

pub fn extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default()
}

pub fn parent_dir(path: &str) -> String {
    normalize_path(Path::new(path).parent().unwrap_or_else(|| Path::new("")))
}

pub fn join_normalized(base: &str, child: &str) -> String {
    if base.is_empty() {
        normalize_path(child)
    } else {
        normalize_path(PathBuf::from(base).join(child))
    }
}

pub fn path_components(path: &str) -> Vec<String> {
    Path::new(path)
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect()
}
