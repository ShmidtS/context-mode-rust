use std::path::PathBuf;

use crate::base::BaseAdapter;
use crate::detect::get_session_dir_segments;
use crate::types::{AdapterError, DiagnosticResult, DiagnosticStatus, HookAdapter, PlatformId};

pub struct OpenCodeAdapter;

impl HookAdapter for OpenCodeAdapter {
    fn install(&self, _plugin_root: &str) -> Result<Vec<String>, AdapterError> {
        Ok(Vec::new())
    }

    fn uninstall(&self) -> Result<Vec<String>, AdapterError> {
        Ok(Vec::new())
    }

    fn diagnostics(&self, _plugin_root: &str) -> Result<Vec<DiagnosticResult>, AdapterError> {
        Ok(Vec::new())
    }

    fn settings_path(&self) -> PathBuf {
        let mut dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        dir.push(".config");
        dir.push("opencode");
        dir.push("settings.json");
        dir
    }

    fn hook_paths(&self, _plugin_root: &str) -> Vec<PathBuf> {
        Vec::new()
    }

    fn platform_id(&self) -> PlatformId {
        PlatformId::OpenCode
    }
}

impl BaseAdapter for OpenCodeAdapter {
    fn session_dir_segments(&self) -> Vec<String> {
        get_session_dir_segments(PlatformId::OpenCode).unwrap_or_default()
    }

    fn check_plugin_registration(&self) -> DiagnosticResult {
        let config_dir = self.config_dir(None);
        let exists = config_dir.exists();
        DiagnosticResult {
            check: "opencode config directory".to_string(),
            status: if exists {
                DiagnosticStatus::Pass
            } else {
                DiagnosticStatus::Warn
            },
            message: if exists {
                format!("Config directory found: {}", config_dir.display())
            } else {
                format!("Config directory not found: {}", config_dir.display())
            },
            fix: if exists {
                None
            } else {
                Some("Install OpenCode and run it to initialize configuration.".to_string())
            },
        }
    }
}
