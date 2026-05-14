use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Application lifecycle manager with graceful shutdown support.
pub struct Lifecycle {
    running: Arc<AtomicBool>,
}

impl Lifecycle {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn shutdown(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    pub fn running_clone(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifecycle() {
        let lc = Lifecycle::new();
        assert!(lc.is_running());
        lc.shutdown();
        assert!(!lc.is_running());
    }
}
