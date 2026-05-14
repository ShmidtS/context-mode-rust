use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use context_mode_core::db_schema;
use rusqlite::Connection;

fn context_mode_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir().context("Could not determine config directory")?;
    Ok(config_dir.join("context-mode"))
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

    Ok(())
}
