# Brain Shell Inc 11 — `!` Bash Mode Execution Design

**Date:** 2026-08-25 · **Status:** Approved design · **Base:** `main @ a8c29608` (post-Inc 10)
**Increment goal:** Make the advertised but non-functional `!` bash mode actually execute commands through the daemon's existing tool stack as standalone persisted turns — no second shell-execution path, no provider involvement, no changes to the agentic loop.

## 0. Problem

The composer detects `!`-prefixed input and records it in history as bash-mode (`PromptInput.tsx:139–142`), then **strips the `!` and submits the bare command as an ordinary prompt** — the mode is lost at `props.onSubmit(bare)`, and the text is mailed to the model. The charter's §3 capability map promises `!` passthrough. Nothing executes.

## 1. Decisions

| Question | Decision | Rationale |
|---|---|---|
| Where does execution live? (**daemon action**, resolved as preferred) | New UDS action `v1/shell/exec`, routed through the existing ToolStack | BashTool, ToolExecutor, authorization, timing, and persistence all live daemon-side; a local-spawn path would be a second execution path by definition |
| Which socket carries it? | Short-lived request/response connection (the `v1/tool/resolve` RPC template, `UdsBrainBackendClient.ts:831`) | Preserves the streaming socket's zero-reconnect/deterministic-sever invariant; one line in, one line out; disconnect-before-response is deterministic |
| Permission semantics | **Keystroke-as-grant**: handler calls `permissions.grant(Permission::Shell)` — the same call at the same structural position as the agentic granted branch (`handlers.rs:2125`) — then executes through `ToolExecutor.execute`'s normal `validate_tool_permissions` gate | The user typing `! cmd ⏎` *is* the consenting authority; the grant replaces the dialog, not the gate. Verified inert for the agentic flow: no `is_granted` precheck exists anywhere in `handlers.rs`, so the agentic round-trip always prompts regardless of grant-set state |
| Standalone turn or agentic participation? | **Standalone turn**, fully persisted to history | The stream socket enforces strictly-consecutive sequences — injecting results into an open generation would trip the shell's gap-abort guard. Nothing is fed to the provider: no GenerationRequest, no feedback loop, no model tokens |
| Response shape | Body = the Inc 8 envelope vocabulary minus the `type`/`v` wrapper (`call_id`, `name`, `input`, `outcome`, `is_error`, `exit_code`, `duration_ms`) | The persisted envelope is built from these exact fields, so response and transcript cannot drift; TS mapping mirrors the existing `tool_result` parser discipline |
| Esc behavior | Cancels the **client's wait** (closes the short-lived socket, returns composer to idle); cannot kill the spawned child | Honest Inc 5 limitation, unchanged: BashTool uses blocking `Command::output()` and never reads the cancellation token; the executor's 30 s policy timeout remains the hard bound |

Rejected shapes: client-side spawn (second execution path; no access to aggregate persistence); interactive dialog for a command the user just typed (absurd UX, and CC doesn't prompt either); queueing behind a live turn (charter §7 queueing is unimplemented — gap #8 stays out of scope).

## 2. Architecture

One-line shape: `v1/shell/exec` validates, grants, persists the user line, executes via the shared stack, responds once, persists the envelope — all on a short-lived socket.

```
PromptInput (mode='bash') ─▶ AppShell.handleSubmit ─▶ controller.runShellCommand(cmd)
                                                          │ busy-guarded
                              UdsBrainBackendClient.execShell(sid, cmd)
                                                          │ new short-lived UDS conn
                              ──▶ daemon: v1/shell/exec arm
                                    1. parse payload; load session (load_session, :1711 seam)
                                    2. validate: known session, non-empty command
                                    3. reject if generation active on session (:1745 backstop)
                                    4. permissions.grant(Permission::Shell)      ← :2125 twin
                                    5. persist User msg "! <cmd>"                ← :1788 twin
                                    6. get_tool("bash") → executor.execute       ← :2148 twin
                                    7. respond body (envelope fields)
                                    8. persist Tool envelope + save_session      ← :2190 twin
                                                          │
                              ◀── response body
                              controller synthesizes [tool_call_requested → tool_result]
                              → BrainTurnTransformer.transform → turnToRows   ← parity machinery
```

### Layer impact

| Layer | Change |
|---|---|
| `daemon/src/transport/uds/handlers.rs` | One new dispatch arm modeled on `v1/model/list` (`:679`): steps 1–8 above |
| `packages/brain-shell/src/ui/composer/PromptInput.tsx` | Widen prop `onSubmit: (value: string) => void` → `(value: string, mode: 'prompt' \| 'bash') => void`; pass `wasBash` through (`:71`, `:144`) |
| `packages/brain-shell/src/ui/shell/AppShell.tsx` | `handleSubmit(text, mode)` routes `bash` → `controller.runShellCommand(text)` (`:171–174`, sole consumer `:228`) |
| `packages/brain-shell/src/state/sessionController.ts` | New `runShellCommand`: busy-guarded notice, user row, RPC, synthesized reducer events, projection, idle restore |
| `packages/brain-shell/src/client/UdsBrainBackendClient.ts` + `BrainBackendClient.ts` | New `execShell(sessionId, command): Promise<ShellExecResult>` on a short-lived socket; interface addition |
| brain-tools crate, BashTool, ToolStack wiring (`tools/mod.rs`), PermissionManager, envelope schema, storage layer, replay code | **untouched** |

### Structural safety facts

- **No `is_granted` precheck exists in `handlers.rs`** (verified by grep): a warmed grant set cannot silently approve future agentic calls — those always emit `tool_permission_requested` and await the wire verdict. Grants are already process-lifetime state today (`:2125` never revokes); this adds no new permanence.
- **Persistence mutual exclusion**: the active-generation registry check (step 3, mirroring `:1745–1761`'s `session_busy` rejection) makes it structurally impossible for the exec arm and the stream arm to interleave `save_session` writes to one aggregate.
- **Replay needs zero new code**: `sessionToRows` already renders User rows and frozen Inc 8 envelopes as cards (Inc 9 fixture proves it). A `!` turn replays as user row + tool card — byte-compatible with what live rendering produces.
- **Live rendering reuses the parity path by construction**: controller projects through the same `BrainTurnTransformer.transform` + `turnToRows` functions the agentic turn uses (the transformer's push branch already handles a bare `tool_result` without a preceding `tool_use`).

## 3. Components & Data Flow

### 3.1 Wire contract

Request (newline-delimited JSON, standard versioned shape):

```json
{"id":"<req-id>","action":"v1/shell/exec",
 "payload":{"session_id":"…","command":"echo hi"}}
```

Success response:

```json
{"version":"1.0","type":"Response","id":"…","status":"success","body":{
  "call_id":"shell-<uuid>","name":"bash","input":{"command":"echo hi"},
  "outcome":"executed","output":"hi\n","is_error":false,
  "exit_code":0,"duration_ms":12}}
```

Error response (validation failures, unknown session, active generation, spawn failure):

```json
{"version":"1.0","type":"Response","id":"…","status":"error",
 "body":{"message":"…"}}
```

Rules:
- `outcome` is always `"executed"` on this path — denial is structurally impossible (keystroke-as-grant).
- `output`/`is_error`/`exit_code` are whatever BashTool produced (merged lossy stdout+stderr, 32 KB truncation marker, `exit_code −1` on signal). No new formatting anywhere.
- `duration_ms` measured daemon-side around `executor.execute` (`Instant::now()` immediately before, `.elapsed().as_millis() as u64` after) — the Inc 10 clock discipline.
- Transport-level errors produce **no envelope and no persisted record** (§4).

### 3.2 Handler order (all inside one dispatch arm)

1. Parse payload; resolve session id.
2. `storage.load_session(&parsed_session_id)` (`:1711` seam) — unknown session → error response.
3. Reject empty/whitespace command → error response.
4. Active-generation check against the registry (mirror `:1745–1761`) → error response.
5. `crate::tools::tool_stack().permissions.grant(Permission::Shell)` — comment must note why this is safe (no agentic precheck exists).
6. Persist `Message::new(…, MessageRole::User, "! <command>")` verbatim-with-bang + best-effort `save_session` (`:1788` twin). Crash mid-exec leaves the attempted command on the transcript, matching agentic Invariant 4 semantics.
7. Build `args_map {"command": …}`, `ToolExecutionContext {session_id, working_dir: std::env::current_dir(), cancellation: fresh CancellationTokenImpl, deadline: None}` (`:2140–2145` twin), `let started = Instant::now()`.
8. `stack.registry.get_tool("bash")` → `stack.executor.execute(...)` (`:2148` twin); extract `(out_text, is_err, exit_code)` from `result.value()` exactly as the stream arm does.
9. Respond with the success body (§3.1).
10. Build the executed-case envelope (`{"type":"tool_event","v":1,…,"outcome":"executed",…,"output": truncate_tool_output(&out_text),"duration_ms": …}`) → `add_message(MessageRole::Tool, …)` → best-effort `save_session` (`:2190` twin, byte-identical schema).

### 3.3 Client: `execShell`

Short-lived socket per call (template: `resolveToolPermission`, `UdsBrainBackendClient.ts:831`): connect → write request line → read one response line → close. Maps `body` to camelCase `ShellExecResult {callId, name, input, outcome, output, isError, exitCode, durationMs}` — same typeof-guarded mapping style as the `tool_result` parser. Connect refusal, non-success status, malformed JSON, or EOF-before-response each reject deterministically with a typed error; **no retry, no reconnect** (invariant preserved). The main stream connection is never touched.

### 3.4 Controller: `runShellCommand(command)`

1. If `this.busy` → notice ("Busy — wait for the current turn"), return (mirrors `:134`'s drop, but visible).
2. Set `busy`; push `{kind:'user', text:"! "+command}` row; emit.
3. Fire `execShell`. On success: synthesize events `[tool_call_requested(callId,'bash',input), tool_result(callId,output,isError,exitCode,durationMs)]`, run `BrainTurnTransformer.transform` + `turnToRows`, append projected card rows. On typed error: system/error notice row. On esc: cancelled notice row.
4. Restore idle (composer usable), emit.

Esc during exec aborts the RPC promise (socket teardown) — step 3's cancelled branch. The child process keeps running server-side until completion or the 30 s policy timeout (documented limitation, unchanged from agentic behavior).

### 3.5 What renders

Cards use existing renderers: completed → output + `Done in Xs`; failed → output + `Failed · exit N`. Replay shows identical frozen cards from the persisted envelope. Working directory remains the **daemon's cwd** (same as agentic bash today) — documented, not changed.

## 4. Error Handling

| Case | Behavior |
|---|---|
| Empty/whitespace command | Client never sends (submit guard); server also rejects → `status:"error"`, nothing granted, nothing persisted |
| Unknown session id | Error response; nothing granted, nothing persisted |
| Session has active generation | Error response (`session_busy` semantics); nothing granted, nothing persisted |
| Spawn failure (`BrainError::Internal` from execute) | Error response; client shows notice row; **no envelope persisted** (transport-error asymmetry — accepted tradeoff) |
| Command completes non-zero | Not an error: success body with `is_error:true` + real exit code; envelope persisted; card renders `Failed · exit N` |
| Socket connect fails / EOF / malformed response | Typed rejection → notice row; main stream untouched; no retry |
| Esc during exec | Wait cancelled, composer freed; child keeps running to its 30 s bound |
| `save_session` fails (either site) | `tracing::warn!`, continue — response still delivered (best-effort idiom, `:2207` twin) |

## 5. Testing Strategy

1. **Rust integration** (extend `uds_feedback_loop_tests` harness):
   - Happy path: exec `echo …` → success body asserts `exit_code == 0`, `is_error == false`, `output` contains marker, `duration_ms` present and u64.
   - Failing command: exec `"command": "false"` (real BashTool → `/bin/bash -c false`) → `exit_code == 1`, `is_error == true`, still `status:"success"`, envelope persisted.
   - Validation: empty command → `status:"error"` and post-load session shows **zero** new messages; unknown session → error.
   - Persistence: after a successful exec, `load_session` yields the `User "! …"` message followed by a `Tool` message whose parsed envelope matches the response fields (`name=="bash"`, `outcome=="executed"`, all executed-case keys incl. `duration_ms`).
   - Busy backstop: register an active generation for the session → exec rejected.
2. **Bun unit**:
   - `execShell` scripted-socket test (pattern: `toolResultWire.test.ts`): snake_case body → camelCase result; error status → typed rejection; EOF-before-response → rejection.
   - Controller: `runShellCommand` pushes user row, projects card rows carrying `Failed · exit 2` label data; busy-time call → notice, zero rows; success restores idle.
   - Parity unit (extend `liveCardParity.test.ts` pattern): locally projected `!` card rows deep-equal `sessionToRows` replay of the same envelope — both success and failed.
   - Composer/AppShell routing: bash-mode submit reaches `runShellCommand`, not `controller.submit`.
3. **PTY smoke** (new inc11 script, stub-daemon discipline carried from inc9): type `! echo bang` → user row + output card + duration label; `! false` → `Failed · exit 1`; slow exec + esc → cancelled notice and usable composer; `/resume` → both commands replay as frozen cards (end-to-end standalone-turn proof). Fixtures regenerated under `src/test/fixtures/pty/inc11`.
4. **Regression gates**: cargo workspace green via rpath wrapper; bun suite green; vendor scan 0; PTY fixtures restored where content is timing-noise-only.

## 6. Non-Goals

- No real process-kill on esc (requires BashTool changes — shared path; candidate follow-up).
- No queued-input preservation while busy (charter §7 gap #8 stays open).
- No always-allow/memory-status/highlighting/reconnect work (audit gaps #2, #7, #11 stay open).
- No changes to the agentic permission round-trip, sequence rules, envelope schema, or replay code.
- No provider involvement of any kind: `!` turns never reach a model.
- No working-directory change (daemon cwd stands).

## 7. Constraints

- Branch: `feature/brain-shell-inc11-bang-mode-execution` from `main @ a8c29608`.
- Every cargo invocation needs the macOS rpath wrapper:
  `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo ...'`
- Daemon package name is **`brain-daemon`**, not `daemon`.
- Working tree carries ~1k files of pre-existing user WIP (including deletions/untracked files under `docs/superpowers/specs/`): stage **explicitly named paths only**; never stash; never wholesale-checkout; never discard Cargo.lock.
- Commits: explicit-path `git add <paths>`; trailer `Co-Authored-By: Claude <noreply@anthropic.com>`; known-harmless noise: `error: daemon terminated` around git ops, CRLF fixture warnings.
- Baselines that must hold: bun shell suite 242 pass / 5 documented environmental fails; `uds_feedback_loop_tests` 6/6; brain-tools integration 6; PTY smoke inc9 15/15 (rerun unchanged paths). Sole permitted failure remains the pre-existing untracked `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers`.
- Vendor-concept scan greps only added lines since this spec's commit:
  `git diff <spec-commit>..HEAD -- crates daemon packages scripts | grep '^+' | grep -icE "anthropic|api\.anthropic|claude"` → expect `0`.
