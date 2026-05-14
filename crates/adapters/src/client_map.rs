use std::collections::HashMap;

use crate::types::PlatformId;

pub fn client_name_to_platform() -> HashMap<&'static str, PlatformId> {
    HashMap::from([
        ("claude-code", PlatformId::ClaudeCode),
        ("gemini-cli-mcp-client", PlatformId::GeminiCli),
        ("cursor-vscode", PlatformId::Cursor),
        ("Visual-Studio-Code", PlatformId::VscodeCopilot),
        ("JetBrains Client", PlatformId::JetbrainsCopilot),
        ("IntelliJ IDEA", PlatformId::JetbrainsCopilot),
        ("PyCharm", PlatformId::JetbrainsCopilot),
        ("Codex", PlatformId::Codex),
        ("codex-mcp-client", PlatformId::Codex),
        ("Kiro CLI", PlatformId::Kiro),
        ("Zed", PlatformId::Zed),
        ("zed", PlatformId::Zed),
        ("qwen-code", PlatformId::QwenCode),
        ("qwen-cli-mcp-client", PlatformId::QwenCode),
    ])
}
