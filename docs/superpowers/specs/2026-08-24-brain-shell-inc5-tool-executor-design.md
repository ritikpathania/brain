# Increment 5 — Daemon-Side Tool Executor (Design)

**Status:** Approved design, 2026-08-24
**Parent spec:** `docs/superpowers/specs/2026-08-23-brain-shell-contracts-first-design.md`
**Predecessor:** Increment 4 permission wire round-trip (`main @ da8ac697`)

## 1. Goal

Close the permission loop: an approved `bash` tool call actually executes
daemon-side, and its output rides back over UDS as a `tool_result` stream
frame that the shell renders on the existing tool card.

## 2. Non-goals (explicit)

- **No agentic feedback loop.** The provider never sees the tool output; the
  turn finishes as it does today after the result frame. Feeding results back
  for continued generation is a future increment.
- **No file tools.** bash only.
- **No session-storage persistence of outputs.** The result lives in the wire
  stream and the shell's frozen transcript only.
- **No shell-side execution.** All execution is daemon-side.
- `gen_request.tools` stays empty; tool discovery/announcement is out of scope.

## 3. Architecture

Reuse the dormant executor stack; write no new execution machinery:

- `brain_core::extensibility` (existing): `Tool` trait (`metadata()`,
  `execute(context, args) -> Result<ExecutionResult, BrainError>`),
  `ToolMetadata` (+ `ExecutionPolicy { timeout_ms }`), `Permission::Shell`,
  `ExecutionContext { session_id, working_dir, cancellation, deadline }`,
  `ExecutionResult` (wraps `serde_json::Value`).
- `crates/brain-tools` (existing): `ToolRegistryImpl`, `PermissionManager`,
  `BlockingToolRunner`, `ToolExecutor` (permission validation → cancellation
  check → `spawn_blocking` with timeout/cancel select).
- New daemon module `daemon/src/tools/`: one concrete tool + wiring.
- Modified: `daemon/src/transport/uds/handlers.rs` post-grant branch;
  shell client chunk parsing + adapter mapping.

## 4. Components

### 4.1 `daemon/src/tools/bash_tool.rs` — `BashTool`

Implements `Tool`:

- Metadata: `{ name: "bash", required_permissions: [Shell],
  execution_policy.timeout_ms: 30_000, causes_side_effects: true,
  supports_streaming: false, is_idempotent: false }` plus description/usage/
  version("0.1.0")/author fields per `ToolMetadata`.
- Input contract: `arguments["command"]` must be a non-empty string; anything
  else → `Err(BrainError::Internal)` (spawn-level failure semantics).
- Executes `/bin/bash -c <command>` with `Command::current_dir(context.working_dir)`
  and inherited environment; captures stdout and stderr **separately**
  (`Command::output()`), then the payload's `output` field is stdout followed
  by stderr (appended after a `\n` separator whenever stderr is non-empty),
  UTF-8-lossy, truncated at 32_768 bytes with a trailing `…[truncated]`.
- Output payload (`ExecutionResult::new(json!({...}))`):
  `{ "output": <as captured above>, "exit_code": <i32>,
  "is_error": <exit != 0> }`. A non-zero exit is a **result**, not an `Err`
  — only spawn/IO failure or invalid arguments return Err.
- No shell injection defenses beyond exact argv form (`["-c", command]`);
  consent happens through the permission round-trip.

### 4.2 `daemon/src/tools/mod.rs` — `ToolStack`

One lazily-initialized global (mirrors `PERMISSION_WAITERS`):

```rust
struct ToolStack {
    registry: ToolRegistryImpl,        // holds Arc<BashTool>
    permissions: PermissionManager,
    executor: ToolExecutor,            // over BlockingToolRunner
}
static TOOL_STACK: OnceLock<Arc<ToolStack>> = …;
fn tool_stack() -> &'static Arc<ToolStack>;
```

### 4.3 `handlers.rs` post-grant branch

Today `if !granted { …emit tool_denied… }` and the granted path does nothing.
After grant:

1. `tool_stack().permissions.grant(Permission::Shell)` — the wire verdict is
   the executor-side authority (single-consent unification).
2. Build `ExecutionContext { session_id: parsed_session_id,
   working_dir: daemon process cwd, cancellation: fresh token,
   deadline: None }`.
3. Registry lookup by `tool_name`:
   - unknown name → emit `tool_result` with `is_error:true`,
     `output:"Unknown tool '<name>'"`, `exit_code:-1`; skip execution.
   - known → `executor.execute(tool, &ctx, &permissions, &args).await`.
     - `Ok(result)` → frame from payload.
     - `Err(_)` → `tool_result` with `is_error:true`,
       `output:"<error summary>"`, `exit_code:-1`.
4. Frame shape (one frame, strictly consecutive sequence):

```json
{"type":"tool_result","generation_id":"…","session_id":"…","sequence":N,
 "call_id":"call_mock_1","tool_name":"bash",
 "output":"hello\n","is_error":false,"exit_code":0,"status":"in_progress"}
```

5. Stream then continues exactly as today (remaining provider chunks →
   `stream_end` → `finished`). Deny path unchanged.

### 4.4 Shell reception

- `UdsBrainBackendClient.ts`: new tolerant branch mapping `tool_result`
  frames → chunk `{ type:'tool_result', callId, toolName, output, isError,
  exitCode, sequence }` (camelCase, same pattern as
  `tool_permission_requested`). Sequence guard applies unchanged.
- `BrainBackendClient.ts`: extend `BrainStreamChunk` union with the
  `tool_result` variant.
- `chunkToTurnEvents.ts`: map to the EXISTING `tool_result` turn event
  (`{ type:'tool_result', callId, output }`) — card rendering already handles
  settled results, so no renderer changes.
- Controller: no changes; end-of-turn settlement keeps covering streams whose
  tools never produce results.

## 5. Error handling

| Condition | Outcome |
|---|---|
| Unknown tool name | `tool_result is_error:true`, turn completes |
| Invalid/missing `command` arg | `Err` → `tool_result is_error:true` |
| Timeout (30 s policy) | `BrainError::Timeout` → `tool_result is_error:true` |
| Cancellation | `BrainError::Cancelled` → `tool_result is_error:true` |
| Non-zero bash exit | normal `tool_result`, `exit_code:N`, `is_error:true` |

The turn always proceeds to completion after the frame.

## 6. Testing strategy

- **Rust** `daemon/tests/uds_tool_execution_tests.rs` (real daemon, harness
  copied from the Inc 4 permission suite): grant flow asserts the
  `tool_result` frame (`echo hello` → output contains `hello`, `is_error:false`,
  consecutive sequences); deny flow asserts NO `tool_result`;
  `[brain-tool:nosuchtool]` sentinel → `is_error:true`; `exit 3` →
  `exit_code:3` + `is_error:true`. Existing suites must stay green.
- **Shell** unit tests: frame→chunk parsing (snake→camel), event mapping, and
  a controller test proving a delivered `tool_result` renders before
  settlement (no duplicate settlement).
- **PTY smoke** `scripts/ptySmokeInc5.py`: allow flow shows the command's
  real output text in the transcript; deny flow unchanged from Inc 4.
- Full gates unchanged (bun suite vs baselines, cargo suites, BUILD_OK via
  canonical bundle command, vendor scans).

## 7. Global constraints (carried verbatim)

- Preserve Brain's architecture, domain model, IPC contracts, runtime,
  memory, retrieval, graph, provenance, agents, and adapter boundaries.
- No Claude/Anthropic models, APIs, auth, pricing, billing, or LLM-specific
  product concepts introduced.
- Claude Code tree stays implementation archaeology outside the repository.
- Stack: Bun + React 19 + Ink 7 + yoga-layout; no framework changes.
- Small increments, each independently verifiable; commits carry explicit
  paths only; trailer `Co-Authored-By: Claude <noreply@anthropic.com>`.
