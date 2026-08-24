# Brain Shell Increment 5 — Daemon-Side Tool Executor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An approved `bash` tool call executes daemon-side and its output returns over UDS as a `tool_result` stream frame that renders on the shell's tool card.

**Architecture:** Reuse the dormant executor stack — `brain_core::extensibility::Tool` trait and `crates/brain-tools`' registry/PermissionManager/executor — via a new daemon-owned `tools` module holding one concrete `BashTool`. The Inc 4 post-grant branch (currently a no-op) dispatches through the registry and emits exactly one `tool_result` frame before the stream continues to completion. The shell parses the frame into the existing `tool_result` turn event and gains minimal card-output rendering.

**Tech Stack:** Rust (tokio, serde_json) daemon; Bun + React 19 + Ink 7 shell; Python 3 PTY harness.

**Spec:** `docs/superpowers/specs/2026-08-24-brain-shell-inc5-tool-executor-design.md`

## Global Constraints

Verbatim from the spec (§7) plus machine discipline:

- Preserve Brain's architecture, domain model, IPC contracts, runtime, memory, retrieval, graph, provenance, agents, and adapter boundaries.
- No Claude/Anthropic models, APIs, auth, pricing, billing, or LLM-specific product concepts introduced.
- The Claude Code tree stays implementation archaeology outside the repository forever.
- Stack: Bun + React 19 + Ink 7 + yoga-layout; no framework changes.
- Small increments, each independently verifiable; commits carry explicitly-added paths only (`git add <path>`, never `git add .`); every commit message ends with the trailer `Co-Authored-By: Claude <noreply@anthropic.com>`.
- **Every cargo invocation on this machine needs the rpath prefix:**
  `RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks"`
- All git commands use `git -C /Users/ritikpathania/Developer/PyCharm/brain …`; bun/cargo invocations are wrapped in `bash -c '…'`.
- Canonical bundle gate is `bun build src/main.tsx --outdir dist --target bun` (there is NO package.json build script).
- Harmless noise to ignore: "error: daemon terminated" on git calls; "duplicate -rpath ignored" linker warning.
- Work on branch `feature/brain-shell-inc5-tool-executor` created from `main`.

### Wire contract (single source of truth for Tasks 2–5)

```json
{"type":"tool_result","generation_id":"<uuid>","session_id":"<id>","sequence":N,
 "call_id":"call_mock_1","tool_name":"bash","output":"hello\n",
 "is_error":false,"exit_code":0,"status":"in_progress"}
```

Sequence arithmetic (strict consecutive guard in the shell): `stream_start`=0, `tool_use`=1, `tool_permission_requested`=2, **`tool_result`=3**, then remaining provider chunks continue from 4 (`stream_end` uses its loop iteration's already-incremented seq).

### Key existing symbols an implementer must not confuse

- `handlers.rs` already imports `brain_application::context::ExecutionContext` (line 10) and `tokio_util::sync::CancellationToken` (line 8). The extensibility context MUST be aliased on import: `use brain_core::extensibility::ExecutionContext as ToolExecutionContext;`
- `brain_domain::SessionId` is `Copy` — pass by value freely.
- `brain_tools` crate-root exports (single-file crate): `CancellationTokenImpl`, `PermissionManager`, `ToolRegistryImpl`, `BlockingToolRunner`, `ToolExecutor`.
- `PermissionManager::{grant(Permission), is_granted(Permission), validate_tool_permissions(&dyn Tool)}`; `ToolExecutor::execute(tool: Arc<dyn Tool>, ctx: &ToolExecutionContext, perms: &PermissionManager, args: &HashMap<String, serde_json::Value>) -> Result<ExecutionResult, BrainError>` (async).
- `brain_core::extensibility::{Tool, ToolMetadata, ExecutionPolicy, Permission, ExecutionResult, ExecutionContext, CancellationToken}`; `BrainError` variants used here: `Internal { message }`.
- Mock sentinel `[brain-tool:NAME|{json}]` in the last user prompt makes DeterministicMockProvider emit `GenerationChunk::ToolUse { id: "call_mock_<n>", name, input }` with tokens `["Invoking tool NAME."]` streamed AFTER the tool-use chunk and `finish_reason: "tool_use"`.

---

### Task 1: `BashTool` + `ToolStack` wiring

**Files:**
- Modify: `daemon/Cargo.toml` (dependencies section)
- Modify: `daemon/src/lib.rs` (module list)
- Create: `daemon/src/tools/mod.rs`
- Create: `daemon/src/tools/bash_tool.rs`

**Interfaces:**
- Consumes: `brain_core::extensibility::*` traits only; nothing from handlers yet.
- Produces:
  - `pub struct BashTool;` implementing `Tool` (metadata name `"bash"`).
  - `crate::tools::tool_stack() -> &'static std::sync::Arc<ToolStack>` where
    `pub struct ToolStack { pub registry: ToolRegistryImpl, pub permissions: PermissionManager, pub executor: ToolExecutor }`.

- [ ] **Step 1: Add the dependency**

In `daemon/Cargo.toml`, in the `[dependencies]` section directly after the line `brain-application = { path = "../crates/brain-application" }`, add:

```toml
brain-tools = { path = "../crates/brain-tools" }
```

In `daemon/src/lib.rs`, after the line `pub mod transport;`, add:

```rust
pub mod tools;
```

- [ ] **Step 2: Write `daemon/src/tools/bash_tool.rs` with tests included**

```rust
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
    fn metadata(&self) -> &ToolMetadata {
        use std::sync::OnceLock;
        static META: OnceLock<ToolMetadata> = OnceLock::new();
        META.get_or_init(|| ToolMetadata {
            name: "bash".to_string(),
            description: "Executes a shell command with /bin/bash -c in the daemon working directory."
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
        let result = BashTool.execute(&ctx(), &args("echo out; echo err 1>&2")).unwrap();
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
        let meta = BashTool.metadata();
        assert_eq!(meta.name, "bash");
        assert!(meta.required_permissions.contains(&Permission::Shell));
        assert_eq!(meta.execution_policy.timeout_ms, 30_000);
    }
}
```

Note: `BrainError` path may be `brain_core::errors::BrainError` or re-exported at `brain_core::BrainError` — check how `crates/brain-tools/src/lib.rs` imports it (it uses `BrainError` from somewhere; mirror that exact path). If the crate path differs, adjust imports; do NOT change trait signatures.

- [ ] **Step 3: Write `daemon/src/tools/mod.rs`**

```rust
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
```

- [ ] **Step 4: Run the new unit tests**

Run:
```bash
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon tools::'
```
Expected: all `tests::` and `stack_tests::` cases PASS. Fix compile errors by mirroring existing import paths (see note in Step 2); do not weaken assertions.

- [ ] **Step 5: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add daemon/Cargo.toml daemon/src/lib.rs daemon/src/tools/mod.rs daemon/src/tools/bash_tool.rs
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "feat(daemon): BashTool on the brain-tools executor stack

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Post-grant execution + `tool_result` frame (daemon)

**Files:**
- Create: `daemon/tests/uds_tool_execution_tests.rs`
- Modify: `daemon/src/transport/uds/handlers.rs` (ToolUse arm, granted branch)

**Interfaces:**
- Consumes: Task 1's `crate::tools::tool_stack()`; the Inc 4 permission gate variables in scope at the insertion point: `call_id: String`, `tool_name: String`, `generation_id`, `session_id_str`, `parsed_session_id` (Copy `SessionId`), `seq: i64-ish`, `writer`, `perm_packet` (still owned, contains the request input under `["input"]`).
- Produces: wire frame per the contract table above; emitted exactly once per granted call, sequence = permission-frame sequence + 1.

- [ ] **Step 1: Write the integration test file**

Create `daemon/tests/uds_tool_execution_tests.rs`. Copy the harness VERBATIM from `daemon/tests/uds_permission_roundtrip_tests.rs`: everything from the `use` block through the helpers `send_frame`, `read_line_frame`, `open_and_create_session`, `start_generation_with_sentinel` (rename the last to `start_generation_with_prompt` taking the prompt as a parameter so each flow controls its sentinel):

```rust
async fn start_generation_with_prompt(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    session_id: &str,
    prompt: &str,
) {
    let req = serde_json::json!({
        "id": "req-tool-exec",
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "model": "brain-default",
            "messages": [{ "role": "user", "content": prompt }]
        }
    });
    send_frame(writer, &req).await;
}
```

Then these four tests (all `#[tokio::test]`; each collects frames into a `Vec<serde_json::Value>` until a frame whose type is `"finished"` OR whose top-level `"type"` equals `"stream_end"` followed by a finished reply, using a 15 s overall deadline):

```rust
const GRANT_ECHO_PROMPT: &str =
    "run [brain-tool:bash|{\"command\":\"echo hello-inc5\"}] please";
const DENY_PROMPT: &str =
    "run [brain-tool:bash|{\"command\":\"echo should-not-run\"}] please";
const UNKNOWN_TOOL_PROMPT: &str =
    "run [brain-tool:nosuchtool|{\"command\":\"x\"}] please";
const FAILING_PROMPT: &str =
    "run [brain-tool:bash|{\"command\":\"exit 3\"}] please";

/// Drives one generation to completion, resolving any permission request via a
/// SECOND connection (the stream connection stays parked). Returns every frame
/// observed on the stream connection.
async fn run_turn_resolving(
    reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    socket_path: &std::path::Path,
    granted: bool,
) -> Vec<serde_json::Value> {
    let mut frames = Vec::new();
    loop {
        let frame =
            tokio::time::timeout(std::time::Duration::from_secs(15), read_line_frame(reader))
                .await
                .expect("frame within 15s");
        let ftype = frame["type"].as_str().unwrap_or("").to_string();
        let is_perm = ftype == "tool_permission_requested";
        frames.push(frame);
        if is_perm {
            let (r2, mut w2) = tokio::net::UnixStream::connect(socket_path)
                .await
                .unwrap()
                .into_split();
            drop(r2);
            let call_id = frames.last().unwrap()["call_id"]
                .as_str()
                .unwrap()
                .to_string();
            send_frame(
                &mut w2,
                &serde_json::json!({
                    "id": "req-resolve",
                    "action": "v1/tool/resolve",
                    "payload": { "call_id": call_id, "granted": granted }
                }),
            )
            .await;
            let _verdict = read_line_frame(&mut BufReader::new(r2_reopen(socket_path)).await).await; // see note
        }
        let last_type = frames.last().unwrap()["type"].as_str().unwrap_or("");
        if last_type == "stream_end" || last_type == "finished" || last_type == "error" {
            break;
        }
    }
    frames
}
```

Implementation note: reading the verdict reply requires owning the second connection's reader. Restructure instead of the sketch above: connect the second connection BEFORE the resolve, keep both halves, `send_frame` the resolve, read one line from its reader (assert `status=="ok"`), then drop both halves. Do not reuse `r2` after moving it into `BufReader`.

The four tests:

```rust
#[tokio::test]
async fn granted_bash_executes_and_emits_tool_result() {
    let daemon = start_test_daemon().await;
    let (mut reader, mut writer, session_id) = open_and_create_session(&daemon.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, GRANT_ECHO_PROMPT).await;
    let frames = run_turn_resolving(&mut reader, &mut writer, &daemon.socket_path, true).await;

    let types: Vec<&str> = frames.iter().map(|f| f["type"].as_str().unwrap_or("")).collect();
    let perm_idx = types.iter().position(|t| *t == "tool_permission_requested").unwrap();
    let result_idx = types.iter().position(|t| *t == "tool_result").unwrap();

    let tr = &frames[result_idx];
    assert_eq!(tr["call_id"], frames[perm_idx]["call_id"]);
    assert_eq!(tr["tool_name"], "bash");
    assert!(tr["output"].as_str().unwrap().contains("hello-inc5"));
    assert_eq!(tr["is_error"], serde_json::json!(false));
    assert_eq!(tr["exit_code"], serde_json::json!(0));
    // Strictly consecutive: tool_result follows the permission request.
    assert_eq!(
        tr["sequence"].as_i64().unwrap(),
        frames[perm_idx]["sequence"].as_i64().unwrap() + 1
    );
}

#[tokio::test]
async fn denied_call_never_emits_tool_result() {
    let daemon = start_test_daemon().await;
    let (mut reader, mut writer, session_id) = open_and_create_session(&daemon.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, DENY_PROMPT).await;
    let frames = run_turn_resolving(&mut reader, &mut writer, &daemon.socket_path, false).await;

    assert!(frames.iter().any(|f| f["type"] == "tool_denied"));
    assert!(!frames.iter().any(|f| f["type"] == "tool_result"));
}

#[tokio::test]
async fn unknown_tool_yields_error_result_without_execution() {
    let daemon = start_test_daemon().await;
    let (mut reader, mut writer, session_id) = open_and_create_session(&daemon.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, UNKNOWN_TOOL_PROMPT).await;
    let frames = run_turn_resolving(&mut reader, &mut writer, &daemon.socket_path, true).await;

    let tr = frames.iter().find(|f| f["type"] == "tool_result").unwrap();
    assert_eq!(tr["is_error"], serde_json::json!(true));
    assert!(tr["output"].as_str().unwrap().contains("Unknown tool"));
}

#[tokio::test]
async fn failing_command_reports_exit_code_as_error_result() {
    let daemon = start_test_daemon().await;
    let (mut reader, mut writer, session_id) = open_and_create_session(&daemon.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, FAILING_PROMPT).await;
    let frames = run_turn_resolving(&mut reader, &mut writer, &daemon.socket_path, true).await;

    let tr = frames.iter().find(|f| f["type"] == "tool_result").unwrap();
    assert_eq!(tr["is_error"], serde_json::json!(true));
    assert_eq!(tr["exit_code"], serde_json::json!(3));
}
```

- [ ] **Step 2: Run to verify RED**

Run:
```bash
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test --test uds_tool_execution_tests 2>&1 | tail -12'
```
Expected: `granted_bash_executes_and_emits_tool_result` and the unknown/failing tests FAIL ("no tool_result"), deny test PASSES trivially. Compile errors mean fix imports first — the runtime behavior must be red, not the build.

- [ ] **Step 3: Implement the granted branch**

In `daemon/src/transport/uds/handlers.rs`, locate the ToolUse arm's post-waiter block:

```rust
                                            get_permission_waiters()
                                                .write()
                                                .await
                                                .remove(&call_id);

                                            if !granted {
```

Insert between those two statements:

```rust
                                            if granted {
                                                // Inc 5: the wire verdict is the
                                                // executor-side authority; execute and
                                                // report one tool_result frame.
                                                use brain_core::extensibility::{
                                                    ExecutionContext as ToolExecutionContext,
                                                    ToolRegistry,
                                                };
                                                let stack = crate::tools::tool_stack();
                                                stack
                                                    .permissions
                                                    .grant(brain_core::extensibility::Permission::Shell);
                                                let mut args_map: HashMap<String, serde_json::Value> =
                                                    HashMap::new();
                                                if let Some(obj) =
                                                    packet["toolUse"]["input"].as_object()
                                                {
                                                    for (k, v) in obj {
                                                        args_map.insert(k.clone(), v.clone());
                                                    }
                                                }
                                                let tool_ctx = ToolExecutionContext {
                                                    session_id: parsed_session_id,
                                                    working_dir: std::env::current_dir()
                                                        .unwrap_or_else(|_| {
                                                            std::path::PathBuf::from(".")
                                                        }),
                                                    cancellation: Arc::new(
                                                        brain_tools::CancellationTokenImpl::default(),
                                                    ),
                                                    deadline: None,
                                                };
                                                seq += 1;
                                                let execution = match stack.registry.get_tool(&tool_name) {
                                                    Some(tool) => stack
                                                        .executor
                                                        .execute(tool, &tool_ctx, &stack.permissions, &args_map)
                                                        .await,
                                                    None => Err(BrainError::Internal {
                                                        message: format!("Unknown tool '{tool_name}'"),
                                                    }),
                                                };
                                                let (out_text, is_err, exit_code) = match execution {
                                                    Ok(result) => {
                                                        let v = result.value().clone();
                                                        (
                                                            v["output"].as_str().unwrap_or("").to_string(),
                                                            v["is_error"].as_bool().unwrap_or(true),
                                                            v["exit_code"].as_i64().unwrap_or(-1),
                                                        )
                                                    }
                                                    Err(e) => (format!("{e}"), true, -1),
                                                };
                                                let result_packet = serde_json::json!({
                                                    "type": "tool_result",
                                                    "generation_id": generation_id,
                                                    "session_id": session_id_str,
                                                    "sequence": seq,
                                                    "call_id": call_id,
                                                    "tool_name": tool_name,
                                                    "output": out_text,
                                                    "is_error": is_err,
                                                    "exit_code": exit_code,
                                                    "status": "in_progress"
                                                });
                                                let mut rj = serde_json::to_string(&result_packet)?;
                                                rj.push('\n');
                                                writer.write_all(rj.as_bytes()).await?;
                                                writer.flush().await?;
                                            }

                                            if !granted {
```

Adjustments the implementer must verify while editing:

1. `packet["toolUse"]["input"]` — confirm `packet` is still in scope and owned at this point (it was only borrowed by `to_string`). If the compiler disagrees, snapshot the input BEFORE building the first tool_use packet: `let tool_input = input.clone();` right after `let tool_name = name.clone();`, and build `args_map` from `&tool_input` instead.
2. `parsed_session_id` is `Copy` — passing it by value into the context is safe ONLY if later code (assistant-message persistence near the end of the stream loop) also has it available. Since it is `Copy`, later uses remain valid.
3. `BrainError` must be imported in handlers.rs (check whether it already is; add `use brain_core::errors::BrainError;` or the crate's actual re-export path, matching whatever `crates/brain-tools/src/lib.rs` uses).
4. If `registry.get_tool` takes `&self` while `stack.registry` is behind `&'static Arc<ToolStack>` — fine as-is; do not wrap in extra locks.

- [ ] **Step 4: Run to GREEN**

Run:
```bash
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test --test uds_tool_execution_tests --test uds_permission_roundtrip_tests --test uds_generation_tests 2>&1 | grep "test result"'
```
Expected: three `ok.` lines — 4/4 new, 3/3 permission, 3/3 generation. Any regression in the older suites means the insertion disturbed the deny path or sequences; fix before committing.

- [ ] **Step 5: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add daemon/tests/uds_tool_execution_tests.rs daemon/src/transport/uds/handlers.rs
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "feat(daemon): execute approved bash calls and emit tool_result

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Shell client parses `tool_result` frames

**Files:**
- Modify: `packages/brain-shell/src/client/BrainBackendClient.ts` (chunk union)
- Modify: `packages/brain-shell/src/client/UdsBrainBackendClient.ts` (parser branch)
- Test: `packages/brain-shell/src/test/client/toolResultWire.test.ts`

**Interfaces:**
- Consumes: wire contract above; the tolerant-reception pattern of the existing `tool_permission_requested` parser branch.
- Produces: chunk `{ type:'tool_result', callId?: string, toolName?: string, output?: string, isError?: boolean, exitCode?: number, sequence?: number, generationId?, sessionId?, status? }` consumed by Task 4.

- [ ] **Step 1: Extend the chunk union**

In `BrainBackendClient.ts`, extend the `BrainStreamChunk.type` union literal set (`'token' | … | 'permission_request'`) with `'tool_result'`, and add fields alongside the permission ones:

```ts
  /** Present when type === 'tool_result'. */
  output?: string;
  isError?: boolean;
  exitCode?: number;
```

(`callId`, `toolName`, `sequence`, `generationId`, `sessionId`, `status` already exist.)

- [ ] **Step 2: Write the failing wire test**

Create `packages/brain-shell/src/test/client/toolResultWire.test.ts` modeled on `resolvePermissionWire.test.ts` (in-process `net.createServer` on an mkdtemp socket):

```ts
import { describe, expect, test, afterAll } from 'bun:test';
import * as net from 'node:net';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { UdsBrainBackendClient } from '../../client/UdsBrainBackendClient.js';

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-tool-result-wire-'));
const sockPath = path.join(dir, 't.sock');

// Scripted daemon: on v1/session/create reply success; on v1/generation/stream
// emit stream_start(0), tool_use(1), tool_permission_requested(2); when ANY
// v1/tool/resolve arrives with granted:true, emit tool_result(3), token(4),
// finished(5). With granted:false emit tool_denied(3), finished(4).
const SCRIPT: string[] = [];
const server = net.createServer((socket) => {
  let buffer = '';
  socket.on('data', (data) => {
    buffer += data.toString('utf8');
    let idx: number;
    while ((idx = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, idx);
      buffer = buffer.slice(idx + 1);
      if (!line.trim()) continue;
      const req = JSON.parse(line) as { id?: number; action?: string; payload?: Record<string, unknown> };
      const reply = (obj: unknown) => socket.write(JSON.stringify(obj) + '\n');
      if (req.action === 'v1/session/create') {
        reply({ id: req.id, status: 'success', body: { session_id: 'stub-tr', title: 't', created_at: 0 } });
      } else if (req.action === 'v1/generation/stream') {
        const sid = 'stub-tr';
        const emit = (o: Record<string, unknown>) => socket.write(JSON.stringify(o) + '\n');
        emit({ type: 'stream_start', session_id: sid, sequence: 0 });
        emit({ type: 'tool_use', session_id: sid, sequence: 1, toolUse: { id: 'call_tr', name: 'bash', input: { command: 'echo hi' } } });
        emit({ type: 'tool_permission_requested', session_id: sid, sequence: 2, call_id: 'call_tr', tool_name: 'bash', reason: 'gate' });
        SCRIPT.push(socket.write.bind(socket)); // resolved below via global hook
        pendingStreams.push((granted: boolean) => {
          if (granted) {
            emit({ type: 'tool_result', session_id: sid, sequence: 3, call_id: 'call_tr', tool_name: 'bash', output: 'hi\n', is_error: false, exit_code: 0, status: 'in_progress' });
            emit({ type: 'token', session_id: sid, sequence: 4, token: 'ok', status: 'in_progress' });
            emit({ type: 'finished', session_id: sid, sequence: 5, status: 'completed' });
          } else {
            emit({ type: 'tool_denied', session_id: sid, sequence: 3, call_id: 'call_tr', tool_name: 'bash', status: 'in_progress' });
            emit({ type: 'finished', session_id: sid, sequence: 4, status: 'completed' });
          }
        });
      } else if (req.action === 'v1/tool/resolve') {
        const granted = Boolean(req.payload?.['granted']);
        reply({ type: 'resolved', status: 'ok' });
        const resume = pendingStreams.shift();
        resume?.(granted);
      }
    }
  });
});
const pendingStreams: Array<(granted: boolean) => void> = [];
server.listen(sockPath);

afterAll(() => {
  server.close();
  fs.rmSync(dir, { recursive: true, force: true });
});

async function collect(chunks: AsyncIterable<{ type: string }>): Promise<Array<{ type: string }>> {
  const out: Array<{ type: string }> = [];
  for await (const c of chunks) out.push(c);
  return out;
}

describe('UDS client parses tool_result frames', () => {
  test('granted flow yields a typed tool_result chunk with camelCase fields', async () => {
    const client = new UdsBrainBackendClient(sockPath);
    const sessionId = (await client.createSession()).sessionId;
    const chunksPromise = collect(
      client.streamText({ sessionId, messages: [] }),
    );
    await new Promise((r) => setTimeout(r, 150));
    await client.resolveToolPermission('call_tr', true);
    const chunks = await chunksPromise;
    const tr = chunks.find((c) => c.type === 'tool_result') as
      | { callId?: string; output?: string; isError?: boolean; exitCode?: number; sequence?: number }
      | undefined;
    expect(tr).toBeDefined();
    expect(tr!.callId).toBe('call_tr');
    expect(tr!.output).toBe('hi\n');
    expect(tr!.isError).toBe(false);
    expect(tr!.exitCode).toBe(0);
    expect(tr!.sequence).toBe(3);
  });

  test('denied flow yields no tool_result chunk', async () => {
    const client = new UdsBrainBackendClient(sockPath);
    const sessionId = (await client.createSession()).sessionId;
    const chunksPromise = collect(client.streamText({ sessionId, messages: [] }));
    await new Promise((r) => setTimeout(r, 150));
    await client.resolveToolPermission('call_tr', false);
    const chunks = await chunksPromise;
    expect(chunks.some((c) => c.type === 'tool_result')).toBe(false);
    expect(chunks.some((c) => c.type === 'finished')).toBe(true);
  });
});
```

If `UdsBrainBackendClient`'s constructor signature differs (check how `AppShell` constructs it — it reads `BRAIN_SOCKET_PATH` from env by default), adapt construction: either pass the socket path explicitly if supported, or set `process.env.BRAIN_SOCKET_PATH` before constructing. Mirror what `resolvePermissionWire.test.ts` does with `LiveUdsClient(sockPath)` — that file constructs with a positional path, so the constructor accepts one.

- [ ] **Step 3: Run to RED**

Run:
```bash
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/client/toolResultWire.test.ts 2>&1 | tail -6'
```
Expected: the granted test FAILS (no `tool_result` chunk found — parser drops the frame today); denied passes trivially.

- [ ] **Step 4: Implement the parser branch**

In `UdsBrainBackendClient.ts`, immediately after the `tool_permission_requested` branch (around line 253–265), add:

```ts
        } else if (raw.type === 'tool_result') {
          pushChunk({
            type: 'tool_result',
            callId: raw.callId ?? raw.call_id,
            toolName: raw.toolName ?? raw.tool_name,
            output: typeof raw.output === 'string' ? raw.output : '',
            isError: Boolean(raw.is_error),
            exitCode: typeof raw.exit_code === 'number' ? raw.exit_code : undefined,
            generationId: chunkGenId,
            sessionId: chunkSessionId,
            sequence: raw.sequence,
          });
        }
```

- [ ] **Step 5: Run to GREEN, then commit**

Run:
```bash
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/client/toolResultWire.test.ts 2>&1 | tail -4'
```
Expected: 2 pass.

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add packages/brain-shell/src/client/BrainBackendClient.ts packages/brain-shell/src/client/UdsBrainBackendClient.ts packages/brain-shell/src/test/client/toolResultWire.test.ts
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "feat(brain-shell): parse tool_result stream frames from the daemon

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Adapter mapping + card output rendering

**Files:**
- Modify: `packages/brain-shell/src/adapter/chunkToTurnEvents.ts`
- Modify: `packages/brain-shell/src/contracts/messages.ts` (`ToolCardData`)
- Modify: `packages/brain-shell/src/ui/transcript/toRows.ts` (`toolCard`)
- Modify: `packages/brain-shell/src/ui/transcript/MessageRow.tsx` (`ToolRowView`)
- Test: `packages/brain-shell/src/test/adapter/toolResultEvents.test.ts`
- Test: `packages/brain-shell/src/test/ui/transcript/toolRowOutput.test.tsx`

**Interfaces:**
- Consumes: Task 3's `'tool_result'` chunk shape.
- Produces: turn event `{ type:'tool_result'; callId: string; output: string; isError?: boolean }` (union member already exists in `BrainTurnEvents.ts`); `ToolCardData` extended with `output?: string; isError?: boolean`.

- [ ] **Step 1: Failing adapter test**

Create `packages/brain-shell/src/test/adapter/toolResultEvents.test.ts`:

```ts
import { describe, expect, test } from 'bun:test';
import { chunkToTurnEvent } from '../../adapter/chunkToTurnEvents.js';

describe('tool_result chunk mapping', () => {
  test('maps to the existing tool_result turn event', () => {
    const event = chunkToTurnEvent({
      type: 'tool_result',
      callId: 'call_tr',
      output: 'hi\n',
      isError: false,
    });
    expect(event).toEqual({ type: 'tool_result', callId: 'call_tr', output: 'hi\n', isError: false });
  });

  test('missing output maps to empty string, never null', () => {
    const event = chunkToTurnEvent({ type: 'tool_result', callId: 'c2' });
    expect(event).not.toBeNull();
    expect((event as { output: string }).output).toBe('');
  });
});
```

Run `bun test src/test/adapter/toolResultEvents.test.ts` → expect FAIL (unknown chunk type returns null today).

- [ ] **Step 2: Map the chunk**

In `chunkToTurnEvents.ts`, add before `case 'error':`:

```ts
    case 'tool_result':
      return typeof chunk.callId === 'string'
        ? {
            type: 'tool_result',
            callId: chunk.callId,
            output: typeof chunk.output === 'string' ? chunk.output : '',
            isError: chunk.isError === true ? true : undefined,
          }
        : null;
```

Run the adapter test → PASS.

- [ ] **Step 3: Failing projection + render tests**

Create `packages/brain-shell/src/test/ui/transcript/toolRowOutput.test.tsx` following the plain-function `textOf` walk convention from `src/test/ui/overlays/permissionDialogView.test.tsx` (copy its `textOf` helper verbatim; import PALETTES for tokens):

```tsx
import { describe, expect, test } from 'bun:test';
import { PALETTES } from '../../../state/palettes.js';
import { ToolRowView } from '../../../ui/transcript/MessageRow.js';

const tokens = PALETTES.auto.resolve(false) ?? PALETTES.dark.resolve(false);

function row(output?: string, isError?: boolean) {
  return {
    kind: 'tool' as const,
    id: 't1',
    tool: {
      callId: 'call_tr',
      toolName: 'bash',
      input: { command: 'echo hi' },
      status: 'completed' as const,
      output,
      isError,
    },
  };
}

describe('ToolRowView output rendering', () => {
  test('collapsed shows a single truncated preview line', () => {
    const el = ToolRowView({
      row: row('x'.repeat(200)),
      expanded: false,
      tokens,
    });
    const text = textOf(el);
    expect(text).toContain('x'.repeat(120));
    expect(text).not.toContain('x'.repeat(121));
  });

  test('expanded shows the full output after the input json', () => {
    const el = ToolRowView({ row: row('line1\nline2'), expanded: true, tokens });
    const text = textOf(el);
    expect(text).toContain('line1\nline2');
    expect(text.indexOf('"command"')).toBeLessThan(text.indexOf('line1'));
  });

  test('no output renders nothing beyond the status line', () => {
    const el = ToolRowView({ row: row(undefined), expanded: false, tokens });
    const text = textOf(el);
    expect(text).not.toContain('[truncated]');
    expect(text).toContain('Done');
  });
});
```

If `PALETTES.auto.resolve` does not exist, inspect `state/palettes.js` exports and construct tokens the same way `permissionDialogView.test.tsx` obtains them (mirror it exactly — that file compiles against the real API).

Also append a projection test to the SAME file (or inline above the describe) asserting `turnToRows` carries output through:

```ts
import { turnToRows } from '../../../ui/transcript/toRows.js';

test('toolCard projection preserves output and isError', () => {
  const vmTurn = {
    id: 'turn_1',
    content: '',
    thinking: undefined,
    tools: [{
      callId: 'call_tr', toolName: 'bash',
      input: { command: 'echo hi' }, status: 'completed',
      output: 'hi\n', isError: false,
    }],
  };
  const rows = turnToRows(vmTurn as never);
  const tool = rows.find((r) => r.kind === 'tool') as { tool: { output?: string; isError?: boolean } };
  expect(tool.tool.output).toBe('hi\n');
  expect(tool.tool.isError).toBe(false);
});
```

Run both new test files → expect FAIL on projection (field dropped by `toolCard`) and rendering (no output shown).

- [ ] **Step 4: Contract + projection + render implementation**

In `contracts/messages.ts`, extend `ToolCardData`:

```ts
export interface ToolCardData {
  callId: string;
  toolName: string;
  input: Record<string, unknown>;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'denied' | 'cancelled';
  durationMs?: number;
  /** Terminal output carried from a delivered tool_result (Inc 5). */
  output?: string;
  isError?: boolean;
}
```

In `toRows.ts` `toolCard`, append two lines to the returned object:

```ts
    output: t.output,
    isError: t.isError,
```

(If `ToolExecutionView` lacks `output`/`isError` fields, check `adapter/BrainViewModels.ts` — the transformer writes `output: event.output, isError: event.isError` onto its tool state (see `BrainTurnTransformer.ts` ~line 195–205), so the view model already carries them; add them to `ToolExecutionView` only if genuinely absent.)

In `MessageRow.tsx` `ToolRowView`, replace the `<Text>` block containing `⎿ {meta.glyph}` with:

```tsx
      <Text>
        {'  '}
        <Text color={statusColor}>⎿ {meta.glyph}</Text>
        {expanded ? (
          <Text color={tokens.subtle}>
            {'\n     '}
            {JSON.stringify(t.input, null, 2).split('\n').join('\n     ')}
            {typeof t.output === 'string' && t.output.length > 0
              ? `\n     ── output ──\n     ${t.output.split('\n').join('\n     ')}`
              : ''}
          </Text>
        ) : (
          <>
            <Text color={tokens.subtle}>{` ${meta.label}`}</Text>
            {typeof t.output === 'string' && t.output.length > 0 ? (
              <Text color={tokens.subtle}>
                {'\n     '}
                {t.output.trimStart().split('\n')[0]!.slice(0, 120)}
              </Text>
            ) : null}
          </>
        )}
      </Text>
```

- [ ] **Step 5: Controller settlement stays duplicate-free (regression test)**

Append to `packages/brain-shell/src/test/state/sessionControllerPermission.test.ts` (same `scriptFake` helper, same file conventions):

```ts
test('delivered tool_result settles the card without duplicate settlement', async () => {
  const ctl = new SessionController(
    scriptFake([
      { type: 'tool_use', toolUse: { id: 'call_r', name: 'bash', input: { command: 'echo hi' } } },
      { type: 'tool_result', callId: 'call_r', toolName: 'bash', output: 'hi\n', isError: false },
      { type: 'finished', status: 'completed' },
    ]),
  );
  await ctl.submit('go');
  const serialized = JSON.stringify(ctl.getSnapshot().rows);
  expect(serialized).toContain('hi');
  expect(serialized.match(/── output ──/g)?.length ?? 0).toBeLessThanOrEqual(1);
});
```

Run the full shell suite:

```bash
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test 2>&1 | tail -4'
```
Expected: prior counts + all new tests green; failures stay exactly the five documented baselines (visualCellParity ×2, sessionSemanticIntegration, brainMemoryIntegration, brainTurnTransformer).

- [ ] **Step 6: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add packages/brain-shell/src/adapter/chunkToTurnEvents.ts packages/brain-shell/src/contracts/messages.ts packages/brain-shell/src/ui/transcript/toRows.ts packages/brain-shell/src/ui/transcript/MessageRow.tsx packages/brain-shell/src/test/adapter/toolResultEvents.test.ts packages/brain-shell/src/test/ui/transcript/toolRowOutput.test.tsx packages/brain-shell/src/test/state/sessionControllerPermission.test.ts
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "feat(brain-shell): render tool_result output on the card

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: PTY smoke + full gates

**Files:**
- Create: `scripts/ptySmokeInc5.py`
- Create fixtures under: `packages/brain-shell/src/test/fixtures/pty/inc5/` (generated by the run)

**Interfaces:**
- Consumes: everything shipped in Tasks 1–4; the Inc 4 smoke discipline INCLUDING the occurrence-count rule for repeated UI (`clean(buf).count(needle) >= N`).
- Produces: exit-0 proof that an allowed bash call's REAL stub-provided output text appears on the collapsed card, and the denied flow stays output-free.

- [ ] **Step 1: Write `scripts/ptySmokeInc5.py`**

Model on `scripts/ptySmokeInc4.py` with these deltas (everything else verbatim — winsize ioctl, per-keystroke writes ≥0.3 s apart, ANSI-stripped matching, teardown):

Constants:

```python
SOCK = "/tmp/brain-inc5-smoke.sock"
FRAMES_FILE = "/tmp/brain-inc5-smoke-requests.jsonl"
FIXTURE_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/src/test/fixtures/pty/inc5"
CONFIG_FILE = "/tmp/brain-inc5-smoke-config.json"
```

Stub stream branch — after emitting `tool_permission_requested(seq 1)` and parking on `PERM_EVENTS`, the resolution handler behaves exactly like Inc 4, but the GRANTED continuation now emits a tool_result before the closing frames:

```python
                    elif act == "v1/generation/stream":
                        reply({"type": "tool_use", "toolUse": {"id": "call_9",
                               "name": "bash", "input": {"command": "echo hello-from-stub"}},
                               "sequence": 0})
                        time.sleep(0.2)
                        reply({"type": "tool_permission_requested", "call_id": "call_9",
                               "tool_name": "bash", "input": {"command": "echo hello-from-stub"},
                               "reason": "shell access", "sequence": 1})
                        evt = threading.Event()
                        PERM_EVENTS["call_9"] = evt
                        resolved = evt.wait(timeout=10)
                        granted = bool(resolved and PERM_GRANTED.get("call_9"))
                        if granted:
                            reply({"type": "tool_result", "call_id": "call_9",
                                   "tool_name": "bash", "output": "hello-from-stub\n",
                                   "is_error": False, "exit_code": 0, "sequence": 2})
                            time.sleep(0.2)
                            reply({"type": "token", "token": "Done.", "sequence": 3})
                        else:
                            reply({"type": "tool_denied", "call_id": "call_9",
                                   "tool_name": "bash", "sequence": 2})
                        time.sleep(0.3)
                        reply({"type": "finished", "status": "completed", "sequence": 4})
```

Flow D1 (allow) — identical skeleton to Inc 4's D1 but asserting the NEW visible outcome:

```python
os.write(fd, b"say hello from stub")
pump(0.3)
os.write(fd, b"\r")
ok &= expect("tool-card", "bash")
ok &= expect_count("dialog-header", "Permission required", 1)
snapshot("permissionAllow-pending")
os.write(fd, b"y")

deadline = time.time() + 6
wire_allow = False
while time.time() < deadline:
    pump(0.1)
    if '"v1/tool/resolve"' in frames_log():
        wire_allow = True
        break
print(("PASS" if wire_allow else "FAIL") + " resolve-on-wire")
ok &= wire_allow
ok &= expect("card-output-preview", "hello-from-stub")
ok &= expect("done-token", "Done.")
snapshot("permissionAllow-done")
```

Flow D2 (deny) — identical to Inc 4's D2 including `expect_count("dialog-header-2", "Permission required", 2)` before sending `n`, plus one negative assertion after `denied-notice`:

```python
seen_deny = clean(buf)
print(("PASS" if seen_deny.count("should-not-appear") == 0 else "FAIL") + " no-output-on-deny")
ok &= seen_deny.count("should-not-appear") == 0
```

For the deny prompt use `"now delete it"` exactly as Inc 4 (the stub ignores prompt text; the negative assertion guards against any accidental output rendering on the denial path — the sentinel string `should-not-appear` must simply never exist in the buffer).

- [ ] **Step 2: Run the smoke and iterate to green**

Run: `python3 scripts/ptySmokeInc5.py` (Bash timeout ≥ 120000 ms).
Expected: all expects PASS incl. `resolve-on-wire`, `card-output-preview`, `deny-on-wire`, `no-output-on-deny`; exit 0; four fixture files under `inc5/`.
Known pitfall (carried): the smoke MUTATES tracked fixture snapshots — after the final gate run, restore with `git checkout -- packages/brain-shell/src/test/fixtures/pty/inc5/` before committing only if the files were previously committed; on FIRST commit the freshly generated files ARE the commit content.

- [ ] **Step 3: Full gates**

```bash
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test 2>&1 | tail -4'
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test --test uds_tool_execution_tests --test uds_permission_roundtrip_tests --test uds_generation_tests 2>&1 | grep "test result"'
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun build src/main.tsx --outdir dist --target bun >/dev/null 2>&1 && echo BUILD_OK'
git -C /Users/ritikpathania/Developer/PyCharm/brain diff main..HEAD -- packages/brain-shell/src/ | grep '^+' | grep -icE 'claude|anthropic|vendor'
git -C /Users/ritikpathania/Developer/PyCharm/brain diff main..HEAD -- packages/brain-shell/src/ ':!packages/brain-shell/src/test' | grep '^+' | grep -icE 'claude|anthropic|vendor'
```

Expected: shell suite green except the five documented baselines; Rust three suites `ok.` (10 tests total); BUILD_OK; vendor scans 0.

- [ ] **Step 4: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add scripts/ptySmokeInc5.py packages/brain-shell/src/test/fixtures/pty/inc5/
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "test(brain-shell): Inc 5 PTY smoke proves tool output on the card

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Self-review record

- **Spec coverage:** §4.1 BashTool → Task 1 (incl. truncation helper and stderr ordering exactly as amended). §4.2 ToolStack → Task 1 Step 3. §4.3 post-grant branch + unification grant + frame shape → Task 2 Step 3 (insertion point named; variable-scope adjustments enumerated). §4.4 client union/parser → Task 3; adapter mapping + card display (spec's corrected renderer subsection) → Task 4 Steps 2/4; settlement non-duplication → Task 4 Step 5. §5 error table rows covered by Task 1 unit tests (invalid arg), Task 2 tests (unknown tool, exit 3), executor policies (timeout/cancel produce Err → is_error frame, asserted indirectly by the Err arm; direct timeout test deliberately omitted as a 30 s wall-clock test). §6 testing strategy mapped across Tasks 1/2/3/4/5. §7 constraints restated in Global Constraints.
- **Placeholder scan:** no TBD/TODO; every code step carries full file content or exact insertion text; the one intentional sketch (`run_turn_resolving`) ships with its known flaw called out and the required restructure stated in the implementation note beneath it.
- **Type consistency:** `tool_stack() -> &'static Arc<ToolStack>` used identically in Task 1 (definition) and Task 2 (call site). Chunk field names `output/isError/exitCode` consistent across Task 3 producer and Task 4 consumer; wire keys `output/is_error/exit_code` consistent between contract table, Task 2 emitter, Task 3 parser, Task 5 stub. Turn-event key set matches the existing `BrainTurnEvents.ts` union verbatim.
- **Known risk recorded:** `run_turn_resolving`'s second-connection reader handling is spelled out as a restructure requirement rather than compilable code — the executor must keep both halves alive through the verdict read, per the note.
- **Signatures verified against source** (crates/brain-tools/src/lib.rs, crates/brain-core/src/extensibility.rs, crates/brain-domain/src/identifiers.rs): `register_tool(Arc<dyn Tool>) -> Result<(), BrainError>` (so `.expect` is valid); `get_tool -> Option<Arc<dyn Tool>>`; registry methods are **trait** methods requiring `use brain_core::extensibility::ToolRegistry` at every call site (baked into Tasks 1 and 2); `ToolExecutor::new(Arc<dyn ToolRunner>)`, async `execute` validating permissions FIRST (handler grants before executing — order matters); `Default` exists for PermissionManager/ToolRegistryImpl/CancellationTokenImpl; `ToolMetadata`'s 10 fields and `ExecutionContext`'s 4 match the plan's struct literals exactly; `SessionId::new()` exists and is `Copy`; `BrainError` lives at `brain_core::errors`. Still mirror-on-sight: PALETTES token construction and `UdsBrainBackendClient` construction arity (each carries an explicit instruction to copy an existing test file's pattern).
