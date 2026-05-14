use context_mode_adapters::base::BaseAdapter;
use context_mode_adapters::detect::get_session_dir_segments;
use context_mode_adapters::platforms::*;
use context_mode_adapters::types::{HookAdapter, PlatformId};

#[test]
fn claude_code_platform_id_and_segments() {
    let adapter = claude_code::ClaudeCodeAdapter;
    assert_eq!(adapter.platform_id(), PlatformId::ClaudeCode);
    assert_eq!(
        adapter.session_dir_segments(),
        get_session_dir_segments(PlatformId::ClaudeCode).unwrap()
    );
    assert!(adapter.hook_paths("").is_empty());
}

#[test]
fn codex_platform_id_and_segments() {
    let adapter = codex::CodexAdapter;
    assert_eq!(adapter.platform_id(), PlatformId::Codex);
    assert_eq!(
        adapter.session_dir_segments(),
        get_session_dir_segments(PlatformId::Codex).unwrap()
    );
    assert!(adapter.hook_paths("").is_empty());
}

#[test]
fn cursor_platform_id_and_segments() {
    let adapter = cursor::CursorAdapter;
    assert_eq!(adapter.platform_id(), PlatformId::Cursor);
    assert_eq!(
        adapter.session_dir_segments(),
        get_session_dir_segments(PlatformId::Cursor).unwrap()
    );
    assert!(adapter.hook_paths("").is_empty());
}

#[test]
fn gemini_cli_platform_id_and_segments() {
    let adapter = gemini_cli::GeminiCliAdapter;
    assert_eq!(adapter.platform_id(), PlatformId::GeminiCli);
    assert_eq!(
        adapter.session_dir_segments(),
        get_session_dir_segments(PlatformId::GeminiCli).unwrap()
    );
    assert!(adapter.hook_paths("").is_empty());
}

#[test]
fn jetbrains_copilot_platform_id_and_segments() {
    let adapter = jetbrains_copilot::JetbrainsCopilotAdapter;
    assert_eq!(adapter.platform_id(), PlatformId::JetbrainsCopilot);
    assert_eq!(
        adapter.session_dir_segments(),
        get_session_dir_segments(PlatformId::JetbrainsCopilot).unwrap()
    );
    assert!(adapter.hook_paths("").is_empty());
}

#[test]
fn kiro_platform_id_and_segments() {
    let adapter = kiro::KiroAdapter;
    assert_eq!(adapter.platform_id(), PlatformId::Kiro);
    assert_eq!(
        adapter.session_dir_segments(),
        get_session_dir_segments(PlatformId::Kiro).unwrap()
    );
    assert!(adapter.hook_paths("").is_empty());
}

#[test]
fn openclaw_platform_id_and_segments() {
    let adapter = openclaw::OpenClawAdapter;
    assert_eq!(adapter.platform_id(), PlatformId::OpenClaw);
    assert_eq!(
        adapter.session_dir_segments(),
        get_session_dir_segments(PlatformId::OpenClaw).unwrap()
    );
    assert!(adapter.hook_paths("").is_empty());
}

#[test]
fn opencode_platform_id_and_segments() {
    let adapter = opencode::OpenCodeAdapter;
    assert_eq!(adapter.platform_id(), PlatformId::OpenCode);
    assert_eq!(
        adapter.session_dir_segments(),
        get_session_dir_segments(PlatformId::OpenCode).unwrap()
    );
    assert!(adapter.hook_paths("").is_empty());
}

#[test]
fn qwen_code_platform_id_and_segments() {
    let adapter = qwen_code::QwenCodeAdapter;
    assert_eq!(adapter.platform_id(), PlatformId::QwenCode);
    assert_eq!(
        adapter.session_dir_segments(),
        get_session_dir_segments(PlatformId::QwenCode).unwrap()
    );
    assert!(adapter.hook_paths("").is_empty());
}

#[test]
fn vscode_copilot_platform_id_and_segments() {
    let adapter = vscode_copilot::VscodeCopilotAdapter;
    assert_eq!(adapter.platform_id(), PlatformId::VscodeCopilot);
    assert_eq!(
        adapter.session_dir_segments(),
        get_session_dir_segments(PlatformId::VscodeCopilot).unwrap()
    );
    assert!(adapter.hook_paths("").is_empty());
}

#[test]
fn zed_platform_id_and_segments() {
    let adapter = zed::ZedAdapter;
    assert_eq!(adapter.platform_id(), PlatformId::Zed);
    assert_eq!(
        adapter.session_dir_segments(),
        get_session_dir_segments(PlatformId::Zed).unwrap()
    );
    assert!(adapter.hook_paths("").is_empty());
}
