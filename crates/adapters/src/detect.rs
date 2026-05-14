use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use crate::client_map::client_name_to_platform;
use crate::types::{AdapterError, DetectionSignal, PlatformDetection, PlatformId};

const PLATFORM_ENV_VARS: &[(PlatformId, &[&str])] = &[
    (
        PlatformId::ClaudeCode,
        &["CLAUDE_PROJECT_DIR", "CLAUDE_SESSION_ID"],
    ),
    (PlatformId::Cursor, &["CURSOR_TRACE_ID", "CURSOR_CLI"]),
    (PlatformId::OpenCode, &["OPENCODE", "OPENCODE_PID"]),
    (PlatformId::Codex, &["CODEX_THREAD_ID", "CODEX_CI"]),
    (PlatformId::GeminiCli, &["GEMINI_PROJECT_DIR", "GEMINI_CLI"]),
    (PlatformId::VscodeCopilot, &["VSCODE_PID", "VSCODE_CWD"]),
    (PlatformId::JetbrainsCopilot, &["IDEA_INITIAL_DIRECTORY"]),
    (PlatformId::QwenCode, &["QWEN_PROJECT_DIR"]),
];

const PLATFORM_CONFIG_PATHS: &[(PlatformId, &[&str])] = &[
    (PlatformId::ClaudeCode, &[".claude"]),
    (PlatformId::GeminiCli, &[".gemini"]),
    (PlatformId::Codex, &[".codex"]),
    (PlatformId::Cursor, &[".cursor"]),
    (PlatformId::Kiro, &[".kiro"]),
    (PlatformId::QwenCode, &[".qwen"]),
    (PlatformId::OpenClaw, &[".openclaw"]),
    (PlatformId::OpenCode, &[".config", "opencode"]),
    (PlatformId::Zed, &[".config", "zed"]),
    (PlatformId::JetbrainsCopilot, &[".config", "JetBrains"]),
    (PlatformId::VscodeCopilot, &[".vscode"]),
];

pub fn detect_platform() -> PlatformDetection {
    let env_vars = env::vars().collect::<HashMap<_, _>>();
    let config_paths = dirs::home_dir()
        .map(existing_config_paths)
        .unwrap_or_default();
    let process_name = env::args().next();

    detect_platform_from_signals(&DetectionSignal {
        env_vars,
        config_paths,
        process_name,
    })
}

pub fn detect_platform_from_client(client_name: &str) -> PlatformDetection {
    let map = client_name_to_platform();
    if let Some(platform) = map.get(client_name).copied() {
        return PlatformDetection {
            platform: Some(platform),
            confidence: 1.0,
            reason: format!("MCP clientInfo.name=\"{client_name}\""),
        };
    }

    if client_name.starts_with("qwen-cli-mcp-client") {
        return PlatformDetection {
            platform: Some(PlatformId::QwenCode),
            confidence: 1.0,
            reason: format!("MCP clientInfo.name=\"{client_name}\" qwen pattern"),
        };
    }

    PlatformDetection {
        platform: None,
        confidence: 0.0,
        reason: "unknown MCP clientInfo.name".to_string(),
    }
}

pub fn detect_platform_from_signals(signal: &DetectionSignal) -> PlatformDetection {
    if let Some(platform_override) = signal.env_vars.get("CONTEXT_MODE_PLATFORM") {
        if let Some(platform) = parse_platform_id(platform_override) {
            return PlatformDetection {
                platform: Some(platform),
                confidence: 1.0,
                reason: format!("CONTEXT_MODE_PLATFORM={platform_override} override"),
            };
        }
    }

    for (platform, vars) in PLATFORM_ENV_VARS {
        if vars.iter().any(|var| signal.env_vars.contains_key(*var)) {
            return PlatformDetection {
                platform: Some(*platform),
                confidence: 1.0,
                reason: format!("{} env var set", vars.join(" or ")),
            };
        }
    }

    for path in &signal.config_paths {
        if let Some(platform) = platform_from_config_path(path) {
            return PlatformDetection {
                platform: Some(platform),
                confidence: 0.7,
                reason: format!("config path exists: {}", path.display()),
            };
        }
    }

    if let Some(process_name) = signal.process_name.as_deref() {
        if let Some(platform) = platform_from_process_name(process_name) {
            return PlatformDetection {
                platform: Some(platform),
                confidence: 0.5,
                reason: format!("process name matched: {process_name}"),
            };
        }
    }

    PlatformDetection {
        platform: None,
        confidence: 0.0,
        reason: "No platform detected".to_string(),
    }
}

pub fn get_adapter(platform: Option<PlatformId>) -> Result<PlatformId, AdapterError> {
    let target = platform
        .or_else(|| detect_platform().platform)
        .unwrap_or(PlatformId::Unknown);
    match target {
        PlatformId::Unknown => Err(AdapterError::UnsupportedPlatform(target)),
        platform => Ok(platform),
    }
}

pub fn get_session_dir_segments(platform: PlatformId) -> Option<Vec<String>> {
    let segments = match platform {
        PlatformId::ClaudeCode => &[".claude"][..],
        PlatformId::GeminiCli => &[".gemini"],
        PlatformId::OpenClaw => &[".openclaw"],
        PlatformId::Codex => &[".codex"],
        PlatformId::Cursor => &[".cursor"],
        PlatformId::VscodeCopilot => &[".vscode"],
        PlatformId::Kiro => &[".kiro"],
        PlatformId::QwenCode => &[".qwen"],
        PlatformId::OpenCode => &[".config", "opencode"],
        PlatformId::Zed => &[".config", "zed"],
        PlatformId::JetbrainsCopilot => &[".config", "JetBrains"],
        PlatformId::Unknown => return None,
    };
    Some(
        segments
            .iter()
            .map(|segment| (*segment).to_string())
            .collect(),
    )
}

fn existing_config_paths(home: PathBuf) -> Vec<PathBuf> {
    PLATFORM_CONFIG_PATHS
        .iter()
        .map(|(_, segments)| {
            segments
                .iter()
                .fold(home.clone(), |path, segment| path.join(segment))
        })
        .filter(|path| path.exists())
        .collect()
}

fn platform_from_config_path(path: &Path) -> Option<PlatformId> {
    let normalized = path.to_string_lossy().replace('\\', "/").to_lowercase();
    PLATFORM_CONFIG_PATHS
        .iter()
        .find_map(|(platform, segments)| {
            let suffix = segments.join("/").to_lowercase();
            normalized.ends_with(&suffix).then_some(*platform)
        })
}

fn platform_from_process_name(process_name: &str) -> Option<PlatformId> {
    let normalized = process_name.to_lowercase();
    if normalized.contains("cursor") {
        Some(PlatformId::Cursor)
    } else if normalized.contains("code") || normalized.contains("vscode") {
        Some(PlatformId::VscodeCopilot)
    } else if normalized.contains("idea") || normalized.contains("jetbrains") {
        Some(PlatformId::JetbrainsCopilot)
    } else if normalized.contains("gemini") {
        Some(PlatformId::GeminiCli)
    } else if normalized.contains("codex") {
        Some(PlatformId::Codex)
    } else if normalized.contains("opencode") {
        Some(PlatformId::OpenCode)
    } else if normalized.contains("qwen") {
        Some(PlatformId::QwenCode)
    } else if normalized.contains("zed") {
        Some(PlatformId::Zed)
    } else {
        None
    }
}

fn parse_platform_id(value: &str) -> Option<PlatformId> {
    match value {
        "claude-code" => Some(PlatformId::ClaudeCode),
        "codex" => Some(PlatformId::Codex),
        "cursor" => Some(PlatformId::Cursor),
        "gemini-cli" => Some(PlatformId::GeminiCli),
        "jetbrains-copilot" => Some(PlatformId::JetbrainsCopilot),
        "kiro" => Some(PlatformId::Kiro),
        "openclaw" => Some(PlatformId::OpenClaw),
        "opencode" => Some(PlatformId::OpenCode),
        "qwen-code" => Some(PlatformId::QwenCode),
        "vscode-copilot" => Some(PlatformId::VscodeCopilot),
        "zed" => Some(PlatformId::Zed),
        _ => None,
    }
}
