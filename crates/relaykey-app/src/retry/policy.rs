#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Total tries including the first. (2 = 1 retry)
    pub max_attempts: usize,
    pub base_backoff_ms: u64,
    pub max_backoff_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 2,
            base_backoff_ms: 50,
            max_backoff_ms: 300,
        }
    }
}