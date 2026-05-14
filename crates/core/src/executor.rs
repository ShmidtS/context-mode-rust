use crate::runtime::{Language, RuntimeMap, build_command, detect_runtimes};
use crate::types::ExecResult;
use anyhow::{Context, anyhow};
use std::io::Write;
use std::process::Stdio;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tempfile::NamedTempFile;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::time::{Duration, sleep, timeout};

/// Options for executing code.
pub struct ExecuteOptions {
    pub language: Language,
    pub code: String,
    pub timeout_ms: Option<u64>,
    pub background: bool,
    pub project_root: String,
    pub hard_cap_bytes: usize,
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

        let mut temp_file = NamedTempFile::with_suffix(language_extension(opts.language))?;
        temp_file.write_all(wrap_code(opts.language, &opts.code).as_bytes())?;
        temp_file.flush()?;
        let file_path = temp_file.path().to_string_lossy().to_string();

        let command_parts = build_command(&self.runtimes, opts.language, &file_path);
        if command_parts.is_empty() {
            return Err(anyhow!("runtime for {:?} is not available", opts.language));
        }

        let mut command = Command::new(&command_parts[0]);
        command
            .args(&command_parts[1..])
            .current_dir(project_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(!opts.background);

        let mut child = command
            .spawn()
            .with_context(|| format!("failed to spawn {}", command_parts[0]))?;
        let stdout = child.stdout.take().context("failed to capture stdout")?;
        let stderr = child.stderr.take().context("failed to capture stderr")?;
        let timeout_ms = opts.timeout_ms.unwrap_or(30_000);

        let execution = async {
            let result = wait_with_output_cap(&mut child, stdout, stderr, hard_cap_bytes).await?;

            Ok::<ExecResult, anyhow::Error>(ExecResult {
                stdout: String::from_utf8_lossy(&result.stdout).to_string(),
                stderr: String::from_utf8_lossy(&result.stderr).to_string(),
                exit_code: result.exit_code,
                timed_out: false,
                backgrounded: false,
            })
        };

        match timeout(Duration::from_millis(timeout_ms), execution).await {
            Ok(result) => result,
            Err(_) if opts.background => {
                let _ = temp_file.keep();
                Ok(ExecResult {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 0,
                    timed_out: true,
                    backgrounded: true,
                })
            }
            Err(_) => Err(anyhow!("execution timed out after {} ms", timeout_ms)),
        }
    }
}

struct ProcessOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_code: i32,
}

struct CappedOutput {
    bytes: Vec<u8>,
    capped: bool,
}

async fn wait_with_output_cap<R1, R2>(
    child: &mut Child,
    stdout: R1,
    stderr: R2,
    cap: usize,
) -> anyhow::Result<ProcessOutput>
where
    R1: AsyncRead + Send + Unpin + 'static,
    R2: AsyncRead + Send + Unpin + 'static,
{
    let total_bytes = Arc::new(AtomicUsize::new(0));
    let mut stdout_task = tokio::spawn(read_capped(stdout, cap, total_bytes.clone()));
    let mut stderr_task = tokio::spawn(read_capped(stderr, cap, total_bytes));
    let mut stdout_done = None;
    let mut stderr_done = None;
    let mut exit_code = None;

    loop {
        tokio::select! {
            status = child.wait(), if exit_code.is_none() => {
                exit_code = Some(status?.code().unwrap_or(-1));
            }
            result = &mut stdout_task, if stdout_done.is_none() => {
                let output = result??;
                let capped = output.capped;
                stdout_done = Some(output);
                if capped {
                    terminate_child(child).await;
                    if exit_code.is_none() {
                        exit_code = Some(child.wait().await?.code().unwrap_or(-1));
                    }
                }
            }
            result = &mut stderr_task, if stderr_done.is_none() => {
                let output = result??;
                let capped = output.capped;
                stderr_done = Some(output);
                if capped {
                    terminate_child(child).await;
                    if exit_code.is_none() {
                        exit_code = Some(child.wait().await?.code().unwrap_or(-1));
                    }
                }
            }
            _ = sleep(Duration::from_millis(1)), if exit_code.is_some() && stdout_done.is_some() && stderr_done.is_some() => {
                break;
            }
        }
    }

    let mut stdout = stdout_done.map(|output| output.bytes).unwrap_or_default();
    let mut stderr = stderr_done.map(|output| output.bytes).unwrap_or_default();
    let mut capped = false;
    enforce_total_cap(&mut stdout, &mut stderr, cap, &mut capped);

    Ok(ProcessOutput {
        stdout,
        stderr,
        exit_code: exit_code.unwrap_or(-1),
    })
}

async fn terminate_child(child: &mut Child) {
    let _ = child.start_kill();
}

async fn read_capped<R>(
    mut reader: R,
    cap: usize,
    total_bytes: Arc<AtomicUsize>,
) -> anyhow::Result<CappedOutput>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut capped = false;

    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }

        let previous_total = total_bytes.fetch_add(read, Ordering::Relaxed);
        if previous_total >= cap {
            capped = true;
            break;
        }

        let remaining_total = cap - previous_total;
        let stream_remaining = cap.saturating_sub(bytes.len());
        let keep = read.min(remaining_total).min(stream_remaining);
        bytes.extend_from_slice(&buffer[..keep]);

        if read > keep || previous_total + read >= cap {
            capped = true;
            break;
        }
    }

    Ok(CappedOutput { bytes, capped })
}

fn enforce_total_cap(stdout: &mut Vec<u8>, stderr: &mut Vec<u8>, cap: usize, capped: &mut bool) {
    let total = stdout.len() + stderr.len();
    if total <= cap {
        return;
    }

    *capped = true;
    if stdout.len() >= cap {
        stdout.truncate(cap);
        stderr.clear();
    } else {
        stderr.truncate(cap - stdout.len());
    }
}

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
