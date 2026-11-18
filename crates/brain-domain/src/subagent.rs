//! Subagent execution domain models, status transitions, and lifecycle invariants.

use crate::errors::DomainError;
use crate::events::DomainEvent;
use crate::identifiers::SessionId;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use ulid::Ulid;

/// Strongly-typed identifier for a subagent execution instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SubagentId(pub Ulid);

impl SubagentId {
    /// Generates a new unique `SubagentId`.
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for SubagentId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SubagentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SubagentId {
    type Err = ulid::DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ulid::from_string(s).map(Self)
    }
}

/// Lifecycle status for a subagent execution instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStatus {
    /// Initial registered state before initialization starts.
    Created,
    /// Preparing worktree, sandbox environment, or context fork.
    Starting,
    /// Active execution in progress.
    Running,
    /// Successful terminal completion.
    Completed,
    /// Terminal failure with error details.
    Failed,
    /// Terminal cancellation by parent or timeout.
    Cancelled,
}

impl fmt::Display for SubagentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Errors raised during subagent state machine transitions.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SubagentTransitionError {
    /// Terminal states are immutable and cannot transition back to active or any other state.
    #[error("terminal subagent state {from:?} is immutable and cannot transition to {to:?}")]
    TerminalStateImmutable {
        /// Current terminal state.
        from: SubagentStatus,
        /// Attempted target state.
        to: SubagentStatus,
    },
    /// Attempted state transition violates the lifecycle DAG.
    #[error("invalid subagent transition from {from:?} to {to:?}")]
    InvalidTransition {
        /// Current status.
        from: SubagentStatus,
        /// Target status.
        to: SubagentStatus,
    },
}

impl SubagentStatus {
    /// Returns true if this state is terminal (`Completed`, `Failed`, or `Cancelled`).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Checks whether transitioning from `self` to `target` is allowed.
    pub fn can_transition_to(&self, target: &SubagentStatus) -> bool {
        if self.is_terminal() {
            return false;
        }
        match (self, target) {
            (Self::Created, Self::Starting) => true,
            (Self::Created, Self::Cancelled) => true,
            (Self::Starting, Self::Running) => true,
            (Self::Starting, Self::Failed) => true,
            (Self::Starting, Self::Cancelled) => true,
            (Self::Running, Self::Running) => true, // heartbeat / progress in running state
            (Self::Running, Self::Completed) => true,
            (Self::Running, Self::Failed) => true,
            (Self::Running, Self::Cancelled) => true,
            _ => false,
        }
    }

    /// Evaluates and transitions the status, enforcing terminal state immutability.
    pub fn transition_to(&mut self, target: SubagentStatus) -> Result<(), SubagentTransitionError> {
        if self.is_terminal() {
            return Err(SubagentTransitionError::TerminalStateImmutable {
                from: *self,
                to: target,
            });
        }
        if !self.can_transition_to(&target) {
            return Err(SubagentTransitionError::InvalidTransition {
                from: *self,
                to: target,
            });
        }
        *self = target;
        Ok(())
    }
}

/// Execution kind distinguishing named agents from conversation forks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubagentKind {
    /// Specialized named agent starting with a fresh conversation context.
    #[default]
    Named,
    /// Conversation fork inheriting parent conversation history.
    Fork,
}

/// Execution mode distinguishing synchronous foreground execution from asynchronous background tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubagentExecutionMode {
    /// Synchronously blocks the parent turn/tool until subagent completion.
    #[default]
    Foreground,
    /// Asynchronously runs in the background and registers as a Fleet task.
    Background,
}

/// Domain aggregate representing a delegated subagent execution instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subagent {
    /// Unique subagent identifier.
    pub id: SubagentId,
    /// Parent session identifier.
    pub parent_session_id: SessionId,
    /// Dedicated child session identifier.
    pub subagent_session_id: SessionId,
    /// Subagent role classification (e.g. "researcher", "coder", "reviewer").
    pub role: String,
    /// Subagent type classification (e.g. "general", "task", "worktree").
    pub agent_type: String,
    /// Execution kind (Named fresh context vs. Fork inheriting parent context).
    pub kind: SubagentKind,
    /// Execution mode (Foreground blocking vs. Background asynchronous).
    pub execution_mode: SubagentExecutionMode,
    /// Current lifecycle status.
    pub status: SubagentStatus,
    /// Delegation recursion depth (root child = 1).
    pub depth: u32,
    /// Creation timestamp in milliseconds since epoch.
    pub created_at_ms: u64,
    /// Last status update timestamp in milliseconds since epoch.
    pub updated_at_ms: u64,
    /// Completion result message when completed.
    pub result: Option<String>,
    /// Error message when failed.
    pub error: Option<String>,
    /// Isolated worktree working directory if assigned.
    pub working_dir: Option<String>,
    /// Staged pure domain events awaiting publication.
    #[serde(skip)]
    pub staged_events: Vec<DomainEvent>,
}

impl Subagent {
    /// Maximum allowed subagent delegation depth.
    pub const MAX_DEPTH: u32 = 3;

    /// Instantiates a new subagent in the `Created` status.
    pub fn new(
        id: SubagentId,
        parent_session_id: SessionId,
        subagent_session_id: SessionId,
        role: impl Into<String>,
        agent_type: impl Into<String>,
        depth: u32,
        timestamp_ms: u64,
    ) -> Result<Self, DomainError> {
        if depth > Self::MAX_DEPTH {
            return Err(DomainError::ValidationError {
                message: format!(
                    "Subagent delegation depth {depth} exceeds anti-recursion guardrail limit of {}",
                    Self::MAX_DEPTH
                ),
                rule_id: Some("SUBAGENT-01-RECURSION-LIMIT".to_string()),
            });
        }

        Ok(Self {
            id,
            parent_session_id,
            subagent_session_id,
            role: role.into(),
            agent_type: agent_type.into(),
            kind: SubagentKind::Named,
            execution_mode: SubagentExecutionMode::Foreground,
            status: SubagentStatus::Created,
            depth,
            created_at_ms: timestamp_ms,
            updated_at_ms: timestamp_ms,
            result: None,
            error: None,
            working_dir: None,
            staged_events: Vec::new(),
        })
    }

    /// Sets the execution kind (Named fresh context vs. Fork parent context).
    pub fn with_kind(mut self, kind: SubagentKind) -> Self {
        self.kind = kind;
        self
    }

    /// Sets the execution mode (Foreground synchronous vs. Background asynchronous).
    pub fn with_execution_mode(mut self, mode: SubagentExecutionMode) -> Self {
        self.execution_mode = mode;
        self
    }

    /// Sets the isolated working directory for this subagent.
    pub fn set_working_dir(&mut self, dir: impl Into<String>) {
        self.working_dir = Some(dir.into());
    }

    /// Transitions subagent into `Running` status and stages `SubagentStarted`.
    pub fn start(&mut self, timestamp_ms: u64) -> Result<DomainEvent, SubagentTransitionError> {
        // Allow transition from Created -> Starting -> Running or directly Created -> Running
        if self.status == SubagentStatus::Created {
            self.status.transition_to(SubagentStatus::Starting)?;
        }
        self.status.transition_to(SubagentStatus::Running)?;
        self.updated_at_ms = timestamp_ms;

        let event = DomainEvent::SubagentStarted {
            subagent_id: self.id,
            parent_session_id: self.parent_session_id,
            subagent_session_id: self.subagent_session_id,
            role: self.role.clone(),
            started_at: timestamp_ms,
        };
        self.staged_events.push(event.clone());
        Ok(event)
    }

    /// Records progress while in `Running` status and stages `SubagentProgressed`.
    pub fn progress(
        &mut self,
        message: impl Into<String>,
        timestamp_ms: u64,
    ) -> Result<DomainEvent, SubagentTransitionError> {
        if self.status.is_terminal() {
            return Err(SubagentTransitionError::TerminalStateImmutable {
                from: self.status,
                to: SubagentStatus::Running,
            });
        }
        if self.status != SubagentStatus::Running {
            return Err(SubagentTransitionError::InvalidTransition {
                from: self.status,
                to: SubagentStatus::Running,
            });
        }
        self.updated_at_ms = timestamp_ms;
        let msg = message.into();
        let event = DomainEvent::SubagentProgressed {
            subagent_id: self.id,
            progress_message: msg,
            timestamp: timestamp_ms,
        };
        self.staged_events.push(event.clone());
        Ok(event)
    }

    /// Transitions subagent to `Completed` status and stages `SubagentCompleted`.
    pub fn complete(
        &mut self,
        result: impl Into<String>,
        timestamp_ms: u64,
    ) -> Result<DomainEvent, SubagentTransitionError> {
        self.status.transition_to(SubagentStatus::Completed)?;
        let res = result.into();
        self.result = Some(res.clone());
        self.updated_at_ms = timestamp_ms;

        let event = DomainEvent::SubagentCompleted {
            subagent_id: self.id,
            result: res,
            completed_at: timestamp_ms,
        };
        self.staged_events.push(event.clone());
        Ok(event)
    }

    /// Transitions subagent to `Failed` status and stages `SubagentFailed`.
    pub fn fail(
        &mut self,
        error: impl Into<String>,
        timestamp_ms: u64,
    ) -> Result<DomainEvent, SubagentTransitionError> {
        self.status.transition_to(SubagentStatus::Failed)?;
        let err = error.into();
        self.error = Some(err.clone());
        self.updated_at_ms = timestamp_ms;

        let event = DomainEvent::SubagentFailed {
            subagent_id: self.id,
            error: err,
            failed_at: timestamp_ms,
        };
        self.staged_events.push(event.clone());
        Ok(event)
    }

    /// Transitions subagent to `Cancelled` status and stages `SubagentCancelled`.
    pub fn cancel(
        &mut self,
        reason: impl Into<String>,
        timestamp_ms: u64,
    ) -> Result<DomainEvent, SubagentTransitionError> {
        self.status.transition_to(SubagentStatus::Cancelled)?;
        let r = reason.into();
        self.result = Some(r.clone());
        self.updated_at_ms = timestamp_ms;

        let event = DomainEvent::SubagentCancelled {
            subagent_id: self.id,
            reason: r,
            cancelled_at: timestamp_ms,
        };
        self.staged_events.push(event.clone());
        Ok(event)
    }

    /// Drains and returns all staged domain events.
    pub fn take_events(&mut self) -> Vec<DomainEvent> {
        std::mem::take(&mut self.staged_events)
    }
}
