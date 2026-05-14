/// Classify process exit codes.
#[derive(Debug, Clone, PartialEq)]
pub enum ExitClassification {
    Success,
    Error,
    Timeout,
    Killed,
    Unknown(i32),
}

/// Classify an exit code into a semantic category.
pub fn classify_exit_code(code: i32) -> ExitClassification {
    match code {
        0 => ExitClassification::Success,
        137 => ExitClassification::Killed,  // SIGKILL
        143 => ExitClassification::Killed,  // SIGTERM
        124 => ExitClassification::Timeout, // timeout exit
        _ if code > 0 => ExitClassification::Error,
        _ => ExitClassification::Unknown(code),
    }
}

/// Check if an exit code indicates success.
pub fn is_success(code: i32) -> bool {
    code == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_success() {
        assert_eq!(classify_exit_code(0), ExitClassification::Success);
    }

    #[test]
    fn test_classify_error() {
        assert_eq!(classify_exit_code(1), ExitClassification::Error);
    }

    #[test]
    fn test_classify_killed() {
        assert_eq!(classify_exit_code(137), ExitClassification::Killed);
    }

    #[test]
    fn test_is_success() {
        assert!(is_success(0));
        assert!(!is_success(1));
    }
}
