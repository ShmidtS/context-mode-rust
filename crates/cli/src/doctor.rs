use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, OpenFlags};

fn context_mode_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir().context("Could not determine config directory")?;
    Ok(config_dir.join("context-mode"))
}

pub async fn run() -> Result<()> {
    println!("Context-mode doctor report");

    let config_dir = context_mode_dir()?;
    report("Config directory", check_config_dir(&config_dir));

    let db_path = config_dir.join("context-mode.db");
    report("Database readable/writable", check_database(&db_path));
    report("SQLite FTS5", check_fts5(&db_path));
    report("Ollama API", check_ollama().await);
    report(
        "context-mode-server in PATH",
        check_executable_in_path("context-mode-server"),
    );
    report(
        "Claude Code settings hooks",
        check_claude_code_settings_hooks(),
    );

    Ok(())
}

fn check_claude_code_settings_hooks() -> Result<()> {
    let settings_path = dirs::home_dir()
        .ok_or_else(|| anyhow!("Could not determine home directory"))?
        .join(".claude")
        .join("settings.json");

    if !settings_path.exists() {
        return Err(anyhow!("{} does not exist", settings_path.display()));
    }

    let raw = fs::read_to_string(&settings_path)
        .with_context(|| format!("Could not read {}", settings_path.display()))?;
    let settings: serde_json::Value = serde_json::from_str(&raw)
        .with_context(|| format!("Could not parse {}", settings_path.display()))?;

    let hooks = settings
        .get("hooks")
        .ok_or_else(|| anyhow!("No 'hooks' key in {}", settings_path.display()))?;

    let posttooluse = hooks
        .get("PostToolUse")
        .ok_or_else(|| anyhow!("No PostToolUse hook in settings"))?;

    let has_context_mode = posttooluse
        .as_array()
        .map(|arr| {
            arr.iter().any(|entry| {
                entry
                    .get("hooks")
                    .and_then(|h| h.as_array())
                    .map(|hooks| {
                        hooks.iter().any(|hook| {
                            hook.get("command")
                                .and_then(|c| c.as_str())
                                .map(|s| {
                                    s.contains("context-mode")
                                        && s.contains("hook")
                                        && s.contains("claude-code")
                                })
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    if !has_context_mode {
        return Err(anyhow!(
            "PostToolUse hook in {} does not contain context-mode command",
            settings_path.display()
        ));
    }

    Ok(())
}

fn report(label: &str, result: Result<()>) {
    match result {
        Ok(()) => println!("\x1b[32m✓\x1b[0m {}", label),
        Err(err) => println!("\x1b[31m✗\x1b[0m {}: {}", label, err),
    }
}

fn check_config_dir(path: &Path) -> Result<()> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(anyhow!("{} does not exist", path.display()))
    }
}

fn check_database(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(anyhow!("{} does not exist", path.display()));
    }

    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .with_context(|| format!("Could not open {} for read/write", path.display()))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS __doctor_write_check (id INTEGER PRIMARY KEY);\
         DROP TABLE __doctor_write_check;",
    )
    .context("Database write check failed")?;
    Ok(())
}

fn check_fts5(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(anyhow!("{} does not exist", path.display()));
    }

    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("Could not open {}", path.display()))?;
    let mut stmt = conn
        .prepare("PRAGMA compile_options")
        .context("Could not read SQLite compile options")?;
    let options = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    if options.iter().any(|option| option == "ENABLE_FTS5") {
        Ok(())
    } else {
        Err(anyhow!("ENABLE_FTS5 not found in SQLite compile options"))
    }
}

async fn check_ollama() -> Result<()> {
    let host = env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
    let url = format!("{}/api/tags", host.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .context("Could not build HTTP client")?;
    let response = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("Could not reach {}", url))?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(anyhow!("{} returned {}", url, response.status()))
    }
}

fn check_executable_in_path(name: &str) -> Result<()> {
    let path = env::var_os("PATH").ok_or_else(|| anyhow!("PATH is not set"))?;
    let candidates = executable_candidates(name);

    for dir in env::split_paths(&path) {
        for candidate in &candidates {
            if dir.join(candidate).is_file() {
                return Ok(());
            }
        }
    }

    Err(anyhow!("{} not found", name))
}

fn executable_candidates(name: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        let has_extension = Path::new(name).extension().is_some();
        if has_extension {
            return vec![name.to_string()];
        }

        let pathext = env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
        pathext
            .split(';')
            .filter(|ext| !ext.is_empty())
            .map(|ext| format!("{}{}", name, ext.to_ascii_lowercase()))
            .chain(std::iter::once(name.to_string()))
            .collect()
    }

    #[cfg(not(windows))]
    {
        vec![name.to_string()]
    }
}
