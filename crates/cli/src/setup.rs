use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use context_mode_adapters::platforms::claude_code::ClaudeCodeAdapter;
use context_mode_adapters::types::HookAdapter;
use context_mode_core::db_schema;
use rusqlite::Connection;

fn context_mode_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir().context("Could not determine config directory")?;
    Ok(config_dir.join("context-mode"))
}

/// Determine plugin root from env var, executable location, or current dir.
/// When installed from marketplace the binary lives in
/// <plugin_root>/.claude-plugin/bin/context-mode.exe; when running a
/// dev build it is in <plugin_root>/target/release/context-mode.exe.
fn resolve_plugin_root() -> PathBuf {
    if let Ok(root) = std::env::var("CLAUDE_PLUGIN_ROOT") {
        return PathBuf::from(root);
    }

    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(PathBuf::from).unwrap_or_default();
        // Walk up looking for .claude-plugin/plugin.json (marketplace) or
        // Cargo.toml (dev workspace root).
        loop {
            if dir.join(".claude-plugin").join("plugin.json").exists()
                || dir.join("Cargo.toml").exists()
                || dir.join("start.mjs").exists()
            {
                return dir;
            }
            if !dir.pop() {
                break;
            }
        }
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn run() -> Result<()> {
    let config_dir = context_mode_dir()?;
    if config_dir.exists() {
        println!("Directory already exists: {}", config_dir.display());
    } else {
        fs::create_dir_all(&config_dir)
            .with_context(|| format!("Failed to create {}", config_dir.display()))?;
        println!("Created directory: {}", config_dir.display());
    }

    let db_path = config_dir.join("context-mode.db");
    if db_path.exists() {
        println!("Database already exists: {}", db_path.display());
    } else {
        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to create {}", db_path.display()))?;
        println!("Created database: {}", db_path.display());
        db_schema::init_local_schema(&conn).context("Failed to initialize database schema")?;
        println!("Initialized database schema: {}", db_path.display());
    }

    let connectors_path = config_dir.join("connectors.json");
    if connectors_path.exists() {
        println!(
            "Connectors file already exists: {}",
            connectors_path.display()
        );
    } else {
        fs::write(&connectors_path, "[]")
            .with_context(|| format!("Failed to create {}", connectors_path.display()))?;
        println!("Created connectors file: {}", connectors_path.display());
    }

    let plugin_root = resolve_plugin_root();

    // On Windows, remove Unix shell-script shims that clash with .exe resolution
    #[cfg(windows)]
    {
        let bin_dir = plugin_root.join(".claude-plugin").join("bin");
        for name in [
            "context-mode",
            "context-mode-server",
            "context-mode-insight",
        ] {
            let shim = bin_dir.join(name);
            if shim.exists() {
                if let Ok(meta) = std::fs::metadata(&shim) {
                    if meta.len() < 4096 {
                        let _ = std::fs::remove_file(&shim);
                        println!("Removed Unix shim on Windows: {}", shim.display());
                    }
                }
            }
        }
    }

    // Install Claude Code hooks (scripts + settings.json)
    let adapter = ClaudeCodeAdapter;
    let plugin_root_str = plugin_root.to_string_lossy().to_string();
    match adapter.install(&plugin_root_str) {
        Ok(messages) => {
            for msg in messages {
                println!("{}", msg);
            }
        }
        Err(e) => {
            println!("Warning: could not install Claude Code hooks: {}", e);
        }
    }

    Ok(())
}
