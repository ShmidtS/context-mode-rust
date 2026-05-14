use regex::Regex;
use std::path::Path;
use std::process::Command;

/// Supported execution languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    JavaScript,
    TypeScript,
    Python,
    Shell,
    Ruby,
    Go,
    Rust,
    Php,
    Perl,
    R,
    Elixir,
}

/// Runtime command paths (None if not available).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeMap {
    pub javascript: Option<String>,
    pub typescript: Option<String>,
    pub python: Option<String>,
    pub shell: Option<String>,
    pub ruby: Option<String>,
    pub go: Option<String>,
    pub rust: Option<String>,
    pub php: Option<String>,
    pub perl: Option<String>,
    pub r: Option<String>,
    pub elixir: Option<String>,
}

/// Check if a shell path basename is allowlisted (bash, sh, zsh, dash, pwsh, powershell, cmd).
pub fn is_allowlisted_shell(shell_path: &str) -> bool {
    let Some(name) = Path::new(shell_path)
        .file_name()
        .and_then(|name| name.to_str())
    else {
        return false;
    };
    Regex::new(r"(?i)^(bash|sh|zsh|dash|pwsh|powershell|cmd)(\.exe)?$")
        .expect("shell allowlist regex should compile")
        .is_match(name)
}

/// Detect available runtimes by checking `which` / `where` commands.
pub fn detect_runtimes() -> RuntimeMap {
    let bun = detect_bun();

    RuntimeMap {
        javascript: bun.clone().or_else(|| detect_command("node")),
        typescript: bun
            .or_else(|| detect_command("tsx"))
            .or_else(|| detect_command("ts-node"))
            .or_else(|| detect_command("deno")),
        python: detect_command("python3").or_else(|| detect_command("python")),
        shell: detect_shell(),
        ruby: detect_command("ruby"),
        go: detect_command("go"),
        rust: detect_command("rustc"),
        php: detect_command("php"),
        perl: detect_command("perl"),
        r: detect_command("Rscript"),
        elixir: detect_command("elixir"),
    }
}

/// Build the command array to execute a script file for a given language.
pub fn build_command(runtimes: &RuntimeMap, language: Language, file_path: &str) -> Vec<String> {
    match language {
        Language::JavaScript => command_with_file(&runtimes.javascript, file_path),
        Language::TypeScript => command_with_file(&runtimes.typescript, file_path),
        Language::Python => command_with_file(&runtimes.python, file_path),
        Language::Shell => command_with_file(&runtimes.shell, file_path),
        Language::Ruby => command_with_file(&runtimes.ruby, file_path),
        Language::Go => command_with_args(&runtimes.go, &["run", file_path]),
        Language::Rust => command_with_file(&runtimes.rust, file_path),
        Language::Php => command_with_file(&runtimes.php, file_path),
        Language::Perl => command_with_file(&runtimes.perl, file_path),
        Language::R => command_with_file(&runtimes.r, file_path),
        Language::Elixir => command_with_file(&runtimes.elixir, file_path),
    }
}

fn command_with_file(runtime: &Option<String>, file_path: &str) -> Vec<String> {
    runtime
        .as_ref()
        .map(|runtime| vec![runtime.clone(), file_path.to_string()])
        .unwrap_or_default()
}

fn command_with_args(runtime: &Option<String>, args: &[&str]) -> Vec<String> {
    runtime
        .as_ref()
        .map(|runtime| {
            let mut command = Vec::with_capacity(args.len() + 1);
            command.push(runtime.clone());
            command.extend(args.iter().map(|arg| arg.to_string()));
            command
        })
        .unwrap_or_default()
}

fn detect_command(cmd: &str) -> Option<String> {
    let lookup = if cfg!(windows) { "where" } else { "which" };
    if let Some(path) = Command::new(lookup)
        .arg(cmd)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(str::to_string)
        })
    {
        return Some(path);
    }

    which::which(cmd)
        .ok()
        .map(|path| path.to_string_lossy().to_string())
}

fn detect_bun() -> Option<String> {
    detect_command("bun").or_else(|| {
        let candidates = bun_fallback_paths();
        candidates
            .into_iter()
            .find(|candidate| Path::new(candidate).exists())
    })
}

fn bun_fallback_paths() -> Vec<String> {
    let mut candidates = Vec::new();

    if cfg!(windows) {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            candidates.push(
                Path::new(&local_app_data)
                    .join("bun")
                    .join("bin")
                    .join("bun.exe")
                    .to_string_lossy()
                    .to_string(),
            );
        }
    } else if let Some(home) = std::env::var_os("HOME") {
        candidates.push(
            Path::new(&home)
                .join(".bun")
                .join("bin")
                .join("bun")
                .to_string_lossy()
                .to_string(),
        );
    }

    candidates
}

fn detect_shell() -> Option<String> {
    if cfg!(windows) {
        detect_windows_bash()
            .or_else(|| detect_command("pwsh"))
            .or_else(|| detect_command("powershell"))
    } else {
        detect_command("bash")
            .or_else(|| detect_command("sh"))
            .filter(|shell| is_allowlisted_shell(shell))
    }
}

fn detect_windows_bash() -> Option<String> {
    let known_paths = [
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files\Git\usr\bin\bash.exe",
        r"C:\msys64\usr\bin\bash.exe",
        r"C:\cygwin64\bin\bash.exe",
    ];

    known_paths
        .iter()
        .find(|path| Path::new(path).exists())
        .map(|path| (*path).to_string())
        .or_else(|| {
            let output = Command::new("where").arg("bash").output().ok()?;
            if !output.status.success() {
                return None;
            }

            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .find(|line| {
                    let lower = line.to_ascii_lowercase();
                    is_allowlisted_shell(line)
                        && !lower.contains(r"\system32\")
                        && !lower.contains(r"\windowsapps\")
                })
                .map(str::to_string)
        })
}
