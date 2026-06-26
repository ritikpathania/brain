use std::collections::HashMap;
use std::sync::Arc;

use brain_core::errors::BrainError;
use brain_core::extensibility::{
    ExecutionContext, ExecutionPolicy, ExecutionResult, Permission,
    Tool, ToolMetadata, ToolRegistry,
};
use brain_domain::SessionId;
use brain_tools::{
    BlockingToolRunner, CancellationTokenImpl, PermissionManager, ToolExecutor, ToolRegistryImpl,
};

struct MockTool {
    metadata: ToolMetadata,
    delay_ms: u64,
}

impl MockTool {
    fn new(name: &str, permissions: Vec<Permission>, timeout_ms: u64, delay_ms: u64) -> Self {
        Self {
            metadata: ToolMetadata {
                name: name.to_string(),
                description: "description".to_string(),
                usage: "usage".to_string(),
                version: "1.0".to_string(),
                author: "author".to_string(),
                required_permissions: permissions,
                execution_policy: ExecutionPolicy { timeout_ms },
                supports_streaming: false,
                is_idempotent: true,
                causes_side_effects: false,
            },
            delay_ms,
        }
    }
}

impl Tool for MockTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    fn execute(
        &self,
        _context: &ExecutionContext,
        _arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult, BrainError> {
        if self.delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(self.delay_ms));
        }
        Ok(ExecutionResult::new(serde_json::Value::String("success".to_string())))
    }
}

#[tokio::test]
async fn test_tool_registry() {
    let registry = ToolRegistryImpl::new();
    let tool_b = Arc::new(MockTool::new("tool_b", vec![], 1000, 0));
    let tool_a = Arc::new(MockTool::new("tool_a", vec![], 1000, 0));
    
    registry.register_tool(tool_b).unwrap();
    registry.register_tool(tool_a).unwrap();

    // Verify fetching by name
    let fetched = registry.get_tool("tool_a");
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().metadata().name, "tool_a");

    // Verify sorting in list_tools
    let list = registry.list_tools();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].metadata().name, "tool_a");
    assert_eq!(list[1].metadata().name, "tool_b");
}

#[tokio::test]
async fn test_tool_registry_duplicate_error() {
    let registry = ToolRegistryImpl::new();
    let tool1 = Arc::new(MockTool::new("tool_a", vec![], 1000, 0));
    let tool2 = Arc::new(MockTool::new("tool_a", vec![], 1000, 0));

    registry.register_tool(tool1).unwrap();
    let res = registry.register_tool(tool2);
    assert!(res.is_err());
    assert!(matches!(res.err().unwrap(), BrainError::InvalidTransition { .. }));
}

#[tokio::test]
async fn test_tool_permission_denied() {
    let runner = Arc::new(BlockingToolRunner);
    let executor = ToolExecutor::new(runner);
    let permission_manager = PermissionManager::new();
    
    let context = ExecutionContext {
        session_id: SessionId::new(),
        working_dir: std::env::temp_dir(),
        cancellation: Arc::new(CancellationTokenImpl::new()),
        deadline: None,
    };

    let tool = Arc::new(MockTool::new(
        "protected_tool",
        vec![Permission::FilesystemRead],
        1000,
        0,
    ));

    let res = executor
        .execute(tool, &context, &permission_manager, &HashMap::new())
        .await;

    assert!(res.is_err());
    assert!(matches!(res.err().unwrap(), BrainError::Authorization { .. }));
}

#[tokio::test]
async fn test_tool_permission_granted() {
    let runner = Arc::new(BlockingToolRunner);
    let executor = ToolExecutor::new(runner);
    let permission_manager = PermissionManager::new();
    permission_manager.grant(Permission::FilesystemRead);

    let context = ExecutionContext {
        session_id: SessionId::new(),
        working_dir: std::env::temp_dir(),
        cancellation: Arc::new(CancellationTokenImpl::new()),
        deadline: None,
    };

    let tool = Arc::new(MockTool::new(
        "protected_tool",
        vec![Permission::FilesystemRead],
        1000,
        0,
    ));

    let res = executor
        .execute(tool, &context, &permission_manager, &HashMap::new())
        .await;

    assert!(res.is_ok());
    assert_eq!(
        res.unwrap().value(),
        &serde_json::Value::String("success".to_string())
    );
}

#[tokio::test]
async fn test_tool_timeout() {
    let runner = Arc::new(BlockingToolRunner);
    let executor = ToolExecutor::new(runner);
    let permission_manager = PermissionManager::new();
    
    let context = ExecutionContext {
        session_id: SessionId::new(),
        working_dir: std::env::temp_dir(),
        cancellation: Arc::new(CancellationTokenImpl::new()),
        deadline: None,
    };

    // Set timeout to 50ms, but execution delays for 200ms
    let tool = Arc::new(MockTool::new("slow_tool", vec![], 50, 200));

    let res = executor
        .execute(tool, &context, &permission_manager, &HashMap::new())
        .await;

    assert!(res.is_err());
    assert!(matches!(res.err().unwrap(), BrainError::Timeout { .. }));
}

#[tokio::test]
async fn test_tool_cancellation() {
    let runner = Arc::new(BlockingToolRunner);
    let executor = ToolExecutor::new(runner);
    let permission_manager = PermissionManager::new();
    let cancellation_token = Arc::new(CancellationTokenImpl::new());
    
    let context = ExecutionContext {
        session_id: SessionId::new(),
        working_dir: std::env::temp_dir(),
        cancellation: cancellation_token.clone(),
        deadline: None,
    };

    // Execution delays for 500ms
    let tool = Arc::new(MockTool::new("cancellable_tool", vec![], 1000, 500));

    // Cancel execution after 100ms
    let token_clone = cancellation_token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        token_clone.cancel();
    });

    let res = executor
        .execute(tool, &context, &permission_manager, &HashMap::new())
        .await;

    assert!(res.is_err());
    assert!(matches!(res.err().unwrap(), BrainError::Cancelled { .. }));
}
