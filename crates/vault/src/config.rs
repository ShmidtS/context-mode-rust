use crate::types::VaultConfig;
use serde_json;
use std::fs;
use std::io;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("home directory not found")]
    HomeNotFound,
}

pub type Result<T> = std::result::Result<T, ConfigError>;

pub fn vault_config_dir() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or(ConfigError::HomeNotFound)?;
    Ok(home.join(".context-mode"))
}

pub fn vault_config_path() -> Result<PathBuf> {
    Ok(vault_config_dir()?.join("vaults.json"))
}

pub fn load_vault_config() -> Result<Vec<VaultConfig>> {
    let path = vault_config_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

pub fn save_vault_config(configs: &[VaultConfig]) -> Result<()> {
    let dir = vault_config_dir()?;
    fs::create_dir_all(&dir)?;
    fs::write(
        dir.join("vaults.json"),
        serde_json::to_string_pretty(configs)?,
    )?;
    Ok(())
}

pub fn add_vault_config(config: VaultConfig) -> Result<()> {
    let mut configs: Vec<VaultConfig> = load_vault_config()?
        .into_iter()
        .filter(|c| c.vault_path != config.vault_path)
        .collect();
    configs.push(config);
    save_vault_config(&configs)
}

pub fn remove_vault_config(vault_path: &str) -> Result<()> {
    let configs: Vec<VaultConfig> = load_vault_config()?
        .into_iter()
        .filter(|c| c.vault_path != vault_path)
        .collect();
    save_vault_config(&configs)
}

pub fn get_vault_config(vault_path: &str) -> Result<Option<VaultConfig>> {
    Ok(load_vault_config()?
        .into_iter()
        .find(|c| c.vault_path == vault_path))
}

pub fn list_vault_configs() -> Result<Vec<VaultConfig>> {
    load_vault_config()
}

pub fn load_configs() -> Result<Vec<VaultConfig>> {
    load_vault_config()
}
