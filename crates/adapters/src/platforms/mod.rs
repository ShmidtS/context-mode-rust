pub mod claude_code;

use std::path::PathBuf;

use crate::base::BaseAdapter;
use crate::detect::get_session_dir_segments;
use crate::types::{AdapterError, DiagnosticResult, DiagnosticStatus, HookAdapter, PlatformId};

/// Generate a zero-boilerplate platform adapter.
///
/// All 10 simple adapters share the same `HookAdapter` + `BaseAdapter` logic;
/// they differ only in struct name, `PlatformId` variant, settings-path segments,
/// and the diagnostic check / fix strings.
macro_rules! simple_adapter {
    (
        $(#[$meta:meta])*
        struct $name:ident;
        platform_id = $pid:ident;
        settings_path = { $($push:literal),* $(,)? };
        check_name = $check:literal;
        fix = $fix:literal;
    ) => {
        $(#[$meta])*
        pub struct $name;

        impl HookAdapter for $name {
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
                let mut dir = context_mode_utils::paths::home_or_current();
                $( dir.push($push); )*
                dir
            }

            fn hook_paths(&self, _plugin_root: &str) -> Vec<PathBuf> {
                Vec::new()
            }

            fn platform_id(&self) -> PlatformId {
                PlatformId::$pid
            }
        }

        impl BaseAdapter for $name {
            fn session_dir_segments(&self) -> Vec<String> {
                get_session_dir_segments(PlatformId::$pid).unwrap_or_default()
            }

            fn check_plugin_registration(&self) -> DiagnosticResult {
                let config_dir = self.config_dir(None);
                let exists = config_dir.exists();
                DiagnosticResult {
                    check: $check.to_string(),
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
                        Some($fix.to_string())
                    },
                }
            }
        }
    };
}

simple_adapter! {
    struct CodexAdapter;
    platform_id = Codex;
    settings_path = { ".codex", "settings.json" };
    check_name = "codex config directory";
    fix = "Install Codex CLI and run 'codex' to initialize configuration.";
}

simple_adapter! {
    struct CursorAdapter;
    platform_id = Cursor;
    settings_path = { ".cursor", "settings.json" };
    check_name = "cursor config directory";
    fix = "Install Cursor and open it to initialize configuration.";
}

simple_adapter! {
    struct GeminiCliAdapter;
    platform_id = GeminiCli;
    settings_path = { ".gemini", "settings.json" };
    check_name = "gemini-cli config directory";
    fix = "Install Gemini CLI and run 'gemini' to initialize configuration.";
}

simple_adapter! {
    struct JetbrainsCopilotAdapter;
    platform_id = JetbrainsCopilot;
    settings_path = { ".config", "JetBrains", "settings.json" };
    check_name = "jetbrains-copilot config directory";
    fix = "Install JetBrains IDE with GitHub Copilot plugin to initialize configuration.";
}

simple_adapter! {
    struct KiroAdapter;
    platform_id = Kiro;
    settings_path = { ".kiro", "settings.json" };
    check_name = "kiro config directory";
    fix = "Install Kiro and run it to initialize configuration.";
}

simple_adapter! {
    struct OpenClawAdapter;
    platform_id = OpenClaw;
    settings_path = { ".openclaw", "settings.json" };
    check_name = "openclaw config directory";
    fix = "Install OpenClaw and run it to initialize configuration.";
}

simple_adapter! {
    struct OpenCodeAdapter;
    platform_id = OpenCode;
    settings_path = { ".config", "opencode", "settings.json" };
    check_name = "opencode config directory";
    fix = "Install OpenCode and run it to initialize configuration.";
}

simple_adapter! {
    struct QwenCodeAdapter;
    platform_id = QwenCode;
    settings_path = { ".qwen", "settings.json" };
    check_name = "qwen-code config directory";
    fix = "Install Qwen Code and run it to initialize configuration.";
}

simple_adapter! {
    struct VscodeCopilotAdapter;
    platform_id = VscodeCopilot;
    settings_path = { ".vscode", "settings.json" };
    check_name = "vscode-copilot config directory";
    fix = "Install VS Code with GitHub Copilot extension to initialize configuration.";
}

simple_adapter! {
    struct ZedAdapter;
    platform_id = Zed;
    settings_path = { ".config", "zed", "settings.json" };
    check_name = "zed config directory";
    fix = "Install Zed editor and open it to initialize configuration.";
}
