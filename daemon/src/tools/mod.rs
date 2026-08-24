//! Daemon-owned concrete tools and their brain-tools wiring (Inc 5).
pub mod bash_tool;

pub use bash_tool::BashTool;

use std::sync::{Arc, OnceLock};

// `register_tool`/`get_tool`/`list_tools` are TRAIT methods (brain_core's
// ToolRegistry), not inherent to ToolRegistryImpl — without this import the
// calls below fail to resolve.
use brain_core::extensibility::{Tool as _, ToolRegistry};
use brain_tools::{BlockingToolRunner, PermissionManager, ToolExecutor, ToolRegistryImpl};

/// One lazily-initialized executor stack shared by every stream connection.
pub struct ToolStack {
    pub registry: ToolRegistryImpl,
    pub permissions: PermissionManager,
    pub executor: ToolExecutor,
}

static TOOL_STACK: OnceLock<Arc<ToolStack>> = OnceLock::new();

pub fn tool_stack() -> &'static Arc<ToolStack> {
    TOOL_STACK.get_or_init(|| {
        let registry = ToolRegistryImpl::default();
        registry
            .register_tool(Arc::new(BashTool))
            .expect("bash tool registers exactly once");
        Arc::new(ToolStack {
            registry,
            permissions: PermissionManager::default(),
            executor: ToolExecutor::new(Arc::new(BlockingToolRunner)),
        })
    })
}

#[cfg(test)]
mod stack_tests {
    use super::*;

    #[test]
    fn stack_registers_bash_and_nothing_else() {
        let names: Vec<String> = tool_stack()
            .registry
            .list_tools()
            .iter()
            .map(|t| t.metadata().name.clone())
            .collect();
        assert_eq!(names, vec!["bash".to_string()]);
        assert!(tool_stack().registry.get_tool("nosuchtool").is_none());
    }
}
