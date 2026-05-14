use std::path::PathBuf;

use crate::base::BaseAdapter;
use crate::detect::get_session_dir_segments;
use crate::types::{AdapterError, DiagnosticResult, DiagnosticStatus, HookAdapter, PlatformId};

pub struct ClaudeCodeAdapter;

impl HookAdapter for ClaudeCodeAdapter {
    fn install(&self, _plugin_root: &str) -> Result<Vec<String>, AdapterError> {
        let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let installer = crate::hook_runtime::HookInstaller::new(base);
        let hook_types = [
            "posttooluse",
            "pretooluse",
            "precompact",
            "sessionstart",
            "userpromptsubmit",
        ];
        let mut installed = Vec::new();
        for hook_type in &hook_types {
            let script = build_hook_script(hook_type);
            let path = installer.install_hook("claude-code", hook_type, &script)?;
            installed.push(format!("Installed {} hook at {}", hook_type, path.display()));
        }
        Ok(installed)
    }

    fn uninstall(&self) -> Result<Vec<String>, AdapterError> {
        let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let installer = crate::hook_runtime::HookInstaller::new(base);
        let hook_types = [
            "posttooluse",
            "pretooluse",
            "precompact",
            "sessionstart",
            "userpromptsubmit",
        ];
        let mut removed = Vec::new();
        for hook_type in &hook_types {
            installer.uninstall_hook("claude-code", hook_type)?;
            removed.push(format!("Uninstalled {} hook", hook_type));
        }
        Ok(removed)
    }

    fn diagnostics(&self, _plugin_root: &str) -> Result<Vec<DiagnosticResult>, AdapterError> {
        Ok(Vec::new())
    }

    fn settings_path(&self) -> PathBuf {
        let mut dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        dir.push(".claude");
        dir.push("settings.json");
        dir
    }

    fn hook_paths(&self, _plugin_root: &str) -> Vec<PathBuf> {
        Vec::new()
    }

    fn platform_id(&self) -> PlatformId {
        PlatformId::ClaudeCode
    }
}

fn build_hook_script(hook_type: &str) -> String {
    if cfg!(windows) {
        format!("@echo off\ncontext-mode hook claude-code {} %*\n", hook_type)
    } else {
        format!(
            "#!/bin/sh\ncontext-mode hook claude-code {} \"{}\"\n",
            hook_type,
            "$@"
        )
    }
}

impl BaseAdapter for ClaudeCodeAdapter {
    fn session_dir_segments(&self) -> Vec<String> {
        get_session_dir_segments(PlatformId::ClaudeCode).unwrap_or_default()
    }

    fn check_plugin_registration(&self) -> DiagnosticResult {
        let config_dir = self.config_dir(None);
        let exists = config_dir.exists();
        DiagnosticResult {
            check: "claude-code config directory".to_string(),
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
                Some(
                    "Install Claude Code and run 'claude' to initialize configuration.".to_string(),
                )
            },
        }
    }
}
