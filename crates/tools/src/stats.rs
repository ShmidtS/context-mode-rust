use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionStats {
    pub total_calls: i64,
    pub total_bytes: i64,
}

pub fn track_response(_tool: &str, _bytes: usize) {}
pub fn persist_stats() {}
