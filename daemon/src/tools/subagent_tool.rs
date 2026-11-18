//! Subagent/Task Tool for LLM agent delegation.
//!
//! Allows an LLM agent to spawn an isolated subagent, optionally within
//! a dedicated Git worktree, returning structured subagent execution metadata.

use brain_core::errors::BrainError;
use brain_core::extensibility::{
    ExecutionContext, ExecutionPolicy, ExecutionResult, Permission, Tool, ToolMetadata,
};
use brain_services::subagent::SubagentSpawnRequest;
use std::collections::HashMap;
use std::sync::Arc;

use crate::transport::uds::handlers::get_subagent_service;

/// Tool allowing LLM agents to spawn subagent tasks.
#[derive(Clone, Default)]
pub struct SubagentTool {
    custom_service: Option<Arc<brain_services::subagent::SubagentService>>,
    model_provider: Option<Arc<dyn brain_core::model::ModelProvider>>,
    child_tool: Option<Arc<dyn Tool>>,
    model_name: Option<String>,
}

/// Type alias for parity with Claude Code `Task` tool.
pub type TaskTool = SubagentTool;

static SUBAGENT_TOOL_METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();

impl SubagentTool {
    /// Creates a new default `SubagentTool`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a custom `SubagentService` instead of the global daemon singleton.
    pub fn with_service(mut self, service: Arc<brain_services::subagent::SubagentService>) -> Self {
        self.custom_service = Some(service);
        self
    }

    /// Sets an underlying `ModelProvider` to execute turns for spawned subagents.
    pub fn with_model_provider(
        mut self,
        provider: Arc<dyn brain_core::model::ModelProvider>,
        model_name: impl Into<String>,
    ) -> Self {
        self.model_provider = Some(provider);
        self.model_name = Some(model_name.into());
        self
    }

    /// Sets an optional tool made available to the subagent.
    pub fn with_child_tool(mut self, tool: Arc<dyn Tool>) -> Self {
        self.child_tool = Some(tool);
        self
    }

    /// Returns static metadata for the subagent delegation tool.
    pub fn meta() -> &'static ToolMetadata {
        SUBAGENT_TOOL_METADATA.get_or_init(|| ToolMetadata {
            name: "task".to_string(),
            description: "Spawns a specialized subagent to execute an isolated task, optionally in a dedicated Git worktree.".to_string(),
            usage: "task(prompt: string, role?: string, worktree?: boolean, mode?: string, kind?: string)".to_string(),
            version: "1.0.0".to_string(),
            author: "Brain Team".to_string(),
            required_permissions: vec![Permission::Shell],
            execution_policy: ExecutionPolicy { timeout_ms: 60_000 },
            supports_streaming: false,
            is_idempotent: false,
            causes_side_effects: true,
            input_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "The specific task instructions for the subagent."
                    },
                    "role": {
                        "type": "string",
                        "description": "The specialized role of the subagent (e.g. researcher, coder, reviewer)."
                    },
                    "worktree": {
                        "type": "boolean",
                        "description": "Whether to run the subagent in an isolated Git worktree."
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["foreground", "background"],
                        "description": "Execution mode (foreground blocking vs background async)."
                    },
                    "kind": {
                        "type": "string",
                        "enum": ["named", "fork"],
                        "description": "Subagent kind (named fresh context vs fork inheriting parent)."
                    }
                },
                "required": ["prompt"]
            })),
        })
    }
}

impl Tool for SubagentTool {
    fn metadata(&self) -> &ToolMetadata {
        Self::meta()
    }

    fn execute(
        &self,
        context: &ExecutionContext,
        arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult, BrainError> {
        let prompt = match arguments.get("prompt").and_then(|v| v.as_str()) {
            Some(p) if !p.trim().is_empty() => p.to_string(),
            _ => {
                return Err(BrainError::Internal {
                    message: "task tool requires a non-empty string 'prompt' argument".to_string(),
                });
            }
        };

        let role = arguments
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("subagent")
            .to_string();

        let use_worktree = arguments
            .get("worktree")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mode_str = arguments
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("foreground");

        let kind_str = arguments
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("named");

        let execution_mode = if mode_str == "background" {
            brain_domain::SubagentExecutionMode::Background
        } else {
            brain_domain::SubagentExecutionMode::Foreground
        };

        let subagent_kind = if kind_str == "fork" {
            brain_domain::SubagentKind::Fork
        } else {
            brain_domain::SubagentKind::Named
        };

        let parent_session_id = context.session_id;

        let subagent_service = self
            .custom_service
            .as_ref()
            .cloned()
            .unwrap_or_else(|| Arc::clone(get_subagent_service()));

        let spawn_request = SubagentSpawnRequest {
            parent_session_id,
            role: role.clone(),
            agent_type: if use_worktree {
                "worktree".to_string()
            } else {
                "task".to_string()
            },
            prompt: prompt.clone(),
            depth: 1,
            system_prompt: None,
            timeout: None,
            use_worktree,
            worktree_name: None,
            kind: Some(subagent_kind),
            execution_mode: Some(execution_mode),
        };

        let subagent_id =
            subagent_service
                .spawn(spawn_request, None)
                .map_err(|e| BrainError::Internal {
                    message: format!("Failed to spawn subagent: {e}"),
                })?;

        let model_name = self
            .model_name
            .clone()
            .unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string());

        if execution_mode == brain_domain::SubagentExecutionMode::Foreground {
            if let Some(ref mp) = self.model_provider {
                let svc = Arc::clone(&subagent_service);
                let prov = Arc::clone(mp);
                let sid = subagent_id;
                let mname = model_name;
                let ctool = self.child_tool.clone();

                let exec_result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    tokio::task::block_in_place(|| {
                        handle.block_on(async {
                            svc.execute_subagent(&sid, prov, &mname, ctool).await
                        })
                    })
                } else {
                    let rt = tokio::runtime::Runtime::new().map_err(|e| BrainError::Internal {
                        message: format!("Failed to create runtime: {e}"),
                    })?;
                    rt.block_on(async { svc.execute_subagent(&sid, prov, &mname, ctool).await })
                };

                let subagent_info = subagent_service.get_subagent(&subagent_id);
                let working_dir = subagent_info.as_ref().and_then(|s| s.working_dir.clone());
                let status = subagent_info
                    .as_ref()
                    .map(|s| s.status.to_string())
                    .unwrap_or_else(|| "completed".to_string());

                let result_text = match exec_result {
                    Ok(res) => res,
                    Err(e) => {
                        return Err(BrainError::Internal {
                            message: format!("Subagent execution failed: {e}"),
                        });
                    }
                };

                return Ok(ExecutionResult::new(serde_json::json!({
                    "subagent_id": subagent_id.to_string(),
                    "status": status,
                    "role": role,
                    "working_dir": working_dir,
                    "result": result_text,
                    "mode": mode_str,
                    "kind": kind_str,
                    "message": format!("Subagent '{role}' [{subagent_id}] completed ({kind_str}, {mode_str})"),
                })));
            }
        } else if execution_mode == brain_domain::SubagentExecutionMode::Background {
            if let Some(ref mp) = self.model_provider {
                let svc = Arc::clone(&subagent_service);
                let prov = Arc::clone(mp);
                let sid = subagent_id;
                let mname = model_name;
                let ctool = self.child_tool.clone();

                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        let _ = svc.execute_subagent(&sid, prov, &mname, ctool).await;
                    });
                }
            }
        }

        // Start subagent if not executed synchronously above
        let _ = subagent_service.start(&subagent_id);

        let subagent_info = subagent_service.get_subagent(&subagent_id);
        let working_dir = subagent_info.as_ref().and_then(|s| s.working_dir.clone());
        let status = subagent_info
            .as_ref()
            .map(|s| s.status.to_string())
            .unwrap_or_else(|| "running".to_string());
        let result_text = subagent_info.as_ref().and_then(|s| s.result.clone());

        Ok(ExecutionResult::new(serde_json::json!({
            "subagent_id": subagent_id.to_string(),
            "status": status,
            "role": role,
            "working_dir": working_dir,
            "result": result_text,
            "mode": mode_str,
            "kind": kind_str,
            "message": format!("Subagent '{role}' [{subagent_id}] ({kind_str}, {mode_str})"),
        })))
    }
}
