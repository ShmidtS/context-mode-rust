use context_mode_core::executor::{ExecuteOptions, PolyglotExecutor};
use context_mode_core::runtime::{
    Language, RuntimeMap, build_command, detect_shell, is_allowlisted_shell,
};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

fn empty_runtimes() -> RuntimeMap {
    RuntimeMap {
        javascript: None,
        typescript: None,
        python: None,
        shell: None,
        ruby: None,
        go: None,
        rust: None,
        php: None,
        perl: None,
        r: None,
        elixir: None,
    }
}

fn shell_executor(project_root: &Path, hard_cap_bytes: usize) -> Option<PolyglotExecutor> {
    let mut runtimes = empty_runtimes();
    runtimes.shell = detect_shell();
    runtimes.shell.as_ref()?;
    Some(PolyglotExecutor {
        runtimes,
        project_root: project_root.to_string_lossy().to_string(),
        hard_cap_bytes,
    })
}

#[test]
fn allowlisted_shell_accepts_known_basenames_case_insensitively() {
    assert!(is_allowlisted_shell("/usr/bin/bash"));
    assert!(is_allowlisted_shell(
        "C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe"
    ));
    assert!(is_allowlisted_shell("PWsh.EXE"));
    assert!(!is_allowlisted_shell("/tmp/not-bash"));
    assert!(!is_allowlisted_shell("bash-malicious"));
}

#[test]
fn build_command_uses_language_specific_runtime_arguments() {
    let mut runtimes = empty_runtimes();
    runtimes.rust = Some("rustc".to_string());
    runtimes.go = Some("go".to_string());
    runtimes.shell = Some("bash".to_string());

    assert_eq!(
        build_command(&runtimes, Language::Rust, "main.rs"),
        vec!["rustc", "main.rs"]
    );
    assert_eq!(
        build_command(&runtimes, Language::Go, "main.go"),
        vec!["go", "run", "main.go"]
    );
    let shell_command = build_command(&runtimes, Language::Shell, "script.sh");
    if cfg!(windows) {
        assert_eq!(
            shell_command,
            vec![
                "bash",
                "-c",
                "export PATH='/usr/bin:$PATH'; source 'script.sh'"
            ]
        );
    } else {
        assert_eq!(shell_command, vec!["bash", "script.sh"]);
    }
}

#[tokio::test]
async fn executor_kills_process_when_output_exceeds_hard_cap() {
    let project_root = tempfile::tempdir().unwrap();
    let Some(executor) = shell_executor(project_root.path(), 128) else {
        return;
    };

    let result = executor
        .execute(ExecuteOptions {
            language: Language::Shell,
            code: "printf '%*s' 4096 x | tr ' ' x; sleep 2".to_string(),
            timeout_ms: Some(5_000),
            background: false,
            project_root: project_root.path().to_string_lossy().to_string(),
            hard_cap_bytes: 128,
        })
        .await
        .unwrap();

    assert!(result.stdout.len() <= 128);
    assert!(!result.timed_out);
    assert!(!result.backgrounded);
}

#[tokio::test]
async fn executor_kills_process_when_combined_output_exceeds_hard_cap() {
    let project_root = tempfile::tempdir().unwrap();
    let Some(executor) = shell_executor(project_root.path(), 128) else {
        return;
    };

    let result = executor
        .execute(ExecuteOptions {
            language: Language::Shell,
            code: "printf '%*s' 80 x | tr ' ' x; printf '%*s' 80 y | tr ' ' y >&2; sleep 2"
                .to_string(),
            timeout_ms: Some(5_000),
            background: false,
            project_root: project_root.path().to_string_lossy().to_string(),
            hard_cap_bytes: 128,
        })
        .await
        .unwrap();

    assert!(result.stdout.len() + result.stderr.len() <= 128);
    assert!(!result.timed_out);
    assert!(!result.backgrounded);
}

#[tokio::test]
async fn executor_timeout_returns_partial_output_and_cleans_up() {
    let project_root = tempfile::tempdir().unwrap();
    let Some(executor) = shell_executor(project_root.path(), 1024) else {
        return;
    };

    let result = executor
        .execute(ExecuteOptions {
            language: Language::Shell,
            code: "echo before-timeout; sleep 5".to_string(),
            timeout_ms: Some(100),
            background: false,
            project_root: project_root.path().to_string_lossy().to_string(),
            hard_cap_bytes: 1024,
        })
        .await
        .unwrap();

    assert!(result.timed_out);
    assert!(result.stdout.contains("before-timeout"));
    assert!(!result.backgrounded);
}

#[tokio::test]
async fn executor_background_detaches_and_creates_log() {
    let project_root = tempfile::tempdir().unwrap();
    let Some(executor) = shell_executor(project_root.path(), 1024) else {
        return;
    };

    let result = executor
        .execute(ExecuteOptions {
            language: Language::Shell,
            code: "echo background-ready; sleep 5".to_string(),
            timeout_ms: None,
            background: true,
            project_root: project_root.path().to_string_lossy().to_string(),
            hard_cap_bytes: 1024,
        })
        .await
        .unwrap();

    assert!(result.backgrounded);
    let pid = result.pid.expect("background result should include pid");
    let log_path = project_root
        .path()
        .join(".ctx")
        .join("logs")
        .join("background")
        .join(format!("{pid}.out"));
    assert!(log_path.exists());
    assert!(process_exists(pid));
    kill_process(pid);
}

#[tokio::test]
async fn executor_smoke_runs_echo_and_python() {
    let project_root = tempfile::tempdir().unwrap();
    let Some(shell_executor) = shell_executor(project_root.path(), 1024) else {
        return;
    };

    let echo = shell_executor
        .execute(ExecuteOptions {
            language: Language::Shell,
            code: "echo hello".to_string(),
            timeout_ms: None,
            background: false,
            project_root: project_root.path().to_string_lossy().to_string(),
            hard_cap_bytes: 1024,
        })
        .await
        .unwrap();
    assert!(echo.stdout.contains("hello"));

    let mut runtimes = empty_runtimes();
    runtimes.python = which::which("python3")
        .or_else(|_| which::which("python"))
        .ok()
        .map(|path| path.to_string_lossy().to_string());
    if runtimes.python.is_none() {
        return;
    }
    let python_executor = PolyglotExecutor {
        runtimes,
        project_root: project_root.path().to_string_lossy().to_string(),
        hard_cap_bytes: 1024,
    };
    let python = python_executor
        .execute(ExecuteOptions {
            language: Language::Python,
            code: "print('ok')".to_string(),
            timeout_ms: None,
            background: false,
            project_root: project_root.path().to_string_lossy().to_string(),
            hard_cap_bytes: 1024,
        })
        .await
        .unwrap();
    assert!(python.stdout.contains("ok"));
}

#[test]
fn execute_options_sanitized_env_strips_injection_vars() {
    let env = ExecuteOptions::sanitize_env_vars([
        ("BASH_ENV".to_string(), "bad".to_string()),
        ("NODE_OPTIONS".to_string(), "bad".to_string()),
        ("PYTHONSTARTUP".to_string(), "bad".to_string()),
        ("LD_PRELOAD".to_string(), "bad".to_string()),
        ("CC".to_string(), "bad".to_string()),
        ("GIT_CONFIG_GLOBAL".to_string(), "bad".to_string()),
        ("PATH".to_string(), "ok".to_string()),
    ]);
    assert!(!env.contains_key("BASH_ENV"));
    assert!(!env.contains_key("NODE_OPTIONS"));
    assert!(!env.contains_key("PYTHONSTARTUP"));
    assert!(!env.contains_key("LD_PRELOAD"));
    assert!(!env.contains_key("CC"));
    assert!(!env.contains_key("GIT_CONFIG_GLOBAL"));
    assert_eq!(env.get("PATH"), Some(&"ok".to_string()));
}

#[test]
fn executor_runs_shell_code_sync() {
    let shell = detect_shell();
    if shell.is_none() {
        return;
    }
    let shell = shell.unwrap();
    let output = Command::new(&shell)
        .args(["-c", "echo hello"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("hello"),
        "stdout={} stderr={}",
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn executor_runs_shell_code_and_captures_output() {
    let project_root = tempfile::tempdir().unwrap();
    let Some(executor) = shell_executor(project_root.path(), 1024) else {
        return;
    };

    let result = executor
        .execute(ExecuteOptions {
            language: Language::Shell,
            code: "printf hello".to_string(),
            timeout_ms: Some(5_000),
            background: false,
            project_root: project_root.path().to_string_lossy().to_string(),
            hard_cap_bytes: 1024,
        })
        .await
        .unwrap();

    assert_eq!(result.stdout, "hello");
    assert_eq!(result.stderr, "");
    assert_eq!(result.exit_code, 0);
    assert!(!result.timed_out);
    assert!(!result.backgrounded);
}

fn process_exists(pid: u32) -> bool {
    if cfg!(windows) {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}")])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    } else {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

fn kill_process(pid: u32) {
    if cfg!(windows) {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .status();
    } else {
        let _ = Command::new("kill").arg(pid.to_string()).status();
    }
    std::thread::sleep(Duration::from_millis(50));
}
