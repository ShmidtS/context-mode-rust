use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::types::AdapterError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookScript {
    pub platform: String,
    pub hook_type: String,
    pub path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct HookInstaller {
    base_config_dir: PathBuf,
}

impl HookInstaller {
    pub fn new(base_config_dir: PathBuf) -> Self {
        Self { base_config_dir }
    }

    pub fn install_hook(
        &self,
        platform: &str,
        hook_type: &str,
        script_content: &str,
    ) -> Result<PathBuf, AdapterError> {
        let path = self.hook_path(platform, hook_type)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, script_content)?;
        make_executable(&path)?;
        Ok(path)
    }

    pub fn uninstall_hook(&self, platform: &str, hook_type: &str) -> Result<(), AdapterError> {
        let path = self.hook_path(platform, hook_type)?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn list_hooks(&self) -> Result<Vec<HookScript>, AdapterError> {
        let mut hooks = Vec::new();
        for platform in supported_platforms() {
            let dir = self.hook_dir(platform)?;
            if !dir.exists() {
                continue;
            }

            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if !path.is_file()
                    || path.extension().and_then(|ext| ext.to_str()) != Some(script_extension())
                {
                    continue;
                }

                if let Some(hook_type) = path.file_stem().and_then(|stem| stem.to_str()) {
                    hooks.push(HookScript {
                        platform: platform.to_string(),
                        hook_type: hook_type.to_string(),
                        content: fs::read_to_string(&path)?,
                        path,
                    });
                }
            }
        }
        Ok(hooks)
    }

    pub fn is_hook_installed(&self, platform: &str, hook_type: &str) -> bool {
        self.hook_path(platform, hook_type)
            .map(|path| path.exists())
            .unwrap_or(false)
    }

    fn default() -> Result<Self, AdapterError> {
        Ok(Self::new(
            dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")),
        ))
    }

    fn hook_path(&self, platform: &str, hook_type: &str) -> Result<PathBuf, AdapterError> {
        Ok(self
            .hook_dir(platform)?
            .join(format!("{hook_type}.{}", script_extension())))
    }

    fn hook_dir(&self, platform: &str) -> Result<PathBuf, AdapterError> {
        Ok(self.base_config_dir.join(platform_hook_subpath(platform)?))
    }
}

pub fn install_hook(
    platform: &str,
    hook_type: &str,
    script_content: &str,
) -> Result<PathBuf, AdapterError> {
    HookInstaller::default()?.install_hook(platform, hook_type, script_content)
}

pub fn uninstall_hook(platform: &str, hook_type: &str) -> Result<(), AdapterError> {
    HookInstaller::default()?.uninstall_hook(platform, hook_type)
}

pub fn list_hooks() -> Result<Vec<HookScript>, AdapterError> {
    HookInstaller::default()?.list_hooks()
}

pub fn is_hook_installed(platform: &str, hook_type: &str) -> bool {
    HookInstaller::default()
        .map(|installer| installer.is_hook_installed(platform, hook_type))
        .unwrap_or(false)
}

fn supported_platforms() -> &'static [&'static str] {
    &[
        "claude-code",
        "cursor",
        "gemini-cli",
        "vscode-copilot",
        "codex",
    ]
}

fn platform_hook_subpath(platform: &str) -> Result<&'static Path, AdapterError> {
    match platform {
        "claude-code" => Ok(Path::new(".claude/hooks")),
        "cursor" => Ok(Path::new(".cursor/hooks")),
        "gemini-cli" => Ok(Path::new(".gemini/hooks")),
        "vscode-copilot" => Ok(Path::new(".vscode/hooks")),
        "codex" => Ok(Path::new(".codex/hooks")),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported platform: {platform}"),
        )
        .into()),
    }
}

fn script_extension() -> &'static str {
    if cfg!(windows) { "cmd" } else { "sh" }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), AdapterError> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), AdapterError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn list_hooks_returns_empty_when_no_hooks_installed() {
        let dir = tempdir().unwrap();
        let installer = HookInstaller::new(dir.path().to_path_buf());

        let hooks = installer.list_hooks().unwrap();

        assert!(hooks.is_empty());
    }

    #[test]
    fn install_hook_creates_file_with_correct_content() {
        let dir = tempdir().unwrap();
        let installer = HookInstaller::new(dir.path().to_path_buf());
        let content = "#!/bin/bash\n# context-mode hook for claude-code pre_tool_use\ncontext-mode hook --platform=claude-code --type=pre_tool_use \"$@\"\n";

        let path = installer
            .install_hook("claude-code", "pre_tool_use", content)
            .unwrap();

        assert!(path.exists());
        assert_eq!(fs::read_to_string(path).unwrap(), content);
    }

    #[test]
    fn is_hook_installed_tracks_install_and_uninstall() {
        let dir = tempdir().unwrap();
        let installer = HookInstaller::new(dir.path().to_path_buf());
        let content = "#!/bin/bash\n# context-mode hook for cursor post_tool_use\ncontext-mode hook --platform=cursor --type=post_tool_use \"$@\"\n";

        assert!(!installer.is_hook_installed("cursor", "post_tool_use"));

        installer
            .install_hook("cursor", "post_tool_use", content)
            .unwrap();
        assert!(installer.is_hook_installed("cursor", "post_tool_use"));

        installer.uninstall_hook("cursor", "post_tool_use").unwrap();
        assert!(!installer.is_hook_installed("cursor", "post_tool_use"));
    }
}
