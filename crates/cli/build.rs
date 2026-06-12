use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let crate_root = std::path::PathBuf::from(manifest_dir);
    let project_root = crate_root.parent().unwrap().parent().unwrap();
    let plugin_dir = project_root.join(".claude-plugin");

    for dir in &["skills", "hooks"] {
        let src = project_root.join(dir);
        let dst = plugin_dir.join(dir);
        if src.exists() {
            let _ = copy_dir_all(&src, &dst);
        }
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_all(&path, &dest)?;
        } else {
            fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}
