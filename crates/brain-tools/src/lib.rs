use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};

use brain_core::errors::BrainError;
use brain_core::extensibility::{
    CancellationToken as CoreCancellationToken, ExecutionContext, ExecutionResult, Permission,
    Tool, ToolRegistry, ToolRunner,
};

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
                message: format!("Tool '{}' is already registered", name),
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

        // 3. Coordinate execution using tokio's spawn_blocking for the runner,
        // selecting over timeouts and cancellation.
        let tool_clone = tool.clone();
        let context_clone = context.clone();
        let args_clone = arguments.clone();
        let runner = self.runner.clone();

        let handle = tokio::task::spawn_blocking(move || {
            runner.run(tool_clone, &context_clone, &args_clone)
        });

        let cancellation_future = async {
            loop {
                if context.cancellation.is_cancelled() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        };

        let timeout_duration = std::time::Duration::from_millis(tool.metadata().execution_policy.timeout_ms);

        tokio::select! {
            res = handle => {
                match res {
                    Ok(execution_res) => execution_res,
                    Err(join_err) => Err(BrainError::Internal {
                        message: format!("Tool runner panicked: {}", join_err),
                    }),
                }
            }
            _ = tokio::time::sleep(timeout_duration) => {
                Err(BrainError::Timeout {
                    elapsed_ms: tool.metadata().execution_policy.timeout_ms,
                    message: format!(
                        "Tool '{}' timed out after {}ms",
                        tool.metadata().name,
                        tool.metadata().execution_policy.timeout_ms
                    ),
                })
            }
            _ = cancellation_future => {
                Err(BrainError::Cancelled {
                    message: format!("Execution of tool '{}' was cancelled", tool.metadata().name),
                })
            }
        }
    }
}
