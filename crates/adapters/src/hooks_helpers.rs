use std::collections::HashMap;

use crate::types::{AdapterError, HookEntry, PlatformId};

pub fn create_is_context_mode_hook<F>(
    _hook_scripts: HashMap<String, String>,
    get_dispatcher_command: F,
) -> impl Fn(&HookEntry, &str) -> bool
where
    F: Fn(&str) -> String,
{
    move |entry, hook_type| {
        let cli_command = get_dispatcher_command(hook_type);
        entry
            .hooks
            .iter()
            .any(|hook| hook.command.contains(&cli_command))
    }
}

pub fn create_build_hook_command(
    _hook_scripts: HashMap<String, String>,
    platform_id: PlatformId,
    _hooks_sub_dir: Option<String>,
    _throw_on_missing_script: bool,
) -> impl Fn(&str, Option<&str>) -> Result<String, AdapterError> {
    move |hook_type, _plugin_root| {
        Ok(format!(
            "context-mode hook {} {}",
            platform_id,
            hook_type.to_lowercase()
        ))
    }
}
