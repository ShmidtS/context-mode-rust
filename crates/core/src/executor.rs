use crate::runtime::{Language, RuntimeMap, build_command, detect_runtimes, is_posix_shell};
use crate::types::ExecResult;
use anyhow::{Context, anyhow};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::time::SystemTime;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::time::Duration;

const BACKGROUND_LOG_LIMIT_BYTES: u64 = 100 * 1024 * 1024;

/// Options for executing code.
pub struct ExecuteOptions {
    pub language: Language,
    pub code: String,
    pub timeout_ms: Option<u64>,
    pub background: bool,
    pub project_root: String,
    pub hard_cap_bytes: usize,
}

impl ExecuteOptions {
    pub fn sanitized_env() -> HashMap<String, String> {
        Self::sanitize_env_vars(std::env::vars())
    }

    pub fn sanitize_env_vars<I>(vars: I) -> HashMap<String, String>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        vars.into_iter()
            .filter(|(key, _)| !is_stripped_env_var(key))
            .collect()
    }
}

pub struct PolyglotExecutor {
    pub runtimes: RuntimeMap,
    pub project_root: String,
    pub hard_cap_bytes: usize,
}

impl PolyglotExecutor {
    pub fn new(project_root: String) -> Self {
        Self {
            runtimes: detect_runtimes(),
            project_root,
            hard_cap_bytes: 1024 * 1024,
        }
    }

    pub async fn execute(&self, opts: ExecuteOptions) -> anyhow::Result<ExecResult> {
        let project_root = if opts.project_root.is_empty() {
            self.project_root.as_str()
        } else {
            opts.project_root.as_str()
        };
        let hard_cap_bytes = if opts.hard_cap_bytes == 0 {
            self.hard_cap_bytes
        } else {
            opts.hard_cap_bytes
        };

        let tmp_dir = tempfile::tempdir()?;
        let file_name = format!("script{}", language_extension(opts.language));
        let file_path = tmp_dir.path().join(&file_name);
        std::fs::write(&file_path, wrap_code(opts.language, &opts.code))?;
        let file_path_str = file_path.to_string_lossy().to_string();

        let arg_path = command_file_path(opts.language, &self.runtimes, file_path_str);
        let command_parts = build_command(&self.runtimes, opts.language, &arg_path);
        if command_parts.is_empty() {
            return Err(anyhow!("runtime for {:?} is not available", opts.language));
        }

        if opts.background {
            return self
                .execute_background(command_parts, project_root, opts.timeout_ms, tmp_dir)
                .await;
        }

        let command = build_process_command(
            &command_parts,
            project_root,
            opts.language == Language::Shell,
        );
        execute_foreground(command, hard_cap_bytes, opts.timeout_ms).await
    }

    async fn execute_background(
        &self,
        command_parts: Vec<String>,
        project_root: &str,
        timeout_ms: Option<u64>,
        tmp_dir: tempfile::TempDir,
    ) -> anyhow::Result<ExecResult> {
        let log_dir = Path::new(project_root)
            .join(".ctx")
            .join("logs")
            .join("background");
        tokio::fs::create_dir_all(&log_dir).await?;
        cleanup_background_logs(&log_dir).await?;

        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("pending.out"))?;
        let stderr_file = log_file.try_clone()?;
        let mut command = build_process_command(&command_parts, project_root, false);
        command
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(stderr_file));

        let mut child = command
            .spawn()
            .with_context(|| format!("failed to spawn {}", command_parts[0]))?;
        let pid = child.id();
        let log_path = pid.map(|pid| {
            log_dir
                .join(format!("{pid}.out"))
                .to_string_lossy()
                .to_string()
        });
        if let Some(ref path) = log_path {
            let _ = tokio::fs::rename(log_dir.join("pending.out"), path).await;
        }

        let _ = tmp_dir.keep();
        if let Some(timeout_ms) = timeout_ms {
            if tokio::time::timeout(Duration::from_millis(timeout_ms), child.wait())
                .await
                .is_err()
            {
                kill_child_tree(&mut child).await;
                return Ok(ExecResult {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 0,
                    timed_out: true,
                    backgrounded: true,
                    pid,
                    log_path: log_path.clone(),
                });
            }
        }

        Ok(ExecResult {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
            timed_out: false,
            backgrounded: true,
            pid,
            log_path,
        })
    }
}

async fn execute_foreground(
    mut command: Command,
    hard_cap_bytes: usize,
    timeout_ms: Option<u64>,
) -> anyhow::Result<ExecResult> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().context("failed to spawn process")?;
    let stdout_pipe = child.stdout.take().context("failed to capture stdout")?;
    let stderr_pipe = child.stderr.take().context("failed to capture stderr")?;
    let (tx, mut rx) = mpsc::channel::<(bool, Vec<u8>)>(16);

    let stdout_tx = tx.clone();
    tokio::spawn(async move {
        read_stream(stdout_pipe, true, stdout_tx).await;
    });
    tokio::spawn(async move {
        read_stream(stderr_pipe, false, tx).await;
    });

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut timeout_sleep =
        timeout_ms.map(|ms| Box::pin(tokio::time::sleep(Duration::from_millis(ms))));
    let mut status_code = None;
    let mut timed_out = false;

    {
        let child_wait = child.wait();
        tokio::pin!(child_wait);
        loop {
            tokio::select! {
                status = &mut child_wait, if status_code.is_none() => {
                    status_code = Some(status?.code().unwrap_or(-1));
                }
                _ = async {
                    if let Some(sleep) = timeout_sleep.as_mut() {
                        sleep.await;
                    }
                }, if timeout_sleep.is_some() && status_code.is_none() => {
                    timed_out = true;
                    break;
                }
                Some((is_stdout, chunk)) = rx.recv() => {
                    append_capped(&mut stdout, &mut stderr, is_stdout, &chunk, hard_cap_bytes);
                    if stdout.len() + stderr.len() >= hard_cap_bytes {
                        break;
                    }
                }
                else => break,
            }
        }
    }

    if status_code.is_none() {
        kill_child_tree(&mut child).await;
        let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
    }

    while let Ok((is_stdout, chunk)) = rx.try_recv() {
        append_capped(&mut stdout, &mut stderr, is_stdout, &chunk, hard_cap_bytes);
    }

    Ok(ExecResult {
        stdout: String::from_utf8_lossy(&stdout).to_string(),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
        exit_code: status_code.unwrap_or(-1),
        timed_out,
        backgrounded: false,
        pid: None,
        log_path: None,
    })
}

async fn read_stream<R>(mut reader: R, is_stdout: bool, tx: mpsc::Sender<(bool, Vec<u8>)>)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buf = [0_u8; 8192];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if tx.send((is_stdout, buf[..n].to_vec())).await.is_err() {
                    break;
                }
            }
        }
    }
}

fn append_capped(
    stdout: &mut Vec<u8>,
    stderr: &mut Vec<u8>,
    is_stdout: bool,
    chunk: &[u8],
    cap: usize,
) {
    let used = stdout.len() + stderr.len();
    if used >= cap {
        return;
    }
    let available = cap - used;
    let take = available.min(chunk.len());
    if is_stdout {
        stdout.extend_from_slice(&chunk[..take]);
    } else {
        stderr.extend_from_slice(&chunk[..take]);
    }
}

fn build_process_command(command_parts: &[String], project_root: &str, shell: bool) -> Command {
    let mut command = Command::new(&command_parts[0]);
    command
        .args(&command_parts[1..])
        .current_dir(project_root)
        .stdin(Stdio::null())
        .env_clear()
        .envs(ExecuteOptions::sanitized_env());
    if shell {
        command
            .env("MSYS_NO_PATHCONV", "1")
            .env("MSYS2_ARG_CONV_EXCL", "*");
    }
    hide_windows_console(&mut command);
    command
}

fn command_file_path(language: Language, runtimes: &RuntimeMap, file_path: String) -> String {
    if language == Language::Shell && is_posix_shell(runtimes.shell.as_deref()) {
        file_path.replace('\\', "/")
    } else {
        file_path
    }
}

fn is_stripped_env_var(key: &str) -> bool {
    matches!(
        key,
        "BASH_ENV"
            | "NODE_OPTIONS"
            | "PYTHONSTARTUP"
            | "LD_PRELOAD"
            | "CC"
            | "CXX"
            | "CFLAGS"
            | "LDFLAGS"
            | "GIT_CONFIG_GLOBAL"
            | "GIT_CONFIG_SYSTEM"
    )
}

async fn cleanup_background_logs(log_dir: &Path) -> anyhow::Result<()> {
    let mut entries = Vec::new();
    let mut read_dir = tokio::fs::read_dir(log_dir).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let metadata = entry.metadata().await?;
        if metadata.is_file() {
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            entries.push((entry.path(), metadata.len(), modified));
        }
    }

    let mut total: u64 = entries.iter().map(|(_, len, _)| *len).sum();
    entries.sort_by_key(|(_, _, modified)| *modified);
    for (path, len, _) in entries {
        if total <= BACKGROUND_LOG_LIMIT_BYTES {
            break;
        }
        if tokio::fs::remove_file(path).await.is_ok() {
            total = total.saturating_sub(len);
        }
    }
    Ok(())
}

async fn kill_child_tree(child: &mut Child) {
    let pid = child.id();
    let _ = child.kill().await;
    kill_process_tree(pid).await;
}

#[cfg(windows)]
async fn kill_process_tree(pid: Option<u32>) {
    if let Some(pid) = pid {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }
}

#[cfg(not(windows))]
async fn kill_process_tree(_pid: Option<u32>) {}

#[cfg(windows)]
fn hide_windows_console(command: &mut Command) {
    command.creation_flags(0x08000000);
}

#[cfg(not(windows))]
fn hide_windows_console(_command: &mut Command) {}

fn wrap_code(language: Language, code: &str) -> String {
    match language {
        Language::Go if !code.contains("package main") => {
            if code.contains("func main") {
                format!("package main\n\n{code}")
            } else {
                format!("package main\n\nfunc main() {{\n{code}\n}}\n")
            }
        }
        Language::Php if !code.trim_start().starts_with("<?php") => format!("<?php\n{code}"),
        _ => code.to_string(),
    }
}

fn language_extension(language: Language) -> &'static str {
    match language {
        Language::JavaScript => ".js",
        Language::TypeScript => ".ts",
        Language::Python => ".py",
        Language::Shell if cfg!(windows) => "",
        Language::Shell => ".sh",
        Language::Ruby => ".rb",
        Language::Go => ".go",
        Language::Rust => ".rs",
        Language::Php => ".php",
        Language::Perl => ".pl",
        Language::R => ".R",
        Language::Elixir => ".exs",
    }
}
