use crate::path_utils::normalize_path;
use std::collections::HashSet;

pub fn resolve_link(
    target_name: &str,
    _source_dir: &str,
    _vault_root: &str,
    all_paths: &HashSet<String>,
) -> Option<String> {
    let target = target_name.trim().trim_matches('/').replace('\\', "/");
    if target.is_empty() {
        return None;
    }

    let exact = normalize_path(format!("{target}.md"));
    for path in all_paths {
        if path_equals(path, &exact) {
            return Some(path.clone());
        }
    }

    let target_as_path = normalize_path(format!("{target}.md"));
    for path in all_paths {
        if path_equals(path, &target_as_path) {
            return Some(path.clone());
        }
    }

    let suffix = normalize_path(format!("/{target}.md"));
    let mut candidates = Vec::new();
    for path in all_paths {
        let haystack = if cfg!(windows) {
            path.to_lowercase()
        } else {
            path.clone()
        };
        let needle = if cfg!(windows) {
            suffix.to_lowercase()
        } else {
            suffix.clone()
        };
        if haystack.ends_with(&needle) {
            candidates.push(path.clone());
        }
    }

    candidates.sort_by(|a, b| {
        let depth_a = a.split('/').count();
        let depth_b = b.split('/').count();
        depth_a.cmp(&depth_b).then_with(|| a.cmp(b))
    });
    candidates.into_iter().next()
}

fn path_equals(a: &str, b: &str) -> bool {
    let a = normalize_path(a);
    let b = normalize_path(b);
    if cfg!(windows) {
        a.eq_ignore_ascii_case(&b)
    } else {
        a == b
    }
}
