use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::types::{AdapterError, DiagnosticResult, HookAdapter};

pub trait BaseAdapter: HookAdapter {
    fn session_dir_segments(&self) -> Vec<String>;

    fn session_dir(&self) -> Result<PathBuf, AdapterError> {
        let mut dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        for segment in self.session_dir_segments() {
            dir.push(&segment);
        }
        dir.push("context-mode");
        dir.push("sessions");
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    fn session_db_path(&self, project_dir: &str) -> Result<PathBuf, AdapterError> {
        Ok(self
            .session_dir()?
            .join(format!("{}.db", project_hash(project_dir))))
    }

    fn session_events_path(&self, project_dir: &str) -> Result<PathBuf, AdapterError> {
        Ok(self
            .session_dir()?
            .join(format!("{}-events.md", project_hash(project_dir))))
    }

    fn config_dir(&self, _project_dir: Option<&Path>) -> PathBuf {
        let mut dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        for segment in self.session_dir_segments() {
            dir.push(&segment);
        }
        dir
    }

    fn instruction_files(&self) -> Vec<String> {
        vec!["CLAUDE.md".to_string()]
    }

    fn memory_dir(&self) -> PathBuf {
        self.config_dir(None).join("memory")
    }

    fn backup_settings(&self) -> Result<Option<PathBuf>, AdapterError> {
        let settings_path = self.settings_path();
        if !settings_path.exists() {
            return Ok(None);
        }
        let backup_path = settings_path.with_extension(
            settings_path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| format!("{ext}.bak"))
                .unwrap_or_else(|| "bak".to_string()),
        );
        fs::copy(&settings_path, &backup_path)?;
        Ok(Some(backup_path))
    }

    fn read_settings(&self) -> Result<Option<serde_json::Value>, AdapterError> {
        let settings_path = self.settings_path();
        if !settings_path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(settings_path)?;
        Ok(Some(serde_json::from_str(&raw)?))
    }

    fn write_settings(&self, settings: &serde_json::Value) -> Result<(), AdapterError> {
        let settings_path = self.settings_path();
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(settings_path, serde_json::to_string_pretty(settings)?)?;
        Ok(())
    }

    fn check_plugin_registration(&self) -> DiagnosticResult;
}

pub fn project_hash(project_dir: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(project_dir.as_bytes());
    hex::encode(hasher.finalize())[..16].to_string()
}
