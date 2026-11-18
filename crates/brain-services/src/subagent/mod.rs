//! Subagent Execution Engine
//!
//! Coordinates the lifecycle, bounded concurrency, cancellation propagation,
//! conversation context isolation, and Fleet background integration for subagents.

use crate::worktree::{WorktreeDescriptor, WorktreeService};
use brain_core::model::{
    ChatRole, GenerationChunk, GenerationRequest, MessageContentBlock, ModelChatMessage,
    ModelProvider, ToolDefinition,
};
use brain_domain::{
    DomainError, FleetJobSummary, MessageId, Session, SessionId, SessionTimestamp, SessionTitle,
    Subagent, SubagentExecutionMode, SubagentId, SubagentKind, SubagentStatus,
    SubagentTransitionError,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

/// Default maximum concurrent active subagents (Claude Code v2.1.302 parity: 4).
pub const DEFAULT_MAX_CONCURRENT_SUBAGENTS: usize = 4;
/// Hard ceiling for subagent concurrency.
pub const MAX_CONCURRENT_SUBAGENTS_CEILING: usize = 8;
/// Default execution timeout watchdog duration (60 seconds).
pub const DEFAULT_SUBAGENT_TIMEOUT: Duration = Duration::from_secs(60);

/// Errors encountered during subagent orchestration and execution.
#[derive(Debug, thiserror::Error)]
pub enum SubagentError {
    /// Recursion guardrail violation: subagent depth exceeds limit.
    #[error("Subagent delegation depth {depth} exceeds anti-recursion limit {max_depth}")]
    RecursionLimitExceeded {
        /// Attempted depth.
        depth: u32,
        /// Maximum allowed depth.
        max_depth: u32,
    },

    /// Active concurrency ceiling reached.
    #[error("Maximum concurrent subagent limit reached ({limit})")]
    ConcurrencyLimitReached {
        /// Configured limit.
        limit: usize,
    },

    /// Subagent not found in active registry.
    #[error("Subagent '{0}' not found")]
    NotFound(SubagentId),

    /// Domain transition error (e.g. terminal state immutability).
    #[error("Subagent transition error: {0}")]
    TransitionError(#[from] SubagentTransitionError),

    /// Underlying domain error.
    #[error("Domain error: {0}")]
    DomainError(#[from] DomainError),

    /// Worktree service error.
    #[error("Worktree error: {0}")]
    WorktreeError(String),

    /// Subagent execution timed out.
    #[error("Subagent '{0}' execution timed out")]
    Timeout(SubagentId),

    /// Execution cancelled.
    #[error("Subagent '{0}' execution was cancelled: {1}")]
    Cancelled(SubagentId, String),
}

/// Request parameters to spawn a new subagent.
#[derive(Debug, Clone)]
pub struct SubagentSpawnRequest {
    /// Authoritative parent session identifier.
    pub parent_session_id: SessionId,
    /// Subagent role classification (e.g. "researcher", "coder", "reviewer").
    pub role: String,
    /// Subagent type classification (e.g. "task", "worktree", "general").
    pub agent_type: String,
    /// Objective prompt or task description.
    pub prompt: String,
    /// Current delegation depth (root caller child = 1).
    pub depth: u32,
    /// Optional custom system prompt for the subagent.
    pub system_prompt: Option<String>,
    /// Optional execution timeout duration.
    pub timeout: Option<Duration>,
    /// Whether to isolate this subagent in a dedicated Git worktree.
    pub use_worktree: bool,
    /// Optional custom worktree name.
    pub worktree_name: Option<String>,
    /// Optional subagent kind (Named or Fork).
    pub kind: Option<SubagentKind>,
    /// Optional execution mode (Foreground or Background).
    pub execution_mode: Option<SubagentExecutionMode>,
}

/// In-memory runtime handle tracking an active subagent instance.
pub struct ActiveSubagentHandle {
    /// Domain aggregate holding state and staged events.
    pub subagent: Subagent,
    /// Subagent objective prompt.
    pub prompt: String,
    /// Optional custom system prompt instructions.
    pub system_prompt: Option<String>,
    /// Cooperative cancellation token.
    pub cancellation_token: CancellationToken,
    /// Watchdog deadline instant.
    pub deadline: Instant,
    /// Assigned worktree descriptor if worktree isolation was used.
    pub worktree: Option<WorktreeDescriptor>,
    /// Instant when execution started.
    pub started_at_instant: Option<Instant>,
    /// Acquired concurrency permit released upon terminal state transition.
    pub permit: Option<OwnedSemaphorePermit>,
}

/// Subagent Execution Service coordinating lifecycle, isolation, and concurrency.
pub struct SubagentService {
    active: Arc<RwLock<HashMap<SubagentId, ActiveSubagentHandle>>>,
    semaphore: Arc<Semaphore>,
    worktree_service: Option<Arc<WorktreeService>>,
    max_concurrency: usize,
}

impl SubagentService {
    /// Instantiates a new `SubagentService` with bounded concurrency.
    pub fn new(max_concurrent: usize, worktree_service: Option<Arc<WorktreeService>>) -> Self {
        let concurrency = max_concurrent.clamp(1, MAX_CONCURRENT_SUBAGENTS_CEILING);
        Self {
            active: Arc::new(RwLock::new(HashMap::new())),
            semaphore: Arc::new(Semaphore::new(concurrency)),
            worktree_service,
            max_concurrency: concurrency,
        }
    }

    /// Spawns a new subagent with stable identities, parent-child linking, and isolation.
    pub fn spawn(
        &self,
        request: SubagentSpawnRequest,
        parent_cancel: Option<&CancellationToken>,
    ) -> Result<SubagentId, SubagentError> {
        // 1. Anti-recursion guardrail check
        if request.depth > Subagent::MAX_DEPTH {
            return Err(SubagentError::RecursionLimitExceeded {
                depth: request.depth,
                max_depth: Subagent::MAX_DEPTH,
            });
        }

        // 2. Concurrency limit check: acquire permit
        let permit = self.semaphore.clone().try_acquire_owned().map_err(|_| {
            SubagentError::ConcurrencyLimitReached {
                limit: self.max_concurrency,
            }
        })?;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let subagent_id = SubagentId::new();
        let subagent_session_id = SessionId::new();

        let mut subagent = Subagent::new(
            subagent_id,
            request.parent_session_id,
            subagent_session_id,
            request.role.clone(),
            request.agent_type.clone(),
            request.depth,
            now_ms,
        )?;

        if let Some(kind) = request.kind {
            subagent = subagent.with_kind(kind);
        }
        if let Some(mode) = request.execution_mode {
            subagent = subagent.with_execution_mode(mode);
        }

        // 3. Optional Worktree Isolation
        let worktree_desc = if request.use_worktree {
            if let Some(ref wt_service) = self.worktree_service {
                let wt_name = request.worktree_name.unwrap_or_else(|| {
                    format!(
                        "{}-{}",
                        request.role,
                        subagent_id.to_string().to_lowercase()
                    )
                });
                let desc = wt_service
                    .create_worktree(&wt_name, Some(subagent_id))
                    .map_err(|e| SubagentError::WorktreeError(e.to_string()))?;
                subagent.set_working_dir(desc.path.to_string_lossy());
                Some(desc)
            } else {
                None
            }
        } else {
            None
        };

        // 4. Cancellation token propagation (parent cancellation cascades to child)
        let child_cancel = match parent_cancel {
            Some(parent) => parent.child_token(),
            None => CancellationToken::new(),
        };

        let timeout = request.timeout.unwrap_or(DEFAULT_SUBAGENT_TIMEOUT);
        let deadline = Instant::now() + timeout;

        let handle = ActiveSubagentHandle {
            subagent,
            prompt: request.prompt,
            system_prompt: request.system_prompt,
            cancellation_token: child_cancel,
            deadline,
            worktree: worktree_desc,
            started_at_instant: None,
            permit: Some(permit),
        };

        let mut map = self.active.write().unwrap();
        map.insert(subagent_id, handle);

        Ok(subagent_id)
    }

    /// Marks a subagent as started and transitioning into `Running`.
    pub fn start(&self, subagent_id: &SubagentId) -> Result<(), SubagentError> {
        let mut map = self.active.write().unwrap();
        let handle = map
            .get_mut(subagent_id)
            .ok_or(SubagentError::NotFound(*subagent_id))?;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        handle.subagent.start(now_ms)?;
        handle.started_at_instant = Some(Instant::now());
        Ok(())
    }

    /// Records a progress message from the subagent.
    pub fn progress(&self, subagent_id: &SubagentId, message: &str) -> Result<(), SubagentError> {
        let mut map = self.active.write().unwrap();
        let handle = map
            .get_mut(subagent_id)
            .ok_or(SubagentError::NotFound(*subagent_id))?;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        handle.subagent.progress(message, now_ms)?;
        Ok(())
    }

    /// Completes the subagent execution with a successful result.
    pub fn complete(&self, subagent_id: &SubagentId, result: &str) -> Result<(), SubagentError> {
        let mut map = self.active.write().unwrap();
        let handle = map
            .get_mut(subagent_id)
            .ok_or(SubagentError::NotFound(*subagent_id))?;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        handle.subagent.complete(result, now_ms)?;
        handle.permit.take();
        Ok(())
    }

    /// Marks the subagent execution as failed with an error message.
    pub fn fail(&self, subagent_id: &SubagentId, error: &str) -> Result<(), SubagentError> {
        let mut map = self.active.write().unwrap();
        let handle = map
            .get_mut(subagent_id)
            .ok_or(SubagentError::NotFound(*subagent_id))?;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        handle.subagent.fail(error, now_ms)?;
        handle.permit.take();
        Ok(())
    }

    /// Cancels a subagent, notifying its cancellation token and updating its status.
    pub fn cancel(&self, subagent_id: &SubagentId, reason: &str) -> Result<(), SubagentError> {
        let mut map = self.active.write().unwrap();
        let handle = map
            .get_mut(subagent_id)
            .ok_or(SubagentError::NotFound(*subagent_id))?;

        handle.cancellation_token.cancel();

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        handle.subagent.cancel(reason, now_ms)?;
        handle.permit.take();
        Ok(())
    }

    /// Cascades cancellation to all active child subagents of a given parent session.
    pub fn cancel_children_of(&self, parent_session_id: &SessionId, reason: &str) -> usize {
        let mut map = self.active.write().unwrap();
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut count = 0;
        for handle in map.values_mut() {
            if handle.subagent.parent_session_id == *parent_session_id
                && !handle.subagent.status.is_terminal()
            {
                handle.cancellation_token.cancel();
                if handle.subagent.cancel(reason, now_ms).is_ok() {
                    handle.permit.take();
                    count += 1;
                }
            }
        }
        count
    }

    /// Retrieves a clone of the subagent domain model.
    pub fn get_subagent(&self, subagent_id: &SubagentId) -> Option<Subagent> {
        let map = self.active.read().unwrap();
        map.get(subagent_id).map(|h| h.subagent.clone())
    }

    /// Retrieves the cancellation token for a subagent.
    pub fn get_cancellation_token(&self, subagent_id: &SubagentId) -> Option<CancellationToken> {
        let map = self.active.read().unwrap();
        map.get(subagent_id).map(|h| h.cancellation_token.clone())
    }

    /// Lists all subagents for a specific parent session.
    pub fn list_for_parent(&self, parent_session_id: &SessionId) -> Vec<Subagent> {
        let map = self.active.read().unwrap();
        map.values()
            .filter(|h| h.subagent.parent_session_id == *parent_session_id)
            .map(|h| h.subagent.clone())
            .collect()
    }

    /// Lists all registered subagents.
    pub fn list_all(&self) -> Vec<Subagent> {
        let map = self.active.read().unwrap();
        map.values().map(|h| h.subagent.clone()).collect()
    }

    /// Projects all subagents into `FleetJobSummary` for Fleet view integration.
    pub fn to_fleet_jobs(&self) -> Vec<FleetJobSummary> {
        let map = self.active.read().unwrap();
        map.values()
            .map(|h| {
                let s = &h.subagent;
                let status_str = match s.status {
                    SubagentStatus::Created => "pending",
                    SubagentStatus::Starting => "pending",
                    SubagentStatus::Running => "running",
                    SubagentStatus::Completed => "completed",
                    SubagentStatus::Failed => "failed",
                    SubagentStatus::Cancelled => "cancelled",
                };

                let cwd = s
                    .working_dir
                    .clone()
                    .unwrap_or_else(|| "default".to_string());

                let subtext = if let Some(ref res) = s.result {
                    res.clone()
                } else if let Some(ref err) = s.error {
                    err.clone()
                } else {
                    h.prompt.chars().take(80).collect()
                };

                FleetJobSummary {
                    id: s.id.to_string(),
                    kind: format!("subagent:{}", s.role),
                    label: format!("@{} subagent", s.role),
                    title: format!("Subagent [{}]", s.role),
                    subtext,
                    status: status_str.to_string(),
                    started_at_ms: s.created_at_ms,
                    cwd,
                    group: Some(s.agent_type.clone()),
                    pinned: false,
                }
            })
            .collect()
    }

    /// Inherits parent conversation context up to `at_message_id` into an isolated child session.
    ///
    /// Preserves parent immutability: the parent session is never modified.
    pub fn fork_parent_session(
        &self,
        parent: &Session,
        at_message_id: Option<&MessageId>,
        child_title: &str,
    ) -> Session {
        let new_id = SessionId::new();
        let new_title = SessionTitle(child_title.to_string());
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        parent.fork(new_id, new_title, at_message_id, SessionTimestamp(now))
    }

    /// Executes a real subagent turn using a model provider and tool execution.
    ///
    /// The runtime loop:
    /// Parent -> TaskTool -> SubagentService -> Model Provider -> Tool Execution -> Result Persistence -> Parent
    pub async fn execute_subagent<M, T>(
        &self,
        subagent_id: &SubagentId,
        model_provider: Arc<M>,
        model_name: &str,
        tool: Option<Arc<T>>,
    ) -> Result<String, SubagentError>
    where
        M: ModelProvider + ?Sized + 'static,
        T: brain_core::extensibility::Tool + ?Sized + 'static,
    {
        // 1. Transition to Starting -> Running
        self.start(subagent_id)?;

        let (prompt, cancel_token, system_prompt, working_dir, parent_session_id) = {
            let map = self.active.read().unwrap();
            let handle = map
                .get(subagent_id)
                .ok_or(SubagentError::NotFound(*subagent_id))?;
            (
                handle.prompt.clone(),
                handle.cancellation_token.clone(),
                handle.system_prompt.clone(),
                handle
                    .subagent
                    .working_dir
                    .clone()
                    .unwrap_or_else(|| ".".to_string()),
                handle.subagent.parent_session_id,
            )
        };

        // 2. Prepare tool definitions if available
        let tool_definitions = if let Some(ref t) = tool {
            let meta = t.metadata();
            vec![ToolDefinition {
                name: meta.name.clone(),
                description: meta.description.clone(),
                parameters: meta
                    .input_schema
                    .clone()
                    .unwrap_or_else(|| serde_json::json!({})),
            }]
        } else {
            Vec::new()
        };

        let request = GenerationRequest {
            model: model_name.to_string(),
            messages: vec![ModelChatMessage::text(ChatRole::User, prompt.clone())],
            system_prompt,
            tools: tool_definitions,
            thinking_budget: None,
        };

        // 3. Invoke Model Provider with cancellation support
        let mut stream = model_provider
            .stream_generation(request, cancel_token.clone())
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                let _ = self.fail(subagent_id, &err_str);
                SubagentError::DomainError(brain_domain::DomainError::ValidationError {
                    message: err_str,
                    rule_id: None,
                })
            })?;

        let mut accumulated_text = String::new();
        let mut tool_calls = Vec::new();

        use futures::StreamExt;
        while let Some(chunk_res) = stream.next().await {
            if cancel_token.is_cancelled() {
                let _ = self.cancel(subagent_id, "User requested cancellation");
                return Err(SubagentError::Cancelled(
                    *subagent_id,
                    "User requested cancellation".to_string(),
                ));
            }

            match chunk_res {
                Ok(GenerationChunk::TextDelta { text }) => {
                    accumulated_text.push_str(&text);
                }
                Ok(GenerationChunk::ToolUse { id, name, input }) => {
                    tool_calls.push((id, name, input));
                }
                Ok(GenerationChunk::Completed { .. }) => {}
                Ok(_) => {}
                Err(e) => {
                    let err_str = e.to_string();
                    let _ = self.fail(subagent_id, &err_str);
                    return Err(SubagentError::DomainError(
                        brain_domain::DomainError::ValidationError {
                            message: err_str,
                            rule_id: None,
                        },
                    ));
                }
            }
        }

        // 4. If the model invoked a tool call, execute it and feed result back to the model
        if !tool_calls.is_empty() {
            if let Some(ref t) = tool {
                for (tool_call_id, tool_name, tool_input) in tool_calls {
                    if cancel_token.is_cancelled() {
                        let _ = self.cancel(subagent_id, "Cancelled before tool execution");
                        return Err(SubagentError::Cancelled(
                            *subagent_id,
                            "Cancelled before tool execution".to_string(),
                        ));
                    }

                    let _ = self.progress(subagent_id, &format!("Executing tool {tool_name}"));

                    let ctx = brain_core::extensibility::ExecutionContext {
                        session_id: parent_session_id,
                        working_dir: std::path::PathBuf::from(working_dir.clone()),
                        cancellation: Arc::new(brain_tools::CancellationTokenImpl::new()),
                        deadline: None,
                    };
                    let args: HashMap<String, serde_json::Value> =
                        if let Some(obj) = tool_input.as_object() {
                            obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
                        } else {
                            HashMap::new()
                        };

                    let tool_result = match t.execute(&ctx, &args) {
                        Ok(res) => res.value().to_string(),
                        Err(e) => format!("Error executing {tool_name}: {e}"),
                    };

                    // Follow-up model call with tool result
                    let follow_up = GenerationRequest {
                        model: model_name.to_string(),
                        messages: vec![
                            ModelChatMessage::text(ChatRole::User, prompt.clone()),
                            ModelChatMessage {
                                role: ChatRole::Assistant,
                                content: vec![MessageContentBlock::ToolUse {
                                    id: tool_call_id.clone(),
                                    name: tool_name.clone(),
                                    input: tool_input,
                                }],
                            },
                            ModelChatMessage {
                                role: ChatRole::User,
                                content: vec![MessageContentBlock::ToolResult {
                                    tool_use_id: tool_call_id,
                                    content: tool_result,
                                    is_error: false,
                                }],
                            },
                        ],
                        system_prompt: None,
                        tools: Vec::new(),
                        thinking_budget: None,
                    };

                    let mut follow_up_stream = model_provider
                        .stream_generation(follow_up, cancel_token.clone())
                        .await
                        .map_err(|e| {
                            let err_str = e.to_string();
                            let _ = self.fail(subagent_id, &err_str);
                            SubagentError::DomainError(brain_domain::DomainError::ValidationError {
                                message: err_str,
                                rule_id: None,
                            })
                        })?;

                    while let Some(chunk_res) = follow_up_stream.next().await {
                        if cancel_token.is_cancelled() {
                            let _ = self.cancel(subagent_id, "Cancelled during follow-up");
                            return Err(SubagentError::Cancelled(
                                *subagent_id,
                                "Cancelled during follow-up".to_string(),
                            ));
                        }
                        if let Ok(GenerationChunk::TextDelta { text }) = chunk_res {
                            accumulated_text.push_str(&text);
                        }
                    }
                }
            }
        }

        // 5. Complete subagent with terminal result
        let final_result = if accumulated_text.is_empty() {
            "Subagent task completed successfully with no additional output.".to_string()
        } else {
            accumulated_text
        };

        self.complete(subagent_id, &final_result)?;
        Ok(final_result)
    }
}
