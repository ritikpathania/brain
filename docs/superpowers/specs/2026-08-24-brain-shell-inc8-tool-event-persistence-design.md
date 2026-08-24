# Brain Shell Inc 8 — Tool-Event Persistence Design

**Date:** 2026-08-24 · **Status:** Approved design · **Base:** `main @ f9ae5c35` (post-Inc 7)
**Increment goal:** Persist every agentic-loop tool outcome (executed and denied) into the session transcript as first-class `MessageRole::Tool` records, without changing any wire behavior, execution semantics, or permission flow.

## 1. Decisions

| Question | Decision | Rationale |
|---|---|---|
| Primary consumer of persisted tool events | **Session transcript** | Reuses `SessionRepository.save_session` end-to-end; replay reads come free via `load_session`; smallest Brain-owned surface |
| Coverage | **Executed + denied** | Transcript stays honest about what the model attempted; refusals visible like Claude Code's |
| Output payload bound | **4 KB on `output` only; inputs verbatim** | Sessions stay small/loadable; the wire transcript already carried full text for the live turn |
| Persistence-failure semantics | **Best-effort, never block** | A disk hiccup must not kill a good generation turn; matches today's `let _ = save_session(...)` idiom |
| Storage vehicle (**Approach A**) | **New `MessageRole::Tool` variant riding the existing message flow** | Zero new storage/repo surface; rides existing aggregate persistence; envelope is JSON in content because Brain owns both writer and future readers |

Rejected shapes: typed `ToolEvent` vec on the aggregate (larger domain/snapshot surface, no projection benefit this increment); assistant-role sentinel lines (stringly-typed, collides with mock provider's `[brain-tool:]` grammar).

## 2. Architecture

One-line shape: tool outcomes become `Message`s written through the session aggregate's existing `add_message` + `save_session` flow at the moment each outcome is known.

```
provider pass ──▶ tool_use chunk
                     │
              permission round-trip (unchanged, sole gate)
                     │
        ┌────────────┴────────────┐
     granted                  denied
        │                        │
  executor.execute         (no execution)
        │ + Instant timing       │
        ▼                        ▼
  wire: tool_result frame   wire: tool_denied frame      ← unchanged bytes/seq
        │                        │
        └──────────┬─────────────┘
                   ▼
   Message::new(MessageId::new(), MessageRole::Tool, envelope_json)
   session_aggregate.add_message(msg)          ← existing API
   storage.save_session(...)                   ← existing repo; best-effort
```

### Layer impact

| Layer | Change |
|---|---|
| `crates/brain-domain/src/entities.rs` | `MessageRole::Tool` variant + `Display` ("tool") + `FromStr` arm; serde emits `"tool"` automatically (`rename_all = "lowercase"`) |
| `daemon/src/transport/uds/handlers.rs` | Two insertion points (granted ~2179, denied ~2204): build envelope, cap output at 4 KB, persist best-effort; plus one pure truncation helper |
| brain-storage / brain-services / brain-shell / packages | **untouched** |

### Structural safety fact

`model_messages` builds exclusively from the *wire request* (`handlers.rs:1808`, `stream_req.messages`) — never from loaded sessions. The resume paths (`:847`, `:971`) are persistence-RPC handlers only. Persisted Tool envelopes therefore structurally cannot leak into provider requests.

## 3. Components & Data Flow

### 3.1 Envelope schema (v1)

Self-identifying JSON inside the message content field:

```json
{"type":"tool_event","v":1,"call_id":"call_mock_1","name":"bash",
 "input":{"command":"ls -la"},"outcome":"executed",
 "is_error":false,"exit_code":0,"output":"src\nCargo.toml","duration_ms":12}
```

```json
{"type":"tool_event","v":1,"call_id":"call_mock_2","name":"bash",
 "input":{"command":"rm -rf /"},"outcome":"denied"}
```

Rules:
- `executed` carries exactly `is_error` (bool), `exit_code` (i64), `output` (string), `duration_ms` (u64).
- `denied` carries none of those fields (nothing ran).
- `input` is stored verbatim from the provider packet (`packet["toolUse"]["input"]`), never truncated.
- `call_id` and `name` mirror the corresponding wire frame.
- `type`/`v` make records forward-discriminable inside plain-text content.

### 3.2 Truncation helper

Pure function in `handlers.rs`:

```rust
fn truncate_tool_output(output: &str) -> String
```

- `<= 4096` bytes → unchanged.
- `> 4096` bytes → walk back to the last `char_boundary` at or under 4096, cut there, append `"\n…[truncated]"`.
- Mirrors `BashTool::execute`'s existing marker idiom; applies to the persisted copy only — wire frames keep full text.

### 3.3 Timing capture

Granted path only: `Instant::now()` immediately before `executor.execute(...)` resolves; `elapsed().as_millis() as u64` after. No timing exists on the denied path (no field).

### 3.4 Write mechanics (both sites, identical shape)

```rust
let envelope = serde_json::json!({ /* outcome-shaped fields per §3.1 */ });
session_aggregate.add_message(brain_domain::Message::new(
    brain_domain::MessageId::new(),
    brain_domain::MessageRole::Tool,
    envelope.to_string(),
));
if let Err(e) = storage.save_session(&parsed_session_id, &session_aggregate) {
    tracing::warn!(error = %e, "tool event persistence failed; continuing");
}
```

- Written **immediately per outcome**: a cancel or crash mid-turn keeps tool events that already happened.
- Session ordering is natural: user msg → tool events → final assistant msg (Invariant 4 unchanged).
- Each save serializes the whole aggregate through the existing SQLite session repo; `session_aggregate` is already in scope across the rounds loop (Invariant 4 uses it post-loop).

### 3.5 Documented non-change

`brain-services/conversation.rs`'s System/non-System context-window filter buckets Tool envelopes as ordinary messages. It is off the daemon's live stream path; refining that is future work if it ever matters.

## 4. Error Handling

| Case | Behavior |
|---|---|
| `save_session` fails | `tracing::warn!`; generation loop continues untouched — never a wire error |
| Output > 4096 bytes | char-boundary cut + `\n…[truncated]` marker (persisted copy only) |
| Multibyte output at cut edge | helper walks back to `char_boundary` — no panics, no mojibake |
| Cancel/fail mid-turn | already-persisted tool events stay persisted; nothing unwinds them |
| Envelope serialization | `serde_json::json!` over owned values is infallible in practice; `.to_string()` cannot fail |

Advertisement/authorization invariant untouched: the permission round-trip remains the sole execution gate; persistence observes outcomes, it never grants anything.

## 5. Testing Strategy

1. **brain-domain unit** (`entities.rs` tests): `MessageRole::Tool` — `Display` == `"tool"`, serde serializes `"tool"`, `FromStr("tool")` == `Tool`.
2. **Truncation helper unit** (daemon): ASCII over-cap → cut + marker; multibyte string cuts at boundary without panic; exactly-at-cap passes through unmarked.
3. **Executed-path integration** (extend `uds_feedback_loop_tests` harness): scripted sentinel tool call drives the stream → `load_session` → assert ≥1 `Tool`-role message whose parsed envelope matches the wire (`name == "bash"`, `input.command`, `outcome == "executed"`, `is_error`/`exit_code` present).
4. **Denied-path integration** (permission-roundtrip style): deny via wire → assert `Tool` message with `outcome == "denied"` and no execution fields.
5. **Regression**: all suites at baseline counts; packet shapes and sequence rules byte-identical.
6. **PTY smoke**: pure rerun (nothing user-visible changed) — 14/14 PASS, fixtures restored if touched.

## 6. Non-Goals

- No shell changes or history-rendering UI (reading these records is future work).
- No `DomainEvent` variants; no `event_envelopes` SQLite usage; no reflection-store usage.
- No memory-engine/retrieval ingestion of tool results.
- No artifact spill-over storage.
- No new wire packets, no sequence-rule changes, no shell protocol edits.
- No changes to `conversation.rs` budgeting behavior (documented in §3.5).

## 7. Constraints

- Branch: `feature/brain-shell-inc8-tool-event-persistence` from `main @ f9ae5c35`.
- Every cargo invocation needs the macOS rpath wrapper:
  `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test ...'`
- Daemon package name is **`brain-daemon`**, not `daemon`.
- Working tree carries ~1k files of pre-existing user WIP: stage **explicitly named paths only**; never stash, never wholesale-checkout, never discard Cargo.lock.
- Commits: explicit-path `git add <paths>`; trailer `Co-Authored-By: Claude <noreply@anthropic.com>`; known-harmless noise: `error: daemon terminated` around git ops, CRLF fixture warnings.
- Baselines that must hold: daemon lib 39/0; UDS feedback 4, generation 3, adversarial 6, tool-execution 4, permission 3, memory 5, load 4, product 6, lifecycle 9, soak 3; brain-tools integration 6; brain-services lib 44/0; shell suite 231 pass / 5 documented fails; PTY smoke 14/14. Sole permitted failure remains the pre-existing untracked `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers`.
- Vendor-concept scan greps only added lines since this spec's commit:
  `git diff <spec-commit>..HEAD -- crates daemon packages scripts | grep '^+' | grep -icE "anthropic|api\.anthropic|claude"` → expect `0`.
