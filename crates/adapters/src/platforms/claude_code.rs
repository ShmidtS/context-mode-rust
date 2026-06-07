use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use crate::base::BaseAdapter;
use crate::detect::get_session_dir_segments;
use crate::types::{AdapterError, DiagnosticResult, DiagnosticStatus, HookAdapter, PlatformId};

pub struct ClaudeCodeAdapter;

impl HookAdapter for ClaudeCodeAdapter {
    fn install(&self, plugin_root: &str) -> Result<Vec<String>, AdapterError> {
        let mut installed = Vec::new();

        // Clean up legacy file-based hooks that cause "cannot execute binary file"
        // errors in Git Bash on Windows (bash cannot execute .cmd files).
        let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let hooks_dir = base.join(".claude").join("hooks");
        let hook_types = [
            "posttooluse",
            "pretooluse",
            "precompact",
            "sessionstart",
            "userpromptsubmit",
        ];
        for hook_type in &hook_types {
            for ext in ["cmd", "sh"] {
                let path = hooks_dir.join(format!("{hook_type}.{ext}"));
                if path.exists() {
                    let _ = fs::remove_file(&path);
                    installed.push(format!("Removed legacy hook file {}", path.display()));
                }
            }
        }

        // Install settings.json hooks (declarative hooks are the only supported mechanism)
        match self.install_settings_hooks(plugin_root) {
            Ok(mut settings_results) => installed.append(&mut settings_results),
            Err(e) => installed.push(format!("Settings hooks install failed: {}", e)),
        }

        // Install slash commands in settings.json
        match self.install_slash_commands(plugin_root) {
            Ok(mut slash_results) => installed.append(&mut slash_results),
            Err(e) => installed.push(format!("Slash commands install failed: {}", e)),
        }

        Ok(installed)
    }

    fn uninstall(&self) -> Result<Vec<String>, AdapterError> {
        let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let hooks_dir = base.join(".claude").join("hooks");
        let hook_types = [
            "posttooluse",
            "pretooluse",
            "precompact",
            "sessionstart",
            "userpromptsubmit",
        ];
        let mut removed = Vec::new();
        for hook_type in &hook_types {
            for ext in ["cmd", "sh"] {
                let path = hooks_dir.join(format!("{hook_type}.{ext}"));
                if path.exists() {
                    fs::remove_file(&path)?;
                    removed.push(format!("Removed legacy hook {}", path.display()));
                }
            }
        }
        Ok(removed)
    }

    fn diagnostics(&self, plugin_root: &str) -> Result<Vec<DiagnosticResult>, AdapterError> {
        let mut results = Vec::new();
        let settings = self.read_settings()?;
        let settings_path = self.settings_path();

        // Check if settings.json exists
        results.push(DiagnosticResult {
            check: "settings.json exists".to_string(),
            status: if settings.is_some() {
                DiagnosticStatus::Pass
            } else {
                DiagnosticStatus::Fail
            },
            message: if settings.is_some() {
                format!("Settings file found at {}", settings_path.display())
            } else {
                format!("Settings file not found at {}", settings_path.display())
            },
            fix: if settings.is_some() {
                None
            } else {
                Some(format!(
                    "Run 'context-mode setup' to create {}",
                    settings_path.display()
                ))
            },
        });

        // Check if hooks are configured
        if let Some(settings) = &settings {
            let hooks_present = settings.get("hooks").is_some();
            results.push(DiagnosticResult {
                check: "hooks configured in settings.json".to_string(),
                status: if hooks_present {
                    DiagnosticStatus::Pass
                } else {
                    DiagnosticStatus::Warn
                },
                message: if hooks_present {
                    "Hooks configured in settings.json".to_string()
                } else {
                    "No hooks configured in settings.json".to_string()
                },
                fix: if hooks_present {
                    None
                } else {
                    Some("Run 'context-mode setup' to install hooks".to_string())
                },
            });
        }

        // Check if context-mode binary is accessible
        let bin_name = if cfg!(windows) {
            "context-mode.exe"
        } else {
            "context-mode"
        };
        let bin_path = PathBuf::from(plugin_root)
            .join(".claude-plugin")
            .join("bin")
            .join(bin_name);
        results.push(DiagnosticResult {
            check: "context-mode binary accessible".to_string(),
            status: if bin_path.exists() {
                DiagnosticStatus::Pass
            } else {
                DiagnosticStatus::Warn
            },
            message: if bin_path.exists() {
                format!("Binary found at {}", bin_path.display())
            } else {
                format!("Binary not found at {}", bin_path.display())
            },
            fix: if bin_path.exists() {
                None
            } else {
                Some("Build the project with 'cargo build --release'".to_string())
            },
        });

        // Check if context-mode-server binary is accessible
        let server_name = if cfg!(windows) {
            "context-mode-server.exe"
        } else {
            "context-mode-server"
        };
        let server_path = PathBuf::from(plugin_root)
            .join(".claude-plugin")
            .join("bin")
            .join(server_name);
        results.push(DiagnosticResult {
            check: "context-mode-server binary accessible".to_string(),
            status: if server_path.exists() {
                DiagnosticStatus::Pass
            } else {
                DiagnosticStatus::Warn
            },
            message: if server_path.exists() {
                format!("Server binary found at {}", server_path.display())
            } else {
                format!("Server binary not found at {}", server_path.display())
            },
            fix: if server_path.exists() {
                None
            } else {
                Some("Build the project with 'cargo build --release'".to_string())
            },
        });

        // Check if slash commands are registered
        if let Some(settings) = &settings {
            let commands = settings.get("customCommands");
            let slash_present = commands.is_some();
            results.push(DiagnosticResult {
                check: "slash commands registered".to_string(),
                status: if slash_present {
                    DiagnosticStatus::Pass
                } else {
                    DiagnosticStatus::Warn
                },
                message: if slash_present {
                    "Slash commands registered in settings.json".to_string()
                } else {
                    "No slash commands registered in settings.json".to_string()
                },
                fix: if slash_present {
                    None
                } else {
                    Some("Run 'context-mode setup' to register slash commands".to_string())
                },
            });
        }

        Ok(results)
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
/// the absolute path to `context-mode.exe` (Windows) or `context-mode` (Unix)
/// so the shell can execute it directly.
fn rewrite_hook_commands(value: &mut Value, plugin_root: &str) {
    let bin_path = PathBuf::from(plugin_root)
        .join(".claude-plugin")
        .join("bin")
        .join(if cfg!(windows) {
            "context-mode.exe"
        } else {
            "context-mode"
        });
    // Use forward slashes on Windows so bash does not interpret backslashes
    // as escape sequences inside double-quoted strings.
    let bin_path_str = if cfg!(windows) {
        bin_path.to_string_lossy().replace('\\', "/")
    } else {
        bin_path.to_string_lossy().to_string()
    };
    let replacement = format!(r#""{}" hook"#, bin_path_str);

    match value {
        Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if key == "command" {
                    if let Value::String(s) = val {
                        *s = s.replace("context-mode hook", &replacement);
                    }
                } else {
                    rewrite_hook_commands(val, plugin_root);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                rewrite_hook_commands(item, plugin_root);
            }
        }
        _ => {}
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

        let mut settings = self
            .read_settings()?
            .unwrap_or_else(|| serde_json::json!({}));

        // On Windows, rewrite "context-mode hook" -> absolute path to context-mode.exe
        // so shells (including Git Bash) can execute the binary directly.
        let mut hooks = hooks.clone();
        if cfg!(windows) {
            rewrite_hook_commands(&mut hooks, plugin_root);
        }

        // Skip write if hooks are already identical
        if let Some(current_hooks) = settings.get("hooks") {
            if current_hooks == &hooks {
                return Ok(vec![format!(
                    "Hooks already up to date in {}",
                    self.settings_path().display()
                )]);
            }
        }

        // Backup existing settings before mutating
        let _ = self.backup_settings();

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

    fn install_slash_commands(&self, plugin_root: &str) -> Result<Vec<String>, AdapterError> {
        let mut settings = self
            .read_settings()?
            .unwrap_or_else(|| serde_json::json!({}));

        let bin_path = PathBuf::from(plugin_root)
            .join(".claude-plugin")
            .join("bin")
            .join(if cfg!(windows) {
                "context-mode.exe"
            } else {
                "context-mode"
            });
        let bin_path_str = if cfg!(windows) {
            bin_path.to_string_lossy().replace('\\', "/")
        } else {
            bin_path.to_string_lossy().to_string()
        };

        let commands = serde_json::json!({
            "ctx-stats": {
                "type": "command",
                "command": format!(r#"{}"#, bin_path_str),
                "args": ["stats"]
            },
            "ctx-doctor": {
                "type": "command",
                "command": format!(r#"{}"#, bin_path_str),
                "args": ["doctor"]
            },
            "ctx-upgrade": {
                "type": "command",
                "command": format!(r#"{}"#, bin_path_str),
                "args": ["upgrade"]
            },
            "ctx-purge": {
                "type": "command",
                "command": format!(r#"{}"#, bin_path_str),
                "args": ["purge", "--confirm"]
            },
            "ctx-search": {
                "type": "command",
                "command": format!(r#"{}"#, bin_path_str),
                "args": ["search"]
            },
            "ctx-insight": {
                "type": "command",
                "command": format!(r#"{}"#, bin_path_str),
                "args": ["insight"]
            }
        });

        // Check if customCommands already matches
        let mut existing = settings
            .get("customCommands")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let existing_obj = existing.as_object_mut().unwrap_or_else(|| {
            settings
                .as_object_mut()
                .unwrap()
                .insert("customCommands".to_string(), serde_json::json!({}));
            settings["customCommands"].as_object_mut().unwrap()
        });
        let commands_obj = commands.as_object().unwrap();

        let mut changed = false;
        for (key, value) in commands_obj {
            if existing_obj.get(key) != Some(value) {
                existing_obj.insert(key.clone(), value.clone());
                changed = true;
            }
        }

        if !changed {
            return Ok(vec![format!(
                "Slash commands already up to date in {}",
                self.settings_path().display()
            )]);
        }

        // Backup existing settings before mutating
        let _ = self.backup_settings();

        if let Some(obj) = settings.as_object_mut() {
            obj.insert("customCommands".to_string(), existing);
        }

        self.write_settings(&settings)?;

        Ok(vec![format!(
            "Installed context-mode slash commands in {}",
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
