use std::collections::HashMap;

use crate::types::{AdapterError, HookEntry, PlatformId, build_node_command};

pub fn create_is_context_mode_hook<F>(
    hook_scripts: HashMap<String, String>,
    get_dispatcher_command: F,
) -> impl Fn(&HookEntry, &str) -> bool
where
    F: Fn(&str) -> String,
{
    move |entry, hook_type| {
        let Some(script_name) = hook_scripts.get(hook_type) else {
            return false;
        };
        let cli_command = get_dispatcher_command(hook_type);
        entry
            .hooks
            .iter()
            .any(|hook| hook.command.contains(script_name) || hook.command.contains(&cli_command))
    }
}

pub fn create_build_hook_command(
    hook_scripts: HashMap<String, String>,
    platform_id: PlatformId,
    hooks_sub_dir: Option<String>,
    throw_on_missing_script: bool,
) -> impl Fn(&str, Option<&str>) -> Result<String, AdapterError> {
    move |hook_type, plugin_root| {
        let script_name = hook_scripts.get(hook_type);

        if throw_on_missing_script && script_name.is_none() {
            return Err(AdapterError::MissingHookScript(hook_type.to_string()));
        }

        if let (Some(plugin_root), Some(script_name)) = (plugin_root, script_name) {
            let script_path = match hooks_sub_dir.as_deref() {
                Some(sub_dir) => format!("{plugin_root}/hooks/{sub_dir}/{script_name}"),
                None => format!("{plugin_root}/hooks/{script_name}"),
            };
            return Ok(build_node_command(script_path));
        }

        Ok(format!(
            "context-mode hook {} {}",
            platform_id,
            hook_type.to_lowercase()
        ))
    }
}
