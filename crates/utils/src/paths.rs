use std::path::PathBuf;

/// Return the user's home directory, falling back to the current directory.
pub fn home_or_current() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}
