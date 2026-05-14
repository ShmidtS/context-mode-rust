use context_mode_core::executor::{ExecuteOptions, PolyglotExecutor};
use context_mode_core::runtime::{
    Language, RuntimeMap, build_command, detect_shell, is_allowlisted_shell,
};

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
    assert_eq!(
        build_command(&runtimes, Language::Shell, "script.sh"),
        vec!["bash", "script.sh"]
    );
}

#[tokio::test]
#[cfg_attr(windows, ignore)]
async fn executor_kills_process_when_output_exceeds_hard_cap() {
    let project_root = tempfile::tempdir().unwrap();
    let mut runtimes = empty_runtimes();
    runtimes.shell = Some("bash".to_string());
    let executor = PolyglotExecutor {
        runtimes,
        project_root: project_root.path().to_string_lossy().to_string(),
        hard_cap_bytes: 128,
    };

    let result = executor
        .execute(ExecuteOptions {
            language: Language::Shell,
            code: "printf '%*s' 4096 x | tr ' ' x; sleep 2".to_string(),
            timeout_ms: Some(500),
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
#[cfg_attr(windows, ignore)]
async fn executor_kills_process_when_combined_output_exceeds_hard_cap() {
    let project_root = tempfile::tempdir().unwrap();
    let mut runtimes = empty_runtimes();
    runtimes.shell = Some("bash".to_string());
    let executor = PolyglotExecutor {
        runtimes,
        project_root: project_root.path().to_string_lossy().to_string(),
        hard_cap_bytes: 128,
    };

    let result = executor
        .execute(ExecuteOptions {
            language: Language::Shell,
            code: "printf '%*s' 80 x | tr ' ' x; printf '%*s' 80 y | tr ' ' y >&2; sleep 2"
                .to_string(),
            timeout_ms: Some(500),
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
async fn executor_runs_shell_code_and_captures_output() {
    let project_root = tempfile::tempdir().unwrap();
    let mut runtimes = empty_runtimes();
    let shell = detect_shell();
    if shell.is_none() {
        return;
    }
    runtimes.shell = shell;
    let executor = PolyglotExecutor {
        runtimes,
        project_root: project_root.path().to_string_lossy().to_string(),
        hard_cap_bytes: 1024,
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
