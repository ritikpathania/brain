//! Concrete bash tool: the first real `Tool` implementation, executed by the
//! brain-tools stack behind the Inc 4 permission round-trip.
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;

use brain_core::errors::BrainError;
use brain_core::extensibility::{
    ExecutionContext, ExecutionPolicy, ExecutionResult, Permission, Tool, ToolMetadata,
};

/// Maximum payload size for combined output before truncation.
const OUTPUT_LIMIT_BYTES: usize = 32_768;

#[derive(Default)]
pub struct BashTool;

impl Tool for BashTool {
    fn metadata(&self) -> &ToolMetadata {
        // Delegate to the inherent static accessor; do NOT write
        // `&self.metadata()` here — that recurses into this very method.
        Self::meta()
    }

    fn execute(
        &self,
        context: &ExecutionContext,
        arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult, BrainError> {
        let command = match arguments.get("command").and_then(|v| v.as_str()) {
            Some(c) if !c.trim().is_empty() => c.to_string(),
            _ => {
                return Err(BrainError::Internal {
                    message: "bash tool requires a non-empty string 'command' argument"
                        .to_string(),
                })
            }
        };

        let output = Command::new("/bin/bash")
            .arg("-c")
            .arg(command)
            .current_dir(&context.working_dir)
            .output()
            .map_err(|e| BrainError::Internal {
                message: format!("failed to spawn /bin/bash: {e}"),
            })?;

        // stdout first, stderr appended after a newline separator when present,
        // UTF-8 lossy, truncated per spec §4.1.
        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        if !output.stderr.is_empty() {
            if !text.ends_with('\n') && !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        if text.len() > OUTPUT_LIMIT_BYTES {
            let mut cut = OUTPUT_LIMIT_BYTES;
            while cut > 0 && !text.is_char_boundary(cut) {
                cut -= 1;
            }
            text.truncate(cut);
            text.push('…');
            text.push_str("[truncated]");
        }

        let exit_code = output.status.code().unwrap_or(-1);
        Ok(ExecutionResult::new(serde_json::json!({
            "output": text,
            "exit_code": exit_code,
            "is_error": !output.status.success(),
        })))
    }
}

impl BashTool {
    fn meta() -> &'static ToolMetadata {
        use std::sync::OnceLock;
        static META: OnceLock<ToolMetadata> = OnceLock::new();
        META.get_or_init(|| ToolMetadata {
            name: "bash".to_string(),
            description:
                "Executes a shell command with /bin/bash -c in the daemon working directory."
                    .to_string(),
            usage: "bash {\"command\": \"<shell command>\"}".to_string(),
            version: "0.1.0".to_string(),
            author: "brain".to_string(),
            required_permissions: vec![Permission::Shell],
            execution_policy: ExecutionPolicy { timeout_ms: 30_000 },
            supports_streaming: false,
            is_idempotent: false,
            causes_side_effects: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ExecutionContext {
        ExecutionContext {
            session_id: brain_domain::SessionId::new(),
            working_dir: std::env::temp_dir(),
            cancellation: Arc::new(brain_tools::CancellationTokenImpl::default()),
            deadline: None,
        }
    }

    fn args(command: &str) -> HashMap<String, serde_json::Value> {
        HashMap::from([("command".to_string(), serde_json::json!(command))])
    }

    #[test]
    fn echoes_stdout_with_zero_exit() {
        let result = BashTool.execute(&ctx(), &args("echo hello-inc5")).unwrap();
        let v = result.value();
        assert_eq!(v["is_error"], serde_json::json!(false));
        assert_eq!(v["exit_code"], serde_json::json!(0));
        assert!(v["output"].as_str().unwrap().contains("hello-inc5"));
    }

    #[test]
    fn non_zero_exit_is_a_result_not_an_error() {
        let result = BashTool.execute(&ctx(), &args("exit 3")).unwrap();
        let v = result.value();
        assert_eq!(v["is_error"], serde_json::json!(true));
        assert_eq!(v["exit_code"], serde_json::json!(3));
    }

    #[test]
    fn stderr_is_appended_after_stdout() {
        let result = BashTool
            .execute(&ctx(), &args("echo out; echo err 1>&2"))
            .unwrap();
        let v = result.value();
        assert!(v["output"].as_str().unwrap().contains("out"));
        assert!(v["output"].as_str().unwrap().contains("err"));
    }

    #[test]
    fn missing_or_empty_command_is_an_err() {
        assert!(BashTool.execute(&ctx(), &HashMap::new()).is_err());
        assert!(BashTool.execute(&ctx(), &args("   ")).is_err());
    }

    #[test]
    fn oversized_output_is_truncated_with_marker() {
        let result = BashTool
            .execute(&ctx(), &args("head -c 40000 /dev/zero | tr '\\0' 'a'"))
            .unwrap();
        let out = result.value()["output"].as_str().unwrap();
        assert!(out.len() <= OUTPUT_LIMIT_BYTES + 16);
        assert!(out.ends_with("…[truncated]"));
    }

    #[test]
    fn metadata_requests_shell_permission() {
        let meta = BashTool::meta();
        assert_eq!(meta.name, "bash");
        assert!(meta.required_permissions.contains(&Permission::Shell));
        assert_eq!(meta.execution_policy.timeout_ms, 30_000);
    }
}
