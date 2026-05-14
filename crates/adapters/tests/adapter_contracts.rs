use std::collections::HashMap;
use std::path::PathBuf;

use context_mode_adapters::client_map::client_name_to_platform;
use context_mode_adapters::detect::{detect_platform_from_signals, get_session_dir_segments};
use context_mode_adapters::hooks_helpers::{
    create_build_hook_command, create_is_context_mode_hook,
};
use context_mode_adapters::shared::{normalize_session_source, upsert_hook_entry};
use context_mode_adapters::types::{HookCommand, HookEntry, PlatformId, SessionStartSource};

#[test]
fn client_map_resolves_known_mcp_client_names() {
    let map = client_name_to_platform();

    assert_eq!(map.get("claude-code"), Some(&PlatformId::ClaudeCode));
    assert_eq!(
        map.get("gemini-cli-mcp-client"),
        Some(&PlatformId::GeminiCli)
    );
    assert_eq!(map.get("qwen-cli-mcp-client"), Some(&PlatformId::QwenCode));
}

#[test]
fn detection_prefers_env_vars_over_config_paths_and_process_names() {
    let signal = context_mode_adapters::types::DetectionSignal {
        env_vars: HashMap::from([("QWEN_PROJECT_DIR".to_string(), "/tmp/project".to_string())]),
        config_paths: vec![PathBuf::from(".claude")],
        process_name: Some("Code".to_string()),
    };

    let result = detect_platform_from_signals(&signal);

    assert_eq!(result.platform, Some(PlatformId::QwenCode));
    assert_eq!(result.confidence, 1.0);
}

#[test]
fn detection_uses_config_paths_when_env_vars_absent() {
    let signal = context_mode_adapters::types::DetectionSignal {
        env_vars: HashMap::new(),
        config_paths: vec![PathBuf::from("/home/alice/.config/opencode")],
        process_name: None,
    };

    let result = detect_platform_from_signals(&signal);

    assert_eq!(result.platform, Some(PlatformId::OpenCode));
    assert!(result.confidence < 1.0);
}

#[test]
fn session_dir_segments_match_supported_platforms() {
    assert_eq!(
        get_session_dir_segments(PlatformId::ClaudeCode),
        Some(vec![".claude".to_string()])
    );
    assert_eq!(
        get_session_dir_segments(PlatformId::OpenCode),
        Some(vec![".config".to_string(), "opencode".to_string()])
    );
    assert_eq!(get_session_dir_segments(PlatformId::Unknown), None);
}

#[test]
fn hook_command_builder_prefers_plugin_script_path() {
    let builder = create_build_hook_command(
        HashMap::from([("PreToolUse".to_string(), "pre-tool-use.js".to_string())]),
        PlatformId::ClaudeCode,
        None,
        false,
    );

    let command = builder("PreToolUse", Some("C:\\plugins\\context-mode")).unwrap();

    assert!(command.contains("context-mode hook"));
    assert!(command.contains("claude-code pretooluse"));
}

#[test]
fn context_mode_hook_checker_matches_dispatcher_command() {
    let checker = create_is_context_mode_hook(HashMap::new(), |hook_type| {
        format!("context-mode hook claude-code {}", hook_type.to_lowercase())
    });
    let entry = HookEntry {
        matcher: String::new(),
        hooks: vec![HookCommand {
            hook_type: "command".to_string(),
            command: "context-mode hook claude-code pretooluse".to_string(),
        }],
    };

    assert!(checker(&entry, "PreToolUse"));
}

#[test]
fn shared_helpers_normalize_source_and_upsert_hooks() {
    assert_eq!(
        normalize_session_source(Some("resume")),
        SessionStartSource::Resume
    );
    assert_eq!(
        normalize_session_source(Some("bogus")),
        SessionStartSource::Startup
    );

    let mut hooks = HashMap::new();
    let mut changes = Vec::new();
    let first = HookEntry {
        matcher: String::new(),
        hooks: vec![HookCommand {
            hook_type: "command".to_string(),
            command: "old pre-tool-use.js".to_string(),
        }],
    };
    let replacement = HookEntry {
        matcher: "Bash".to_string(),
        hooks: vec![HookCommand {
            hook_type: "command".to_string(),
            command: "new pre-tool-use.js".to_string(),
        }],
    };

    upsert_hook_entry(&mut hooks, "PreToolUse", first, &mut changes, |_| false);
    upsert_hook_entry(
        &mut hooks,
        "PreToolUse",
        replacement,
        &mut changes,
        |entry| {
            entry
                .hooks
                .iter()
                .any(|hook| hook.command.contains("pre-tool-use.js"))
        },
    );

    assert_eq!(hooks["PreToolUse"].len(), 1);
    assert_eq!(hooks["PreToolUse"][0].matcher, "Bash");
    assert_eq!(
        changes,
        vec![
            "Added PreToolUse hook entry",
            "Updated existing PreToolUse hook entry"
        ]
    );
}
