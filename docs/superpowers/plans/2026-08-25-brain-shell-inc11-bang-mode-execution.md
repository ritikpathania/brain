# Inc 11 Bang-Mode Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the advertised `!` bash mode execute commands through the daemon's existing ToolStack as standalone persisted turns.

**Architecture:** One new UDS action `v1/shell/exec` in the daemon validates → grants `Permission::Shell` (keystroke-as-grant) → persists the user line → executes via the shared `ToolStack.executor` → responds once with Inc 8 envelope vocabulary → persists the executed envelope. The shell fires it on a short-lived socket via the existing `callRpc` template, renders the result through the same transformer/projection machinery live turns use, and never touches the stream connection. Replay of these turns already works at HEAD — zero replay-code changes.

**Tech Stack:** Rust daemon (tokio UDS handlers, brain-tools executor stack), Bun + TypeScript shell (React 19 / Ink 7), Python PTY harness.

**Spec:** `docs/superpowers/specs/2026-08-25-brain-shell-inc11-bang-mode-execution-design.md` — read it first; this plan argues from its sections (§ references below).

## Global Constraints

- Branch: `feature/brain-shell-inc11-bang-mode-execution` from `main @ 05c5051b`.
- Every cargo invocation needs the macOS rpath wrapper:
  `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo ...'`
- Daemon package name is **`brain-daemon`**, not `daemon`.
- Working tree carries ~1k files of pre-existing user WIP (including deletions/untracked files under `docs/superpowers/specs/`): stage **explicitly named paths only**; NEVER stash; never wholesale-checkout; never discard Cargo.lock.
- Commits: explicit-path `git add <paths>`; trailer `Co-Authored-By: Claude <noreply@anthropic.com>`; known-harmless noise: `error: daemon terminated` around git ops, CRLF fixture warnings.
- Baselines that must hold at every gate: bun shell suite 242 pass / 5 documented environmental fails; `uds_feedback_loop_tests` 6/6; brain-tools integration 6; PTY smoke inc9 15/15. Sole permitted failure remains the pre-existing untracked `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers`.
- Vendor scan after final task: `git diff <spec-commit>..HEAD -- crates daemon packages scripts | grep '^+' | grep -icE "anthropic|api\.anthropic|claude"` where `<spec-commit>` = `05c5051b` → expect `0`.

---

### Task 1: Daemon `v1/shell/exec` action

**Files:**
- Modify: `daemon/src/transport/uds/handlers.rs` (insert one dispatch arm above the `model/resolve` arm)
- Test: `daemon/tests/uds_feedback_loop_tests.rs` (append helper + five tests)

**Interfaces:**
- Consumes: `crate::tools::tool_stack()` → `ToolStack { registry: ToolRegistryImpl, permissions: PermissionManager, executor: ToolExecutor }`; `ToolExecutor::execute(&self, tool: Arc<dyn Tool>, context: &ExecutionContext, permission_manager: &PermissionManager, arguments: &HashMap<String, serde_json::Value>) -> Result<ExecutionResult, BrainError>`; `truncate_tool_output(&str) -> String` (existing, same file); `get_generation_registry()` (existing, same file); `parse_session_id_flexible(&str) -> SessionId` (existing, same file); storage seam `let storage = app.runtime().sqlite_storage(); use brain_core::repositories::SessionRepository;` (exact idiom of `handlers.rs:1710–1711`).
- Produces (wire contract, spec §3.1): request `{"id","action":"v1/shell/exec","payload":{"session_id","command"}}` (accepts `sessionId` alias); versioned success `{"version":"1.0","type":"Response","id",…,"status":"success","body":{call_id,name,input,outcome:"executed",output,is_error,exit_code,duration_ms}}`; versioned errors `{"version":"1.0","type":"Error","id","status":"error","body":"<message>"}` — body MUST be a plain string so generic RPC clients surface it readably. Unversioned twins follow the file's `{"status":"ok"/"error","body"/"message"}` duality.

- [ ] **Step 1: Write the failing integration tests**

Append to `daemon/tests/uds_feedback_loop_tests.rs` (helpers `start_test_daemon`, `open_and_create_session`, `send_frame`, `read_line_frame`, `load_session_messages` already exist in this file):

```rust
/// Sends one `v1/shell/exec` request on its own short-lived connection and
/// returns the single reply frame.
async fn exec_shell_command(
    socket_path: &std::path::Path,
    session_id: &str,
    command: &str,
) -> serde_json::Value {
    let conn = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = conn.into_split();
    let mut buf = BufReader::new(reader);
    send_frame(
        &mut writer,
        &serde_json::json!({
            "version": "1.0",
            "type": "Request",
            "id": "req-exec",
            "action": "v1/shell/exec",
            "payload": { "session_id": session_id, "command": command }
        }),
    )
    .await;
    read_line_frame(&mut buf).await
}

#[tokio::test]
async fn shell_exec_runs_command_and_persists_standalone_turn() {
    let proc = start_test_daemon(&[]).await;
    let (_r, _w, session_id) = open_and_create_session(&proc.socket_path).await;

    let reply = exec_shell_command(&proc.socket_path, &session_id, "echo bang-inc11").await;

    assert_eq!(reply["status"], "success");
    let body = &reply["body"];
    assert_eq!(body["name"], "bash");
    assert_eq!(body["input"]["command"], "echo bang-inc11");
    assert_eq!(body["outcome"], "executed");
    assert_eq!(body["output"], "bang-inc11\n");
    assert_eq!(body["is_error"], false);
    assert_eq!(body["exit_code"], 0);
    assert!(body["duration_ms"].is_u64());

    // Standalone turn persisted: user line (verbatim, with `!`) + envelope.
    let messages = load_session_messages(&proc.socket_path, &session_id).await;
    assert_eq!(messages.len(), 2, "messages: {messages:?}");
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["content"], "! echo bang-inc11");
    assert_eq!(messages[1]["role"], "tool");
    let env: serde_json::Value =
        serde_json::from_str(messages[1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(env["type"], "tool_event");
    assert_eq!(env["v"], 1);
    assert_eq!(env["name"], "bash");
    assert_eq!(env["input"]["command"], "echo bang-inc11");
    assert_eq!(env["outcome"], "executed");
    assert_eq!(env["exit_code"], 0);
    assert_eq!(env["output"], "bang-inc11\n");
    assert!(env["duration_ms"].is_u64());
}

#[tokio::test]
async fn shell_exec_nonzero_exit_is_success_with_error_fields() {
    let proc = start_test_daemon(&[]).await;
    let (_r, _w, session_id) = open_and_create_session(&proc.socket_path).await;

    // Real BashTool runs `/bin/bash -c false` -> exit 1.
    let reply = exec_shell_command(&proc.socket_path, &session_id, "false").await;

    assert_eq!(reply["status"], "success");
    assert_eq!(reply["body"]["exit_code"], 1);
    assert_eq!(reply["body"]["is_error"], true);

    let messages = load_session_messages(&proc.socket_path, &session_id).await;
    let env: serde_json::Value =
        serde_json::from_str(messages[1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(env["outcome"], "executed");
    assert_eq!(env["exit_code"], 1);
    assert_eq!(env["is_error"], true);
}

#[tokio::test]
async fn shell_exec_rejects_empty_command_without_touching_the_transcript() {
    let proc = start_test_daemon(&[]).await;
    let (_r, _w, session_id) = open_and_create_session(&proc.socket_path).await;

    let reply = exec_shell_command(&proc.socket_path, &session_id, "   ").await;

    assert_eq!(reply["status"], "error");
    let messages = load_session_messages(&proc.socket_path, &session_id).await;
    assert_eq!(messages.len(), 0, "nothing persisted: {messages:?}");
}

#[tokio::test]
async fn shell_exec_rejects_unknown_session() {
    let proc = start_test_daemon(&[]).await;
    let reply = exec_shell_command(&proc.socket_path, "no-such-session", "echo hi").await;
    assert_eq!(reply["status"], "error");
}

#[tokio::test]
async fn shell_exec_rejects_while_a_generation_is_active() {
    let proc = start_test_daemon(&[("BRAIN_MOCK_SCRIPTED_RESPONSES", TWO_ROUND_SCRIPT)]).await;
    let (mut reader, mut writer, session_id) = open_and_create_session(&proc.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, "occupying").await;
    // The generation registers in the active-generation map BEFORE its first
    // wire frame, so one received frame proves the entry exists.
    let _first =
        tokio::time::timeout(Duration::from_secs(15), read_line_frame(&mut reader))
            .await
            .expect("first stream frame");
    let reply = exec_shell_command(&proc.socket_path, &session_id, "echo nope").await;
    assert_eq!(reply["status"], "error");
    // Let the generation finish cleanly so process drop isn't mid-write.
    run_turn_resolving(&mut reader, &proc.socket_path, true).await;
}
```

- [ ] **Step 2: Run to verify they fail**

Run:
```bash
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --test uds_feedback_loop_tests shell_exec -- --nocapture'
```
Expected: all five FAIL — unknown actions fall through to the generic reply, so `status` is not `"success"` / transcript assertions see zero new messages.

- [ ] **Step 3: Implement the dispatch arm**

In `daemon/src/transport/uds/handlers.rs`, insert this arm **immediately above** the line `        if action == "model/resolve" || action == "v1/model/resolve" {`:

```rust
        // ── Inc 11: user-initiated shell passthrough (`!`) ──────────────
        // One command per short-lived connection. Validate → grant →
        // persist the user line → execute through THE SAME ToolStack as
        // agentic calls → respond once → persist the executed envelope.
        // Nothing here reaches a provider, and BashTool remains the only
        // code path that spawns /bin/bash.
        if action == "shell/exec" || action == "v1/shell/exec" {
            let write_error = |msg: String| {
                // Error bodies stay plain strings so generic RPC clients
                // surface them readably (parsed.body feeds new Error()).
            };
            let exec_req: serde_json::Value = if payload.trim().is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::from_str(payload).unwrap_or(serde_json::Value::Null)
            };
            let command = exec_req["command"].as_str().unwrap_or("").trim().to_string();
            let sid_str = exec_req["session_id"]
                .as_str()
                .or_else(|| exec_req["sessionId"].as_str())
                .unwrap_or("")
                .to_string();
            if sid_str.is_empty() {
                let response = if is_versioned {
                    serde_json::json!({
                        "version": "1.0", "type": "Error", "id": req_id_val,
                        "status": "error", "body": "missing sessionId"
                    })
                } else {
                    serde_json::json!({ "status": "error", "message": "missing sessionId" })
                };
                let mut rj = serde_json::to_string(&response)?;
                rj.push('\n');
                let mut w = writer.lock().await;
                w.write_all(rj.as_bytes()).await?;
                w.flush().await?;
                continue;
            }
            let parsed_session_id = parse_session_id_flexible(&sid_str);

            let storage = app.runtime().sqlite_storage();
            use brain_core::repositories::SessionRepository;
            let mut session_aggregate = match storage.load_session(&parsed_session_id) {
                Ok(Some(s)) => s,
                _ => {
                    let msg = format!("Session '{}' not found", sid_str);
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0", "type": "Error", "id": req_id_val,
                            "status": "error", "body": msg
                        })
                    } else {
                        serde_json::json!({ "status": "error", "message": msg })
                    };
                    let mut rj = serde_json::to_string(&response)?;
                    rj.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(rj.as_bytes()).await?;
                    w.flush().await?;
                    continue;
                }
            };

            if command.is_empty() {
                let response = if is_versioned {
                    serde_json::json!({
                        "version": "1.0", "type": "Error", "id": req_id_val,
                        "status": "error", "body": "command must be a non-empty string"
                    })
                } else {
                    serde_json::json!({ "status": "error", "message": "command must be a non-empty string" })
                };
                let mut rj = serde_json::to_string(&response)?;
                rj.push('\n');
                let mut w = writer.lock().await;
                w.write_all(rj.as_bytes()).await?;
                w.flush().await?;
                continue;
            }

            // Backstop mirroring the stream arm's Invariant 8: the two arms
            // must never interleave save_session writes to one aggregate.
            {
                let registry = get_generation_registry();
                let reg = registry.read().await;
                if reg.values().any(|active| active.session_id == parsed_session_id) {
                    let msg = format!(
                        "Session '{}' is busy with an active generation",
                        sid_str
                    );
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0", "type": "Error", "id": req_id_val,
                            "status": "error", "body": msg
                        })
                    } else {
                        serde_json::json!({ "status": "error", "message": msg })
                    };
                    let mut rj = serde_json::to_string(&response)?;
                    rj.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(rj.as_bytes()).await?;
                    w.flush().await?;
                    continue;
                }
            }

            // Keystroke-as-grant (spec §2): the user typed and submitted this
            // command, which IS the consent act. This grant replaces the
            // dialog, not the gate — execution still flows through
            // ToolExecutor::validate_tool_permissions. Verified inert for the
            // agentic round-trip: handlers.rs has NO is_granted precheck, so
            // agentic calls always prompt regardless of grant-set state.
            crate::tools::tool_stack()
                .permissions
                .grant(brain_core::extensibility::Permission::Shell);

            // Persist the attempted command upon acceptance (stream-arm
            // Invariant 4 twin): a crash mid-exec still leaves it recorded.
            let _ = session_aggregate.add_message(brain_domain::Message::new(
                brain_domain::MessageId::new(),
                brain_domain::MessageRole::User,
                format!("! {}", command),
            ));
            let _ = storage.save_session(&parsed_session_id, &session_aggregate);

            use brain_core::extensibility::{
                ExecutionContext as ToolExecutionContext,
                ToolRegistry,
            };
            let stack = crate::tools::tool_stack();
            let mut args_map: std::collections::HashMap<String, serde_json::Value> =
                std::collections::HashMap::new();
            args_map.insert("command".to_string(), serde_json::json!(command));
            let tool_ctx = ToolExecutionContext {
                session_id: parsed_session_id.clone(),
                working_dir: std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from(".")),
                cancellation: Arc::new(brain_tools::CancellationTokenImpl::default()),
                deadline: None,
            };
            let tool_exec_started = std::time::Instant::now();

            match stack.registry.get_tool("bash") {
                Some(tool) => {
                    match stack
                        .executor
                        .execute(tool, &tool_ctx, &stack.permissions, &args_map)
                        .await
                    {
                        Ok(result) => {
                            let v = result.value();
                            let out_text = v["output"].as_str().unwrap_or("").to_string();
                            let is_err = v["is_error"].as_bool().unwrap_or(true);
                            let exit_code = v["exit_code"].as_i64().unwrap_or(-1);
                            let duration_ms =
                                tool_exec_started.elapsed().as_millis() as u64;
                            let call_id =
                                format!("shell-{}", uuid::Uuid::new_v4().simple());
                            let resp_body = serde_json::json!({
                                "call_id": call_id,
                                "name": "bash",
                                "input": { "command": command },
                                "outcome": "executed",
                                "output": out_text,
                                "is_error": is_err,
                                "exit_code": exit_code,
                                "duration_ms": duration_ms,
                            });
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0", "type": "Response",
                                    "id": req_id_val, "status": "success",
                                    "body": resp_body
                                })
                            } else {
                                serde_json::json!({ "status": "ok", "body": resp_body })
                            };
                            let mut rj = serde_json::to_string(&response)?;
                            rj.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(rj.as_bytes()).await?;
                            w.flush().await?;
                            drop(w);

                            // Persist the standard executed-case envelope
                            // (byte-identical schema to the agentic site).
                            let envelope = serde_json::json!({
                                "type": "tool_event",
                                "v": 1,
                                "call_id": call_id,
                                "name": "bash",
                                "input": { "command": command },
                                "outcome": "executed",
                                "is_error": is_err,
                                "exit_code": exit_code,
                                "output": truncate_tool_output(&out_text),
                                "duration_ms": duration_ms,
                            });
                            let _ = session_aggregate.add_message(
                                brain_domain::Message::new(
                                    brain_domain::MessageId::new(),
                                    brain_domain::MessageRole::Tool,
                                    envelope.to_string(),
                                ),
                            );
                            if let Err(e) = storage.save_session(
                                &parsed_session_id,
                                &session_aggregate,
                            ) {
                                tracing::warn!(
                                    error = %e,
                                    "tool event persistence failed; continuing"
                                );
                            }
                        }
                        Err(e) => {
                            let msg = format!("shell exec failed: {}", e);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0", "type": "Error",
                                    "id": req_id_val, "status": "error",
                                    "body": msg
                                })
                            } else {
                                serde_json::json!({ "status": "error", "message": msg })
                            };
                            let mut rj = serde_json::to_string(&response)?;
                            rj.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(rj.as_bytes()).await?;
                            w.flush().await?;
                        }
                    }
                }
                None => {
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0", "type": "Error", "id": req_id_val,
                            "status": "error", "body": "bash tool is not registered"
                        })
                    } else {
                        serde_json::json!({ "status": "error", "message": "bash tool is not registered" })
                    };
                    let mut rj = serde_json::to_string(&response)?;
                    rj.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(rj.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }
```

Then DELETE the placeholder lines (they were scaffolding for readability of this listing only):
```rust
            let write_error = |msg: String| {
                // Error bodies stay plain strings so generic RPC clients
                // surface them readably (parsed.body feeds new Error()).
            };
```

Implementation notes:
- `writer.lock().await` (not bare `.write_all`) because this scope's `writer` is the shared mutex-guarded half — copy the `model/list` arm's idiom exactly.
- `truncate_tool_output` already exists in this file (Inc 8).
- If `parse_session_id_flexible` rejects `"no-such-session"` by minting a fresh id instead of failing, the `load_session` mismatch still produces the `_ =>` error branch — the unknown-session test passes either way.
- If rustc reports an unused-import for `ToolRegistry` inside the arm, remove just that import name (the `get_tool` trait method may resolve via the file's top-level imports).

- [ ] **Step 4: Run to verify they pass**

Same command as Step 2. Expected: all five PASS.

- [ ] **Step 5: Run the full daemon suite**

Run:
```bash
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon'
```
Expected: green; `uds_feedback_loop_tests` now 11/11; sole permitted failure remains the pre-existing security-audit test.

- [ ] **Step 6: Commit**

```bash
git add daemon/src/transport/uds/handlers.rs daemon/tests/uds_feedback_loop_tests.rs
git commit -m "feat(daemon): v1/shell/exec action routes ! commands through the shared bash tool stack

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Client `execShell` on the short-lived socket

**Files:**
- Modify: `packages/brain-shell/src/client/UdsBrainBackendClient.ts` (`callRpc` ~line 384; new method next to `resolveToolPermission` ~line 838)
- Modify: `packages/brain-shell/src/client/BrainBackendClient.ts` (interface + result type)
- Test: `packages/brain-shell/src/test/client/shellExecWire.test.ts` (new)

**Interfaces:**
- Consumes: `private callRpc<T>(action, payload?, timeoutMs?)` (widened in Step 1).
- Produces: `ShellExecResult { callId: string; name: string; input: Record<string, unknown>; outcome: 'executed'; output: string; isError: boolean; exitCode: number; durationMs?: number }`; `UdsBrainBackendClient.execShell(sessionId: string, command: string, signal?: AbortSignal): Promise<ShellExecResult>`; optional interface method `execShell?` (optional, like `resolveToolPermission?`, so legacy fakes/mock implementations stay valid). Task 3 consumes both.

- [ ] **Step 1: Write the failing wire test**

Create `packages/brain-shell/src/test/client/shellExecWire.test.ts`:

```ts
import { describe, expect, test, afterAll } from 'bun:test';
import * as net from 'node:net';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { UdsBrainBackendClient } from '../../client/UdsBrainBackendClient.js';

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-shell-exec-wire-'));
const sockPath = path.join(dir, 't.sock');

// Scripted daemon: echoes the exec payload back shaped like the daemon's
// success body; supports slow replies for abort testing.
const server = net.createServer((socket) => {
  let buffer = '';
  socket.on('data', (data) => {
    buffer += data.toString('utf8');
    let idx: number;
    while ((idx = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, idx);
      buffer = buffer.slice(idx + 1);
      if (!line.trim()) continue;
      const req = JSON.parse(line) as {
        action?: string;
        payload?: Record<string, unknown>;
      };
      const reply = (obj: unknown) => socket.write(JSON.stringify(obj) + '\n');
      if (req.action !== 'v1/shell/exec') return;
      const cmd = String(req.payload?.['command'] ?? '');
      if (cmd === 'slow') {
        setTimeout(() => {
          reply({
            version: '1.0',
            type: 'Response',
            id: 'x',
            status: 'success',
            body: {
              call_id: 'shell-slow',
              name: 'bash',
              input: { command: 'slow' },
              outcome: 'executed',
              output: 'late',
              is_error: false,
              exit_code: 0,
              duration_ms: 5,
            },
          });
        }, 1500);
        return;
      }
      if (cmd === 'boom') {
        reply({ version: '1.0', type: 'Error', id: 'x', status: 'error', body: 'shell exec failed: nope' });
        return;
      }
      reply({
        version: '1.0',
        type: 'Response',
        id: 'x',
        status: 'success',
        body: {
          call_id: 'shell-abc',
          name: 'bash',
          input: { command: cmd },
          outcome: 'executed',
          output: `${cmd}\n`,
          is_error: false,
          exit_code: 0,
          duration_ms: 12,
        },
      });
    }
  });
});
server.listen(sockPath);

afterAll(() => {
  server.close();
  fs.rmSync(dir, { recursive: true, force: true });
});

describe('UDS client execShell', () => {
  test('maps snake_case body to camelCase ShellExecResult', async () => {
    const client = new UdsBrainBackendClient(sockPath);
    const res = await client.execShell('sess-1', 'echo hi');
    expect(res.callId).toBe('shell-abc');
    expect(res.name).toBe('bash');
    expect(res.input).toEqual({ command: 'echo hi' });
    expect(res.outcome).toBe('executed');
    expect(res.output).toBe('hi\n');
    expect(res.isError).toBe(false);
    expect(res.exitCode).toBe(0);
    expect(res.durationMs).toBe(12);
  });

  test('error-status responses reject with the daemon message', async () => {
    const client = new UdsBrainBackendClient(sockPath);
    await expect(client.execShell('sess-1', 'boom')).rejects.toThrow('shell exec failed');
  });

  test('aborting the signal tears down the wait deterministically', async () => {
    const client = new UdsBrainBackendClient(sockPath);
    const ac = new AbortController();
    const pending = client.execShell('sess-1', 'slow', ac.signal);
    setTimeout(() => ac.abort(), 100);
    await expect(pending).rejects.toThrow(/abort/i);
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/client/shellExecWire.test.ts`
Expected: FAIL — `execShell is not a function` (method doesn't exist).

- [ ] **Step 3: Widen `callRpc` (timeout + abort signal)**

In `packages/brain-shell/src/client/UdsBrainBackendClient.ts`, change the signature at ~line 384:

```ts
  private async callRpc<T>(
    action: string,
    payload: any = {},
    timeoutMs = 10_000,
    signal?: AbortSignal,
  ): Promise<T> {
```

Replace the hard-coded timeout lines:

```ts
      socket.setTimeout(timeoutMs);
      socket.once('timeout', () => {
        finishError(new Error(`Brain daemon RPC timeout (${timeoutMs}ms) on ${action}`));
      });
```

Immediately after those lines, add abort wiring:

```ts
      if (signal) {
        if (signal.aborted) {
          finishError(new Error(`${action} aborted`));
          return;
        }
        signal.addEventListener(
          'abort',
          () => finishError(new Error(`${action} aborted`)),
          { once: true },
        );
      }
```

(`finishError` is guarded by the `resolved` flag, so a post-resolution abort is a no-op.)

- [ ] **Step 4: Add `execShell`**

Next to `resolveToolPermission` (~line 838):

```ts
  /**
   * Inc 11: executes one user-typed `!` command through the daemon's shared
   * bash tool stack on its own short-lived connection. The generous timeout
   * lets the executor's own 30 s policy bound win the race, never the socket.
   */
  async execShell(
    sessionId: string,
    command: string,
    signal?: AbortSignal,
  ): Promise<ShellExecResult> {
    const raw = await this.callRpc<any>(
      'v1/shell/exec',
      { session_id: sessionId, command },
      35_000,
      signal,
    );
    return {
      callId: typeof raw?.call_id === 'string' ? raw.call_id : '',
      name: typeof raw?.name === 'string' ? raw.name : 'bash',
      input:
        raw?.input && typeof raw.input === 'object'
          ? (raw.input as Record<string, unknown>)
          : {},
      outcome: 'executed',
      output: typeof raw?.output === 'string' ? raw.output : '',
      isError: raw?.is_error === true,
      exitCode: typeof raw?.exit_code === 'number' ? raw.exit_code : -1,
      durationMs: typeof raw?.duration_ms === 'number' ? raw.duration_ms : undefined,
    };
  }
```

Add `ShellExecResult` to the imports at the top of the file:

```ts
import type { ..., ShellExecResult } from './BrainBackendClient.js';
```
(extending whatever import list exists — do not duplicate an existing import of that module).

- [ ] **Step 5: Add the type + interface method in `BrainBackendClient.ts`**

Near the other exported interfaces (~line 420 region, beside `resolveToolPermission?`):

```ts
/** Result of one user-initiated `!` shell execution (Inc 11). Field
 * vocabulary mirrors the daemon's persisted tool_event envelope. */
export interface ShellExecResult {
  callId: string;
  name: string;
  input: Record<string, unknown>;
  outcome: 'executed';
  output: string;
  isError: boolean;
  exitCode: number;
  durationMs?: number;
}
```

And inside the `BrainBackendClient` interface, directly under `resolveToolPermission?(...)`:

```ts
  execShell?(sessionId: string, command: string, signal?: AbortSignal): Promise<ShellExecResult>;
```

- [ ] **Step 6: Run to verify it passes**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/client/shellExecWire.test.ts src/test/client/toolResultWire.test.ts src/test/client/resolvePermissionWire.test.ts`
Expected: all PASS (the two sibling suites prove the `callRpc` widening regressed nothing).

- [ ] **Step 7: Commit**

```bash
git add packages/brain-shell/src/client/UdsBrainBackendClient.ts packages/brain-shell/src/client/BrainBackendClient.ts packages/brain-shell/src/test/client/shellExecWire.test.ts
git commit -m "feat(shell-client): execShell rides a short-lived socket with abortable waits

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Controller `runShellCommand`

**Files:**
- Modify: `packages/brain-shell/src/state/sessionController.ts` (add method after `submit`/`finishTurn` region)
- Test: `packages/brain-shell/src/test/state/sessionControllerShellExec.test.ts` (new)

**Interfaces:**
- Consumes: `execShell(sessionId, command, signal?)` (Task 2); `BrainTurnTransformer.transform(events)`; `turnToRows(vm)`; existing privates `busy`, `rows`, `sessionId`, `sysSeq`, `notice`, `emit`.
- Produces: `controller.runShellCommand(command: string): Promise<void>` — Task 4 (AppShell routing) consumes it.

- [ ] **Step 1: Write the failing controller tests**

Create `packages/brain-shell/src/test/state/sessionControllerShellExec.test.ts`:

```ts
import { describe, it, expect } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import type {
  BrainBackendClient,
  CreateSessionResponse,
  ShellExecResult,
} from '../../client/BrainBackendClient.js';

function fakeExecClient(
  execImpl: (sessionId: string, command: string, signal?: AbortSignal) => Promise<ShellExecResult>,
  delayFirstTurn = false,
) {
  const client = {
    async createSession(): Promise<CreateSessionResponse> {
      return { sessionId: 'stub-session-x', title: 'stub', createdAtMs: 0 };
    },
    execShell,
  } as unknown as BrainBackendClient;
  function execShell(
    sessionId: string,
    command: string,
    signal?: AbortSignal,
  ): Promise<ShellExecResult> {
    return execImpl(sessionId, command, signal);
  }
  void delayFirstTurn;
  return client;
}

function ok(command: string, exitCode = 0, output = ''): ShellExecResult {
  return {
    callId: `shell-${command.length}`,
    name: 'bash',
    input: { command },
    outcome: 'executed',
    output: output || `${command}\n`,
    isError: exitCode !== 0,
    exitCode,
    durationMs: 42,
  };
}

describe('runShellCommand (Inc 11)', () => {
  it('pushes the user line, projects a completed card, restores idle', async () => {
    const ctl = new SessionController(fakeExecClient((_sid, cmd) => ok(cmd)));
    await ctl.runShellCommand('echo bang');

    const snap = ctl.getSnapshot();
    expect(snap.rows[0]).toMatchObject({ kind: 'user', text: '! echo bang' });
    const cardRow = snap.rows.find((r) => r.kind === 'tool');
    expect(cardRow).toBeDefined();
    if (cardRow?.kind === 'tool') {
      expect(cardRow.tool.toolName).toBe('bash');
      expect(cardRow.tool.status).toBe('completed');
      expect(cardRow.tool.output).toBe('echo bang\n');
      expect(cardRow.tool.durationMs).toBe(42);
      expect(cardRow.tool.exitCode).toBe(0);
    }
    expect(snap.busy).toBe(false);
    expect(snap.live.phase).toBe('idle');
  });

  it('projects a failed card carrying the real exit code', async () => {
    const ctl = new SessionController(fakeExecClient((_sid, cmd) => ok(cmd, 2, 'boom')));
    await ctl.runShellCommand('false');

    const snap = ctl.getSnapshot();
    const cardRow = snap.rows.find((r) => r.kind === 'tool');
    if (cardRow?.kind === 'tool') {
      expect(cardRow.tool.status).toBe('failed');
      expect(cardRow.tool.isError).toBe(true);
      expect(cardRow.tool.exitCode).toBe(2);
      expect(cardRow.tool.output).toBe('boom');
    } else {
      throw new Error('expected a tool row');
    }
  });

  it('rejects a second command while busy with a visible notice', async () => {
    let release!: (r: ShellExecResult) => void;
    const gate = new Promise<ShellExecResult>((res) => (release = res));
    const ctl = new SessionController(fakeExecClient(() => gate));

    const first = ctl.runShellCommand('sleepish');
    await new Promise((r) => setTimeout(r, 20)); // let busy flip
    expect(ctl.getSnapshot().busy).toBe(true);

    await ctl.runShellCommand('echo again'); // dropped with a notice
    const notices = ctl.getSnapshot().rows.filter((r) => r.kind === 'system');
    expect(notices.some((n) => n.kind === 'system' && n.text.includes('Busy'))).toBe(true);

    release(ok('sleepish'));
    await first;
    expect(ctl.getSnapshot().busy).toBe(false);
  });

  it('surfaces backend rejections as a notice and stays usable', async () => {
    const ctl = new SessionController(
      fakeExecClient(() => Promise.reject(new Error('Brain daemon RPC timeout (35000ms) on v1/shell/exec'))),
    );
    await ctl.runShellCommand('whatever');
    const snap = ctl.getSnapshot();
    expect(snap.rows.some((r) => r.kind === 'system' && r.text.includes('Could not run command'))).toBe(true);
    expect(snap.busy).toBe(false);
  });

  it('esc-abort during exec lands the cancelled notice', async () => {
    const ctl = new SessionController(
      fakeExecClient(
        (_sid, _cmd, signal) =>
          new Promise((_res, rej) => {
            signal?.addEventListener('abort', () => rej(new Error('v1/shell/exec aborted')), { once: true });
          }),
      ),
    );
    const pending = ctl.runShellCommand('long-runner');
    await new Promise((r) => setTimeout(r, 20));
    ctl.abort();
    await pending;
    const snap = ctl.getSnapshot();
    expect(snap.rows.some((r) => r.kind === 'system' && r.text.includes('cancelled'))).toBe(true);
    expect(snap.busy).toBe(false);
  });
});
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/state/sessionControllerShellExec.test.ts`
Expected: FAIL — `runShellCommand is not a function`.

- [ ] **Step 3: Implement `runShellCommand`**

In `packages/brain-shell/src/state/sessionController.ts`, add after the `resumeSession` method:

```ts
  /** Inc 11: `!` bash passthrough — a standalone turn that never reaches a
   * provider. Rendered through the same reducer/projection path as live
   * agentic cards, so local and replayed rows agree by construction. */
  async runShellCommand(command: string): Promise<void> {
    if (this.busy) {
      this.notice('Busy — wait for the current turn to finish.');
      return;
    }
    const trimmed = command.trim();
    if (trimmed.length === 0) return;
    this.busy = true;
    this.connectionError = undefined;
    const turnId = `turn_${++this.turnSeq}`;
    this.rows = [...this.rows, { kind: 'user', id: `user:${turnId}`, text: `! ${trimmed}` }];
    this.aborter = new AbortController();
    this.emit();
    try {
      if (this.sessionId === undefined) {
        this.sessionId = (await this.client.createSession()).sessionId;
      }
      const result = await this.client.execShell?.(this.sessionId, trimmed, this.aborter.signal);
      if (!result) {
        this.notice('This backend cannot execute shell commands.');
        return;
      }
      const callId = result.callId || `shell_${turnId}`;
      const vm = BrainTurnTransformer.transform([
        { type: 'tool_call_requested', callId, toolName: 'bash', input: result.input },
        {
          type: 'tool_result',
          callId,
          output: result.output,
          isError: result.isError,
          exitCode: result.exitCode,
          durationMs: result.durationMs,
        },
      ]);
      const projected = turnToRows(vm).filter(
        (r) => !(r.kind === 'assistant' && r.markdown.trim().length === 0),
      );
      this.rows = [...this.rows, ...projected];
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      this.notice(/abort/i.test(msg) ? 'Shell command cancelled.' : `Could not run command: ${msg}`);
    } finally {
      this.busy = false;
      this.aborter = null;
      this.emit();
    }
  }
```

- [ ] **Step 4: Run to verify they pass**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/state/`
Expected: all state-suite files PASS including the five new tests.

- [ ] **Step 5: Commit**

```bash
git add packages/brain-shell/src/state/sessionController.ts packages/brain-shell/src/test/state/sessionControllerShellExec.test.ts
git commit -m "feat(shell-state): runShellCommand renders ! results through the live-card pipeline

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Parity proof — local `!` card equals replayed card

**Files:**
- Test: `packages/brain-shell/src/test/adapter/shellExecParity.test.ts` (new)

**Interfaces:**
- Consumes: `BrainTurnTransformer.transform`, `turnToRows`, `sessionToRows(session)` where `session: BrainSession {id,title,createdAtMs,updatedAtMs,pinned,archived,messages:{id,role,content}[]}`.

- [ ] **Step 1: Write the parity test**

Create `packages/brain-shell/src/test/adapter/shellExecParity.test.ts`:

```ts
import { describe, it, expect } from 'bun:test';
import { BrainTurnTransformer } from '../../adapter/BrainTurnTransformer.js';
import type { BrainTurnEvent } from '../../adapter/BrainTurnEvents.js';
import { turnToRows } from '../../ui/transcript/toRows.js';
import { sessionToRows } from '../../state/sessionReplay.js';
import type { BrainSession } from '../../client/BrainBackendClient.js';

/**
 * Inc 11: the card runShellCommand projects locally must equal the frozen
 * card sessionToRows replays from the persisted envelope — the same
 * guarantee Inc 10 proved for agentic cards, extended to user-initiated
 * `!` turns.
 */
function envelope(command: string, exitCode: number, durationMs: number, output: string): string {
  return JSON.stringify({
    type: 'tool_event',
    v: 1,
    call_id: 'shell-parity',
    name: 'bash',
    input: { command },
    outcome: 'executed',
    is_error: exitCode !== 0,
    exit_code: exitCode,
    output,
    duration_ms: durationMs,
  });
}

function localCard(events: BrainTurnEvent[]) {
  const rows = turnToRows(BrainTurnTransformer.transform(events));
  const card = rows.find((r) => r.kind === 'tool');
  if (card?.kind !== 'tool') throw new Error('local side produced no tool row');
  return card.tool;
}

function replayedCard(command: string, exitCode: number, durationMs: number, output: string) {
  const session: BrainSession = {
    id: 's',
    title: 't',
    createdAtMs: 0,
    updatedAtMs: 0,
    pinned: false,
    archived: false,
    messages: [
      { id: 'm0', role: 'user', content: `! ${command}` },
      { id: 'm1', role: 'tool', content: envelope(command, exitCode, durationMs, output) },
    ],
  };
  const card = sessionToRows(session).find((r) => r.kind === 'tool');
  if (card?.kind !== 'tool') throw new Error('replay produced no tool row');
  return card.tool;
}

describe('shell-exec live/replay parity', () => {
  it('successful command: local projection deep-equals replayed envelope card', () => {
    const events: BrainTurnEvent[] = [
      { type: 'tool_call_requested', callId: 'shell-parity', toolName: 'bash', input: { command: 'echo bang' } },
      { type: 'tool_result', callId: 'shell-parity', output: 'bang\n', isError: false, exitCode: 0, durationMs: 88 },
    ];
    expect(localCard(events)).toEqual(replayedCard('echo bang', 0, 88, 'bang\n'));
  });

  it('failed command: both sides agree on failed status and exit code', () => {
    const events: BrainTurnEvent[] = [
      { type: 'tool_call_requested', callId: 'shell-parity', toolName: 'bash', input: { command: 'false' } },
      { type: 'tool_result', callId: 'shell-parity', output: '', isError: true, exitCode: 1, durationMs: 15 },
    ];
    expect(localCard(events)).toEqual(replayedCard('false', 1, 15, ''));
  });
});
```

If the deep-equality fails only on fields one side legitimately lacks (e.g., an optional `undefined` field serialized differently), normalize BOTH sides by deleting keys whose value is `undefined` before comparing — do not weaken the assertion otherwise.

- [ ] **Step 2: Run to verify**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/adapter/shellExecParity.test.ts`
Expected: PASS (both sides consume the same facts through the same renderers; this is a proof, expected green immediately — a failure here exposes a genuine asymmetry between `toRows` and `sessionReplay` that MUST be fixed, not papered over).

- [ ] **Step 3: Commit**

```bash
git add packages/brain-shell/src/test/adapter/shellExecParity.test.ts
git commit -m "test(shell): ! card parity between live projection and persisted replay

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: UI wiring — PromptInput mode pass-through, AppShell routing

**Files:**
- Modify: `packages/brain-shell/src/ui/composer/PromptInput.tsx:71` and `:144`
- Modify: `packages/brain-shell/src/ui/shell/AppShell.tsx:171–174`

**Interfaces:**
- Consumes: `controller.runShellCommand(command)` (Task 3).
- Produces: `PromptInput` prop signature `onSubmit: (value: string, mode: 'prompt' | 'bash') => void`. Verification here is compile + suite + the PTY end-to-end flow (Task 6); ink `useInput` cannot be driven from bun:test without a harness, matching repo precedent (`promptInputView.test.tsx` is a mount-smoke only).

- [ ] **Step 1: Widen the PromptInput callback**

In `packages/brain-shell/src/ui/composer/PromptInput.tsx`, change the prop declaration (~line 71):

```ts
  onSubmit: (value: string, mode: 'prompt' | 'bash') => void;
```

and the submit site (~line 144) — `wasBash` is already computed three lines above:

```ts
      props.onSubmit(bare, wasBash ? 'bash' : 'prompt');
```

- [ ] **Step 2: Route bash-mode submits in AppShell**

In `packages/brain-shell/src/ui/shell/AppShell.tsx`, replace `handleSubmit` (~lines 171–174):

```ts
  const handleSubmit = (text: string, mode: 'prompt' | 'bash' = 'prompt'): void => {
    if (mode === 'bash') void controller.runShellCommand(text);
    else if (text.trimStart().startsWith('/')) runCommand(text);
    else void controller.submit(text);
  };
```

(The default parameter keeps any other callers compiling unchanged.)

- [ ] **Step 3: Verify compile + suites**

Run:
```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bunx tsc --noEmit 2> "$CLAUDE_JOB_DIR/tmp/tsc-inc11.txt"; echo "REAL-TSC-EXIT=$?"; bun test
```
Expected: `REAL-TSC-EXIT=1` is the documented environmental baseline (missing @types/node etc.) — compare the error list against HEAD's baseline: zero NEW errors beyond the known MockBrainBackendClient/goals/BrainSessionMessage set. `bun test`: 242 prior passes + Tasks 2–4 additions, 5 documented environmental fails, no new failures.

- [ ] **Step 4: Commit**

```bash
git add packages/brain-shell/src/ui/composer/PromptInput.tsx packages/brain-shell/src/ui/shell/AppShell.tsx
git commit -m "feat(shell-ui): route !-prefixed submits to the shell-execution path

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: PTY smoke — `!` end-to-end against a scripted daemon

**Files:**
- Create: `scripts/ptySmokeInc11.py`
- Create fixtures under: `packages/brain-shell/src/test/fixtures/pty/inc11/` (generated by the run)

**Interfaces:**
- Consumes: the committed harness discipline of `scripts/ptySmokeInc9.py` (stub UDS daemon, `TIOCSWINSZ` before exec, discrete keystrokes ≥0.3 s pumps, ANSI-strip matching, occurrence-count waits, strictly consecutive sequences, never close the shared connection early).
- Produces: 13-assertion proof incl. esc-cancel; fixtures `boot.txt`, `bang.txt`, `failed.txt`, `cancel.txt`, `picker.txt`, `resumed.txt`.

- [ ] **Step 1: Derive the script from `ptySmokeInc9.py`**

Copy `scripts/ptySmokeInc9.py` → `scripts/ptySmokeInc11.py`, then apply these exact changes:

1. Rename docstring to describe Inc 11; update constants:
   - `SOCK = "/tmp/brain-inc11-smoke.sock"`
   - `FIXTURE_DIR = ".../src/test/fixtures/pty/inc11"`
   - `CONFIG_FILE = "/tmp/brain-inc11-smoke-config.json"`
2. Replace `LOADED_MESSAGES` with the two-command stored history the resume flow will replay:

```python
LOADED_MESSAGES = [
    {"id": "m1", "role": "user", "content": "! echo bang-inc11"},
    {"id": "m2", "role": "tool", "content": env(
        call_id="shell-bang", name="bash", input={"command": "echo bang-inc11"},
        outcome="executed", is_error=False, exit_code=0,
        output="bang-inc11\n", duration_ms=88)},
    {"id": "m3", "role": "user", "content": "! false"},
    {"id": "m4", "role": "tool", "content": env(
        call_id="shell-false", name="bash", input={"command": "false"},
        outcome="executed", is_error=True, exit_code=1,
        output="", duration_ms=15)},
]
```

3. DELETE the whole `elif act == "v1/generation/stream":` arm and the `PERM_EVENTS`/`PERM_GRANTED` globals and the `v1/tool/resolve` arm (no permission dialog exists in this increment's flows). ADD:

```python
                    elif act == "v1/shell/exec":
                        payload = req.get("payload", {})
                        cmd = str(payload.get("command", ""))
                        # Slow command lets the esc-cancel flow race the reply.
                        if cmd == "sleep 5":
                            time.sleep(1.5)
                            reply({"version": "1.0", "type": "Response", "id": rid,
                                   "status": "success",
                                   "body": {"call_id": "shell-slow", "name": "bash",
                                            "input": {"command": cmd},
                                            "outcome": "executed", "output": "",
                                            "is_error": False, "exit_code": 0,
                                            "duration_ms": 1500}})
                        elif cmd == "false":
                            reply({"version": "1.0", "type": "Response", "id": rid,
                                   "status": "success",
                                   "body": {"call_id": "shell-false", "name": "bash",
                                            "input": {"command": cmd},
                                            "outcome": "executed", "output": "",
                                            "is_error": True, "exit_code": 1,
                                            "duration_ms": 15}})
                        else:
                            reply({"version": "1.0", "type": "Response", "id": rid,
                                   "status": "success",
                                   "body": {"call_id": "shell-bang", "name": "bash",
                                            "input": {"command": cmd},
                                            "outcome": "executed",
                                            "output": cmd.replace("echo ", "") + "\n",
                                            "is_error": False, "exit_code": 0,
                                            "duration_ms": 88}})
```

4. Replace Flow A2 and Flow B with (keep `snapshot`/`expect`/`expect_count` helpers as-is):

```python
ok = True

# ── Flow A: boot ───────────────────────────────────────────────────────────
ok &= expect("welcome-wordmark", "◆ BRAIN")
ok &= expect("launch-prompt", "❯")
snapshot("boot")

# ── Flow B: successful ! command renders user row + completed card ────────
os.write(fd, b"! echo bang-inc11")
pump(0.4)
os.write(fd, b"\r")
ok &= expect("bang-output", "bang-inc11")
ok &= expect("bang-duration-label", "Done in 0.1s")
pump(0.5)
snapshot("bang")

# ── Flow C: failing ! command renders Failed · exit 1 ─────────────────────
os.write(fd, b"! false")
pump(0.4)
os.write(fd, b"\r")
ok &= expect("failed-exit-code", "Failed · exit 1")
pump(0.5)
snapshot("failed")

# ── Flow D: esc during a slow command cancels the wait, composer freed ────
os.write(fd, b"! sleep 5")
pump(0.4)
os.write(fd, b"\r")
pump(0.4)
os.write(fd, b"\x1b")          # esc -> abort signal -> cancelled notice
ok &= expect("cancelled-notice", "Shell command cancelled.")
pump(0.5)
snapshot("cancel")

# ── Flow E: /resume replays both commands as frozen cards ─────────────────
os.write(fd, b"/resume")
pump(0.4)
os.write(fd, b"\r")
ok &= expect("picker-listing", "Persisted events demo")
pump(0.5)
snapshot("picker")
os.write(fd, b"\r")
ok &= expect("resume-notice", "Resumed")
ok &= expect("replay-bang-card", "bang-inc11")
ok &= expect_count("replay-failed-cards", "Failed · exit 1", 2)
ok &= expect("replay-user-line", "! echo bang-inc11")
snapshot("resumed")
```

Notes for the implementer:
- The picker title `"Persisted events demo"` comes from the existing `session/list` arm — leave that stub arm untouched except the `message_count` (now 4).
- `expect_count(..., 2)` for `Failed · exit 1`: once live (Flow C) and once replayed (Flow E) — cumulative-buffer counting, the Inc 6 pattern.
- Esc reaches the composer only when not paused; no dialogs exist in these flows, so `\x1b` maps to `onAbort` → `controller.abort()` → the Task 2/3 abort chain.

- [ ] **Step 2: Run the smoke**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain && python3 scripts/ptySmokeInc11.py`
Expected: 13 PASS lines, exit 0. On any FAIL: capture `clean(buf)` tail, fix, rerun; regenerate fixtures only when content genuinely changed (verify ⏺/⎵ card lines byte-stable across reruns; restore timing-noise-only churn via `git checkout -- <fixture>`).

- [ ] **Step 3: Commit**

```bash
git add scripts/ptySmokeInc11.py packages/brain-shell/src/test/fixtures/pty/inc11
git commit -m "test(smoke): drive ! execution, cancel, and replay through a PTY

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 7: Full regression gates

**Files:** none modified unless a gate forces a fixture restore.

- [ ] **Step 1: Cargo workspace**

Run:
```bash
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test --workspace'
```
Expected: green everywhere except the pre-existing security-audit test.

- [ ] **Step 2: Bun suite**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test`
Expected: all Task 2–4 additions pass; 5 documented environmental fails; nothing else red.

- [ ] **Step 3: Prior PTY smokes unchanged**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain && python3 scripts/ptySmokeInc6.py && python3 scripts/ptySmokeInc9.py`
Expected: 14/14 and 15/15. Restore noise-only fixture churn with explicit-path checkout.

- [ ] **Step 4: Vendor-concept scan**

Run:
```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git diff 05c5051b..HEAD -- crates daemon packages scripts | grep '^+' | grep -icE "anthropic|api\.anthropic|claude"
```
Expected: `0`.

- [ ] **Step 5: Report**

Summarize gate results honestly (including any baseline drift) and hand off to finishing-a-development-branch.
