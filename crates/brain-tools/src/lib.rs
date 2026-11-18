use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use brain_core::errors::BrainError;
use brain_core::extensibility::{
    CancellationToken as CoreCancellationToken, ExecutionContext, ExecutionResult, Permission,
    Tool, ToolRegistry, ToolRunner,
};

pub mod sandbox;

/// Concrete Tokio-backed implementation of the core CancellationToken trait.
#[derive(Clone)]
pub struct CancellationTokenImpl {
    cancelled: Arc<AtomicBool>,
    notify: Arc<tokio::sync::Notify>,
}

impl Default for CancellationTokenImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationTokenImpl {
    /// Creates a new CancellationTokenImpl.
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Triggers cancellation and notifies waiters.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }
}

impl CoreCancellationToken for CancellationTokenImpl {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl std::fmt::Debug for CancellationTokenImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancellationTokenImpl")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

/// Central permission validation manager.
pub struct PermissionManager {
    granted: RwLock<HashSet<Permission>>,
}

impl Clone for PermissionManager {
    fn clone(&self) -> Self {
        let granted = self.granted.read().unwrap().clone();
        Self {
            granted: RwLock::new(granted),
        }
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionManager {
    /// Creates a new PermissionManager.
    pub fn new() -> Self {
        Self {
            granted: RwLock::new(HashSet::new()),
        }
    }

    /// Creates an independent scoped permission manager containing the union of
    /// this manager's permissions and any newly granted permissions.
    pub fn scoped_with(&self, additional: impl IntoIterator<Item = Permission>) -> Self {
        let mut set = self.granted.read().unwrap().clone();
        set.extend(additional);
        Self {
            granted: RwLock::new(set),
        }
    }

    /// Grants a permission scope to the session.
    pub fn grant(&self, permission: Permission) {
        self.granted.write().unwrap().insert(permission);
    }

    /// Returns true if the requested permission scope is granted.
    pub fn is_granted(&self, permission: Permission) -> bool {
        self.granted.read().unwrap().contains(&permission)
    }

    /// Validates if all required permissions for a tool are granted.
    pub fn validate_tool_permissions(&self, tool: &dyn Tool) -> Result<(), BrainError> {
        let meta = tool.metadata();
        for perm in &meta.required_permissions {
            if !self.is_granted(*perm) {
                return Err(BrainError::Authorization {
                    message: format!(
                        "Permission '{:?}' is required to execute tool '{}'",
                        perm, meta.name
                    ),
                });
            }
        }
        Ok(())
    }
}

/// Thread-safe in-memory Tool Registry implementation using a BTreeMap to guarantee sorting.
pub struct ToolRegistryImpl {
    tools: RwLock<BTreeMap<String, Arc<dyn Tool>>>,
}

impl Default for ToolRegistryImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistryImpl {
    /// Creates a new ToolRegistryImpl.
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(BTreeMap::new()),
        }
    }
}

impl ToolRegistry for ToolRegistryImpl {
    fn register_tool(&self, tool: Arc<dyn Tool>) -> Result<(), BrainError> {
        let name = tool.metadata().name.clone();
        let mut tools = self.tools.write().unwrap();
        if tools.contains_key(&name) {
            return Err(BrainError::InvalidTransition {
                message: format!("Tool '{name}' is already registered"),
            });
        }
        tools.insert(name, tool);
        Ok(())
    }

    fn get_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.read().unwrap().get(name).cloned()
    }

    fn list_tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.read().unwrap().values().cloned().collect()
    }
}

/// A policy-free tool runner that executes tools synchronously on the calling thread.
pub struct BlockingToolRunner;

impl ToolRunner for BlockingToolRunner {
    fn run(
        &self,
        tool: Arc<dyn Tool>,
        context: &ExecutionContext,
        arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult, BrainError> {
        // Enforce the policy-free invariant: simply delegate directly to the tool.
        tool.execute(context, arguments)
    }
}

/// Orchestration coordinator responsible for tool execution.
/// Enforces permissions, timeouts, and cancellation, offloading the actual execution
/// to the injected runner.
pub struct ToolExecutor {
    runner: Arc<dyn ToolRunner>,
}

impl ToolExecutor {
    /// Creates a new ToolExecutor with the given runner strategy.
    pub fn new(runner: Arc<dyn ToolRunner>) -> Self {
        Self { runner }
    }

    /// Executes a tool, coordinating all policies.
    pub async fn execute(
        &self,
        tool: Arc<dyn Tool>,
        context: &ExecutionContext,
        permission_manager: &PermissionManager,
        arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult, BrainError> {
        // 1. Enforce permission checks
        permission_manager.validate_tool_permissions(tool.as_ref())?;

        // 2. Enforce early cancellation check
        if context.cancellation.is_cancelled() {
            return Err(BrainError::Cancelled {
                message: format!("Execution of tool '{}' was cancelled", tool.metadata().name),
            });
        }

        // 3. Put the policy deadline in the execution context before starting
        // the runner. A tool must observe this deadline and the cancellation
        // token before it returns a terminal outcome. We deliberately do not
        // abandon a `spawn_blocking` runner with `select!`: doing so would let
        // a reported cancellation or timeout outlive the underlying work.
        let started_at = Instant::now();
        let policy_timeout = Duration::from_millis(tool.metadata().execution_policy.timeout_ms);
        let policy_deadline =
            started_at
                .checked_add(policy_timeout)
                .ok_or_else(|| BrainError::Internal {
                    message: format!(
                        "Tool '{}' timeout cannot be represented as an Instant",
                        tool.metadata().name
                    ),
                })?;
        let deadline = context
            .deadline
            .map(|existing| existing.min(policy_deadline))
            .unwrap_or(policy_deadline);

        if deadline <= started_at {
            return Err(BrainError::Timeout {
                elapsed_ms: 0,
                message: format!(
                    "Tool '{}' deadline elapsed before execution",
                    tool.metadata().name
                ),
            });
        }

        let tool_clone = tool.clone();
        let context_clone = ExecutionContext {
            deadline: Some(deadline),
            ..context.clone()
        };
        let args_clone = arguments.clone();
        let runner = self.runner.clone();

        let handle = tokio::task::spawn_blocking(move || {
            runner.run(tool_clone, &context_clone, &args_clone)
        });

        match handle.await {
            Ok(execution_res) => execution_res,
            Err(join_err) => Err(BrainError::Internal {
                message: format!("Tool runner panicked: {join_err}"),
            }),
        }
    }
}
