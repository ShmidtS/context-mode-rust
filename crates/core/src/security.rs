use regex::Regex;
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};

const REGEX_CACHE_MAX: usize = 256;

pub type PermissionDecision = &'static str;

pub struct SecurityPolicy {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub ask: Vec<String>,
}

struct RegexCache {
    entries: HashMap<String, Regex>,
    order: VecDeque<String>,
}

static REGEX_CACHE: OnceLock<Mutex<RegexCache>> = OnceLock::new();

fn regex_cache() -> &'static Mutex<RegexCache> {
    REGEX_CACHE.get_or_init(|| {
        Mutex::new(RegexCache {
            entries: HashMap::new(),
            order: VecDeque::new(),
        })
    })
}

/// Extract the glob from a Bash permission pattern like "Bash(sudo *)".
pub fn parse_bash_pattern(pattern: &str) -> Option<&str> {
    let (tool, glob) = parse_tool_pattern(pattern)?;
    (tool == "Bash").then_some(glob)
}

/// Parse a tool permission pattern like "ToolName(glob)".
pub fn parse_tool_pattern(pattern: &str) -> Option<(&str, &str)> {
    let open = pattern.find('(')?;
    let close = pattern.rfind(')')?;

    if close != pattern.len() - 1 || open == 0 || close <= open + 1 {
        return None;
    }

    Some((&pattern[..open], &pattern[open + 1..close]))
}

/// Convert a Bash permission glob to a Regex.
pub fn glob_to_regex(glob: &str, case_insensitive: bool) -> Result<Regex, regex::Error> {
    let key = format!("bash:{case_insensitive}:{glob}");
    cached_regex(key, || {
        let body = if let Some(command) = glob.strip_suffix(":*") {
            format!("{}(\\s.*)?", escape_regex(command))
        } else {
            convert_glob_part(glob)
        };
        let prefix = if case_insensitive { "(?i)" } else { "" };
        Regex::new(&format!("{prefix}^{body}$"))
    })
}

/// Convert a file path glob to a regex.
pub fn file_glob_to_regex(glob: &str, case_insensitive: bool) -> Result<Regex, regex::Error> {
    let key = format!("file:{case_insensitive}:{glob}");
    cached_regex(key, || {
        let mut body = String::new();
        let mut chars = glob.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '*' && chars.peek() == Some(&'*') {
                chars.next();
                if chars.peek() == Some(&'/') {
                    chars.next();
                    body.push_str("(?:.*/)?");
                } else {
                    body.push_str(".*");
                }
            } else if ch == '*' {
                body.push_str("[^/]*");
            } else {
                body.push_str(&escape_regex_char(ch));
            }
        }

        let prefix = if case_insensitive { "(?i)" } else { "" };
        Regex::new(&format!("{prefix}^{body}$"))
    })
}

/// Check if a command matches any Bash pattern in the list.
pub fn matches_any_pattern(
    command: &str,
    patterns: &[String],
    case_insensitive: bool,
) -> Option<String> {
    patterns.iter().find_map(|pattern| {
        let glob = parse_bash_pattern(pattern)?;
        let regex = glob_to_regex(glob, case_insensitive).ok()?;
        regex.is_match(command).then(|| glob.to_string())
    })
}

/// Split a shell command on chain operators (&&, ||, ;, |) respecting quotes.
pub fn split_chained_commands(command: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    let mut in_backtick = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        if ch == '\\' {
            current.push(ch);
            escaped = true;
            continue;
        }

        match ch {
            '\'' if !in_double && !in_backtick => {
                in_single = !in_single;
                current.push(ch);
            }
            '"' if !in_single && !in_backtick => {
                in_double = !in_double;
                current.push(ch);
            }
            '`' if !in_single && !in_double => {
                in_backtick = !in_backtick;
                current.push(ch);
            }
            '&' if !in_single && !in_double && !in_backtick && chars.peek() == Some(&'&') => {
                chars.next();
                push_command(&mut commands, &mut current);
            }
            '|' if !in_single && !in_double && !in_backtick => {
                if chars.peek() == Some(&'|') {
                    chars.next();
                }
                push_command(&mut commands, &mut current);
            }
            ';' if !in_single && !in_double && !in_backtick => {
                push_command(&mut commands, &mut current);
            }
            _ => current.push(ch),
        }
    }

    push_command(&mut commands, &mut current);
    commands
}

fn cached_regex<F>(key: String, build: F) -> Result<Regex, regex::Error>
where
    F: FnOnce() -> Result<Regex, regex::Error>,
{
    let cache = regex_cache();
    if let Some(regex) = cache
        .lock()
        .expect("regex cache poisoned")
        .entries
        .get(&key)
    {
        return Ok(regex.clone());
    }

    let regex = build()?;
    let mut cache = cache.lock().expect("regex cache poisoned");
    if cache.entries.len() >= REGEX_CACHE_MAX {
        if let Some(oldest) = cache.order.pop_front() {
            cache.entries.remove(&oldest);
        }
    }
    cache.order.push_back(key.clone());
    cache.entries.insert(key, regex.clone());

    Ok(regex)
}

fn escape_regex(value: &str) -> String {
    value.chars().map(escape_regex_char).collect()
}

fn escape_regex_char(ch: char) -> String {
    match ch {
        '.' | '+' | '?' | '^' | '$' | '{' | '}' | '(' | ')' | '|' | '[' | ']' | '\\' => {
            format!("\\{ch}")
        }
        _ => ch.to_string(),
    }
}

fn convert_glob_part(glob: &str) -> String {
    let mut result = String::new();
    for ch in glob.chars() {
        if ch == '*' {
            result.push_str(".*");
        } else {
            result.push_str(&escape_regex_char(ch));
        }
    }
    result
}

fn push_command(commands: &mut Vec<String>, current: &mut String) {
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        commands.push(trimmed.to_string());
    }
    current.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bash_pattern_valid() {
        assert_eq!(parse_bash_pattern("Bash(sudo *)"), Some("sudo *"));
    }

    #[test]
    fn test_parse_bash_pattern_invalid() {
        assert_eq!(parse_bash_pattern("Read(*)"), None);
        assert_eq!(parse_bash_pattern("Bash(sudo *"), None);
    }

    #[test]
    fn test_parse_tool_pattern() {
        assert_eq!(
            parse_tool_pattern("Read(**/*.rs)"),
            Some(("Read", "**/*.rs"))
        );
        assert_eq!(parse_tool_pattern("Bash(sudo *)"), Some(("Bash", "sudo *")));
        assert_eq!(parse_tool_pattern("Read(**/*.rs"), None);
    }

    #[test]
    fn test_glob_to_regex_colon_format() {
        let regex = glob_to_regex("tree:*", false).unwrap();

        assert!(regex.is_match("tree"));
        assert!(regex.is_match("tree src"));
        assert!(!regex.is_match("treehouse"));
    }

    #[test]
    fn test_glob_to_regex_space_format() {
        let regex = glob_to_regex("sudo *", false).unwrap();

        assert!(regex.is_match("sudo apt update"));
        assert!(!regex.is_match("sudo"));
        assert!(!regex.is_match("SUDO apt update"));
    }

    #[test]
    fn test_file_glob_to_regex_globstar() {
        let regex = file_glob_to_regex("src/**/*.rs", false).unwrap();

        assert!(regex.is_match("src/lib.rs"));
        assert!(regex.is_match("src/nested/mod.rs"));
        assert!(!regex.is_match("src/lib.ts"));
    }

    #[test]
    fn test_matches_any_pattern() {
        let patterns = vec!["Bash(tree:*)".to_string(), "Bash(sudo *)".to_string()];

        assert_eq!(
            matches_any_pattern("tree src", &patterns, false),
            Some("tree:*".to_string())
        );
        assert_eq!(matches_any_pattern("rm -rf target", &patterns, false), None);
    }

    #[test]
    fn test_split_chained_commands_simple() {
        assert_eq!(
            split_chained_commands("npm test && cargo test || echo fail; pwd | wc -l"),
            vec!["npm test", "cargo test", "echo fail", "pwd", "wc -l"]
        );
    }

    #[test]
    fn test_split_chained_commands_with_quotes() {
        assert_eq!(
            split_chained_commands("echo 'a && b' && echo \"c | d\"; echo `e || f`"),
            vec!["echo 'a && b'", "echo \"c | d\"", "echo `e || f`"]
        );
    }
}
