//! Daemon-owned concrete tools and their brain-tools wiring (Inc 5).
pub mod bash_tool;

pub use bash_tool::BashTool;

use std::sync::{Arc, OnceLock};

// `register_tool`/`get_tool`/`list_tools` are TRAIT methods (brain_core's
// ToolRegistry), not inherent to ToolRegistryImpl — without this import the
// calls below fail to resolve.
use brain_core::extensibility::ToolRegistry;
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

/// Converts registered tool metadata into provider-facing
/// [`ToolDefinition`]s (Inc 7). Pure advertisement: this changes nothing
/// about execution — the permission round-trip remains the sole gate.
///
/// Tools without an `input_schema` advertise a permissive
/// `{"type":"object"}`; `list_tools()` is name-sorted, so advertisement
/// order is deterministic.
pub fn definitions_from(
    registry: &dyn ToolRegistry,
) -> Vec<brain_core::model::ToolDefinition> {
    registry
        .list_tools()
        .iter()
        .map(|tool| {
            let meta = tool.metadata();
            brain_core::model::ToolDefinition {
                name: meta.name.clone(),
                description: meta.description.clone(),
                parameters: meta
                    .input_schema
                    .clone()
                    .unwrap_or_else(|| serde_json::json!({"type": "object"})),
            }
        })
        .collect()
}

/// Advertises the daemon's global tool stack to providers (Inc 7).
pub fn advertised_definitions() -> Vec<brain_core::model::ToolDefinition> {
    definitions_from(&tool_stack().registry)
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

#[cfg(test)]
mod definition_tests {
    use super::*;
    use brain_core::errors::BrainError;
    use brain_core::extensibility::{
        ExecutionContext, ExecutionPolicy, ExecutionResult, Permission, Tool,
    };
    use std::collections::HashMap;

    struct FakeTool {
        meta: brain_core::extensibility::ToolMetadata,
    }
    impl Tool for FakeTool {
        fn metadata(&self) -> &brain_core::extensibility::ToolMetadata {
            &self.meta
        }
        fn execute(
            &self,
            _: &ExecutionContext,
            _: &HashMap<String, serde_json::Value>,
        ) -> Result<ExecutionResult, BrainError> {
            Ok(ExecutionResult::new(serde_json::Value::Null))
        }
    }

    fn fake(name: &str, description: &str, schema: Option<serde_json::Value>) -> FakeTool {
        FakeTool {
            meta: brain_core::extensibility::ToolMetadata {
                name: name.to_string(),
                description: description.to_string(),
                usage: String::new(),
                version: "0".to_string(),
                author: "test".to_string(),
                required_permissions: Vec::<Permission>::new(),
                execution_policy: ExecutionPolicy { timeout_ms: 100 },
                supports_streaming: false,
                is_idempotent: true,
                causes_side_effects: false,
                input_schema: schema,
            },
        }
    }

    #[test]
    fn definitions_are_name_sorted_with_schema_passthrough_and_fallback() {
        let registry = ToolRegistryImpl::default();
        registry
            .register_tool(Arc::new(fake(
                "zeta",
                "last",
                Some(serde_json::json!({"type": "string"})),
            )))
            .unwrap();
        registry.register_tool(Arc::new(fake("alpha", "first", None))).unwrap();

        let defs = definitions_from(&registry);
        assert_eq!(
            defs.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(),
            vec!["alpha", "zeta"],
            "BTreeMap-backed list_tools is already name-sorted"
        );
        assert_eq!(defs[0].description, "first");
        assert_eq!(defs[0].parameters, serde_json::json!({"type": "object"}));
        assert_eq!(defs[1].parameters, serde_json::json!({"type": "string"}));
    }

    #[test]
    fn empty_registry_yields_no_definitions() {
        let registry = ToolRegistryImpl::default();
        assert!(definitions_from(&registry).is_empty());
    }

    #[test]
    fn global_stack_advertises_exactly_bash() {
        let defs = advertised_definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "bash");
        assert_eq!(defs[0].parameters["required"][0], "command");
    }
}
