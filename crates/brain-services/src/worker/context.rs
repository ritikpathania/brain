#![allow(missing_docs)]

use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct TaskExecutionContext {
    pub cancellation_token: CancellationToken,
    pub started_at: Instant,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_execution_context_creation() {
        let token = CancellationToken::new();
        let started_at = Instant::now();

        let ctx = TaskExecutionContext {
            cancellation_token: token.clone(),
            started_at,
        };

        assert!(!ctx.cancellation_token.is_cancelled());
        token.cancel();
        assert!(ctx.cancellation_token.is_cancelled());
    }
}
