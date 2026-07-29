//! Configurable `RetryPolicy`, `RetryClassifier` trait, and `BackoffStrategy` schedule calculation (Phase 7 Milestone 7.5).
//!
//! ### Architectural Invariants:
//! 1. Separation of Concerns: `RetryClassifier` decides IF a retry should occur; `BackoffStrategy` calculates WHEN (delay).
//! 2. `TaskExecutionRuntime` consumes `RetryPolicy` without hardcoding retry rules into execution loops.

use crate::planning::execution_runtime::{ExecutionFailure, ExecutionFailureKind};
use serde::{Deserialize, Serialize};

/// Strategy calculating backoff delay in milliseconds for retried task step execution attempts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum BackoffStrategy {
    /// Retry immediately without delay.
    #[default]
    Immediate,
    /// Fixed delay in milliseconds between retries.
    FixedMs(u64),
    /// Exponential backoff delay with multiplier and upper bound cap.
    Exponential {
        /// Initial delay in milliseconds.
        initial_ms: u64,
        /// Growth multiplier (e.g. 2.0).
        multiplier: f32,
        /// Maximum delay upper bound in milliseconds.
        max_ms: u64,
    },
}

impl BackoffStrategy {
    /// Calculates the delay in milliseconds for a given 1-based attempt index.
    pub fn calculate_delay_ms(&self, attempt: u32) -> u64 {
        match self {
            Self::Immediate => 0,
            Self::FixedMs(ms) => *ms,
            Self::Exponential {
                initial_ms,
                multiplier,
                max_ms,
            } => {
                if attempt <= 1 {
                    *initial_ms
                } else {
                    let delay =
                        (*initial_ms as f64) * (multiplier.powi((attempt - 1) as i32) as f64);
                    (delay as u64).min(*max_ms)
                }
            }
        }
    }
}

/// Trait evaluating whether an `ExecutionFailure` is retryable.
pub trait RetryClassifier: Send + Sync {
    /// Evaluates if a task step failure should trigger a retry attempt.
    fn should_retry(&self, failure: &ExecutionFailure, current_attempt: u32) -> bool;
}

/// Default classifier retrying transient task step failures (`TaskFailure`, `Timeout`) up to `max_attempts`.
#[derive(Debug, Clone)]
pub struct DefaultRetryClassifier {
    /// Maximum allowed total attempts (1 initial + retries).
    pub max_attempts: u32,
}

impl Default for DefaultRetryClassifier {
    fn default() -> Self {
        Self { max_attempts: 3 }
    }
}

impl RetryClassifier for DefaultRetryClassifier {
    fn should_retry(&self, failure: &ExecutionFailure, current_attempt: u32) -> bool {
        if current_attempt >= self.max_attempts {
            return false;
        }

        matches!(
            failure.kind,
            ExecutionFailureKind::TaskFailure | ExecutionFailureKind::Timeout
        )
    }
}

/// Policy combining a `RetryClassifier` and `BackoffStrategy`.
pub struct RetryPolicy {
    /// Classifier evaluating retry eligibility.
    pub classifier: Box<dyn RetryClassifier>,
    /// Backoff strategy calculating retry delay schedule.
    pub backoff: BackoffStrategy,
    /// Maximum total attempts bound.
    pub max_attempts: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            classifier: Box::new(DefaultRetryClassifier::default()),
            backoff: BackoffStrategy::Immediate,
            max_attempts: 3,
        }
    }
}

impl RetryPolicy {
    /// Instantiates a new `RetryPolicy`.
    pub fn new(
        classifier: Box<dyn RetryClassifier>,
        backoff: BackoffStrategy,
        max_attempts: u32,
    ) -> Self {
        Self {
            classifier,
            backoff,
            max_attempts,
        }
    }

    /// Evaluates whether a retry should be executed for the specified failure and attempt count.
    pub fn should_retry(&self, failure: &ExecutionFailure, attempt: u32) -> bool {
        self.classifier.should_retry(failure, attempt)
    }

    /// Calculates backoff delay for the specified attempt count.
    pub fn delay_ms(&self, attempt: u32) -> u64 {
        self.backoff.calculate_delay_ms(attempt)
    }
}
