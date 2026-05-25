use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use crate::base::BaseAdapter;
use crate::detect::get_session_dir_segments;
use crate::types::{AdapterError, DiagnosticResult, DiagnosticStatus, HookAdapter, PlatformId};

pub struct ClaudeCodeAdapter;

impl HookAdapter for ClaudeCodeAdapter {
    fn install(&self, plugin_root: &str) -> Result<Vec<String>, AdapterError> {
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
            installed.push(format!(
                "Installed {} hook script at {}",
                hook_type,
                path.display()
            ));
        }

        // Install settings.json hooks
        match self.install_settings_hooks(plugin_root) {
            Ok(mut settings_results) => installed.append(&mut settings_results),
            Err(e) => installed.push(format!("Settings hooks install failed: {}", e)),
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

/// Recursively walk a JSON value and replace `"context-mode hook"` with
/// `"context-mode.cmd hook"` in every `"command"` string field (Windows only).
fn rewrite_hook_commands(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if key == "command" {
                    if let Value::String(s) = val {
                        *s = s.replace("context-mode hook", "context-mode.cmd hook");
                    }
                } else {
                    rewrite_hook_commands(val);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                rewrite_hook_commands(item);
            }
        }
        _ => {}
    }
}

fn build_hook_script(hook_type: &str) -> String {
    if cfg!(windows) {
        format!(
            "@echo off\ncontext-mode.cmd hook claude-code {} %*\n",
            hook_type
        )
    } else {
        format!(
            "#!/bin/sh\ncontext-mode hook claude-code {} \"{}\"\n",
            hook_type, "$@"
        )
    }
}

impl ClaudeCodeAdapter {
    fn install_settings_hooks(&self, plugin_root: &str) -> Result<Vec<String>, AdapterError> {
        let hooks_path = PathBuf::from(plugin_root).join("hooks").join("hooks.json");
        if !hooks_path.exists() {
            return Ok(vec![format!(
                "hooks.json not found at {}, skipping settings hooks",
                hooks_path.display()
            )]);
        }

        let hooks_raw = fs::read_to_string(&hooks_path)?;
        let hooks_value: Value = serde_json::from_str(&hooks_raw)?;
        let hooks = hooks_value.get("hooks").ok_or_else(|| {
            AdapterError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "hooks.json missing 'hooks' field",
            ))
        })?;

        // Backup existing settings
        let _ = self.backup_settings();

        let mut settings = self.read_settings()?.unwrap_or_else(|| {
            serde_json::json!({})
        });

        // On Windows, rewrite "context-mode hook" -> "context-mode.cmd hook" in all
        // command strings so shells resolve the .cmd wrapper, not the extension-less bash script.
        let mut hooks = hooks.clone();
        if cfg!(windows) {
            rewrite_hook_commands(&mut hooks);
        }

        // Insert or replace hooks in settings
        if let Some(obj) = settings.as_object_mut() {
            obj.insert("hooks".to_string(), hooks);
        }

        self.write_settings(&settings)?;

        Ok(vec![format!(
            "Installed context-mode hooks in {}",
            self.settings_path().display()
        )])
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
