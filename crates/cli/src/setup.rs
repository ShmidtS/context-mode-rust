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

/// Detect whether the plugin is running from a marketplace install or a dev build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallKind {
    Marketplace,
    DevBuild,
    Unknown,
}

fn detect_install_kind() -> InstallKind {
    if let Ok(exe) = std::env::current_exe() {
        let path = exe.to_string_lossy();
        if path.contains(".claude-plugin") {
            return InstallKind::Marketplace;
        }
        if path.contains("target/release") || path.contains("target/debug") {
            return InstallKind::DevBuild;
        }
    }
    if let Ok(root) = std::env::var("CLAUDE_PLUGIN_ROOT") {
        if root.contains(".claude-plugin") {
            return InstallKind::Marketplace;
        }
    }
    InstallKind::Unknown
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

/// Register the `context-mode-server` binary in the user's PATH so that
/// Claude Code can invoke it from hooks and slash commands.
fn register_context_mode_server_binary(plugin_root: &PathBuf) -> Result<Vec<String>> {
    let mut results = Vec::new();
    let bin_dir = plugin_root.join(".claude-plugin").join("bin");
    let server_src = bin_dir.join(if cfg!(windows) {
        "context-mode-server.exe"
    } else {
        "context-mode-server"
    });

    if !server_src.exists() {
        results.push(format!(
            "Warning: context-mode-server binary not found at {} — skipping registration",
            server_src.display()
        ));
        return Ok(results);
    }

    // On Unix, create symlink in ~/.local/bin
    #[cfg(not(windows))]
    {
        let local_bin = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".local")
            .join("bin");
        if let Err(e) = fs::create_dir_all(&local_bin) {
            results.push(format!("Could not create {}: {}", local_bin.display(), e));
        } else {
            let link = local_bin.join("context-mode-server");
            if link.exists() {
                let _ = fs::remove_file(&link);
            }
            if let Err(e) = std::os::unix::fs::symlink(&server_src, &link) {
                results.push(format!(
                    "Could not symlink {} to {}: {}",
                    server_src.display(),
                    link.display(),
                    e
                ));
            } else {
                results.push(format!(
                    "Registered context-mode-server binary: {} -> {}",
                    link.display(),
                    server_src.display()
                ));
            }
        }
    }

    // On Windows, register via PATH update (claude-plugin/bin is already in PATH)
    #[cfg(windows)]
    {
        results.push(format!(
            "context-mode-server binary available at: {}",
            server_src.display()
        ));
    }

    Ok(results)
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
    let install_kind = detect_install_kind();
    println!("Install kind: {:?}", install_kind);
    println!("Plugin root: {}", plugin_root.display());

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

    // Register context-mode-server binary
    match register_context_mode_server_binary(&plugin_root) {
        Ok(msgs) => {
            for msg in msgs {
                println!("{}", msg);
            }
        }
        Err(e) => {
            println!(
                "Warning: could not register context-mode-server binary: {}",
                e
            );
        }
    }

    // Install Claude Code hooks (scripts + settings.json) and slash commands
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
