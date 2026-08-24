# Increment 6 — Agentic Feedback Loop Design

**Status:** Approved design, awaiting implementation plan.
**Extends:** `docs/superpowers/specs/2026-08-24-brain-shell-inc5-tool-executor-design.md` (whose §2 non-goal "single-pass only" this increment opens).
**Date:** 2026-08-24

## 1. Context and Goal

Increment 5 gave Brain's Rust daemon a real tool executor: when the model emits a
tool call, the daemon gates it behind a permission round trip, executes approved
calls through `brain-tools`' `ToolExecutor` (`BashTool` today), and reports one
`tool_result` frame back over the UDS wire. The turn still ends after that single
provider pass — the result is shown to the user but never seen by the model.

Increment 6 closes that loop **within one turn**: resolved tool calls are appended
to the conversation as provider-visible messages and `stream_generation` is
re-invoked on the same wire stream, so the model can react to output, issue
follow-up calls, and finish coherently. This is the daemon-side agentic loop;
nothing about it introduces any provider-specific or LLM-vendor concept.

**Goal:** one user prompt can drive N sequential tool rounds —
`tokens → tool_use → permission → tool_result → tokens → … → stream_end` —
with the model observing every result before deciding what comes next.

## 2. Decisions (settled during brainstorming)

| Question | Decision |
|---|---|
| How many rounds per turn? | Cap of **8**, env-overridable via `BRAIN_TOOL_MAX_ROUNDS`; parser floors at 1; garbage/unset ⇒ default. Exhaustion ends the turn gracefully. |
| What happens on denial mid-loop? | The denial is fed back to the provider as a failed tool result and the loop continues — the model sees it and may apologize, adapt, or finish. Deny never aborts the turn. |
| Where does the loop live? | Approach A: outer rounds loop inside the existing generation arm of `daemon/src/transport/uds/handlers.rs`. No new module; pure helpers extracted only where unit-testing demands. |

## 3. Architecture

Wrap the single-pass drain block in `'rounds: for _ in 0..max_rounds`. The cap
is the **maximum number of provider passes per turn**: `BRAIN_TOOL_MAX_ROUNDS=8`
allows at most eight `stream_generation` invocations for one prompt.
Request construction and the `stream_generation` call move **inside** the loop;
memory-context assembly, system-prompt combination, and model resolution stay
**outside** (assembled once per turn, reused verbatim across passes).

Continuation rule after a pass ends with `Completed`:

> continue iff **that pass resolved ≥ 1 tool call** (executed *or* denied — both
> produce feedback) **and** rounds used < cap.

A pass that resolves zero tool calls always terminates the turn. Streams that end
via `None` (no `Completed`) or via provider `Err` never continue.

### Layer impact

| Layer | Change |
|---|---|
| `daemon/src/transport/uds/handlers.rs` | **Only production file with behavioral change.** Rounds loop; private collector structs; two pure helpers. |
| `crates/brain-services/src/model/mock.rs` | Test-infrastructure addition only (§4.3); inert without its env var. |
| brain-core | None — `MessageContentBlock::{ToolUse, ToolResult}` already exist (`crates/brain-core/src/model.rs:71-86`). |
| brain-tools / executor / permission manager | None — invoked exactly as Inc 5 wired them. |
| brain-shell | None — frame shapes, ordering rules, and the terminator are unchanged; `stream_end` simply arrives later. |

## 4. Components

### 4.1 Collectors (private to handlers.rs)

```rust
struct PassToolUse { call_id: String, name: String, input: serde_json::Value }
struct ToolFeedback { call_id: String, name: String, input: serde_json::Value,
                      output: String, is_error: bool }
```

`PassToolUse` is recorded when a `GenerationChunk::ToolUse` chunk arrives.
`ToolFeedback` is recorded at resolution time — the grant/execute path *and* the
deny path both push one entry. Both vectors clear at pass start.

### 4.2 Pure helpers

```rust
fn feedback_messages(
    pass_text: &str,
    calls: &[PassToolUse],
    results: &[ToolFeedback],
) -> Vec<brain_core::model::ModelChatMessage>
```

Returns exactly two messages:

1. **Assistant** — content blocks: a `Text { text: pass_text }` block first when
   the pass emitted non-empty text, then one `ToolUse` block per call in arrival
   order.
2. **User** — one `ToolResult` block per feedback entry, ordered to match the
   calls. Denial entries carry the fixed content
   `"User denied permission for this tool call."` with `is_error: true`;
   executed entries carry the executor's output text and its `is_error`.

```rust
fn parse_max_rounds(raw: Option<&str>) -> u32
```

Default 8; floors at 1; unparseable input ⇒ default. The env read stays in a
thin caller so tests never mutate process environment.

### 4.3 Mock multi-response seeding (`crates/brain-services/src/model/mock.rs`)

If `BRAIN_MOCK_SCRIPTED_RESPONSES` is set at provider construction, parse it as a
JSON array of `ScriptedResponse` objects and seed `scripted_queue`. The queue
already pops one response per `stream_generation` call, so a K-element array
scripts a K-pass turn deterministically. Malformed JSON ⇒ warn once, seed nothing,
behave exactly as today. Without the variable the constructor is byte-identical
to the current implementation (same pattern as the existing
`BRAIN_MOCK_CHUNK_DELAY_MS`).

### 4.4 Wire contract

Zero new frame types, zero field changes. Observable shape of a two-round turn:

```
stream_start(0)
token(1…)                      ← round-1 text
tool_use(n)                    ← round-1 call
tool_permission_requested(n+1)
tool_result(n+2)               ← executed (or tool_denied on refusal)
token(…)                       ← round-2 text
…
stream_end(final)              ← sole terminator; sequence monotonic throughout
```

Rules the loop must preserve:

- Intermediate `Completed` chunks emit **no frame** — they are absorbed by the
  loop decision. Only the final pass's `Completed` becomes `stream_end`.
- `seq` is one turn-scoped counter across all passes; numbering stays strictly
  consecutive (the shell aborts streams on gaps).
- `usage` in `stream_end.metadata` **sums across passes** (each pass reports its
  own input/output tokens).
- Final `finish_reason` = last pass's finish reason, except cap exhaustion with
  pending feedback ⇒ `"max_tool_rounds"`.
- `accumulated_response` spans all passes and is persisted as the single
  assistant message on graceful completion, exactly as Inc 5 shipped.

## 5. Error Handling

| Event | Behavior |
|---|---|
| Executor returns `Err` / unknown tool name | Already shaped as failed output by Inc 5 (`format!("{e}")`, `is_error: true`); collected as ordinary feedback; loop continues. |
| Denial (explicit refusal or permission timeout) | Fixed denial feedback message; loop continues. Behavior note: a timed-out permission previously stranded the turn silently; the model now observes the denial and may finish gracefully. |
| Provider yields `Err` in any pass | Existing error-frame path; loop breaks; assistant persist skipped (unchanged invariant). |
| Stream ends without `Completed` (`None`) | Terminal; counted successful as today; no continuation. |
| Cancellation (Esc / disconnect) | Per-pass `select!` unchanged; token re-checked before each re-invoke; cancelled turns persist nothing. |
| Cap exhausted with pending feedback | Graceful `stream_end` with `finish_reason: "max_tool_rounds"`; accumulated text persisted if non-empty. |
| Malformed `BRAIN_MOCK_SCRIPTED_RESPONSES` | Warn-log once, seed nothing, default behavior. |

## 6. Non-Goals

- Advertising `ToolDefinition`s to providers — `gen_request.tools` remains empty;
  `ToolMetadata` has no parameter-schema field yet, so honest advertisement needs
  its own increment.
- Parallel tool execution within a pass (sequential, matching current code).
- Streaming/partial tool output.
- Persisting tool events into session history (session messages remain
  user-prompt + final assistant text).
- Any brain-shell production change.

## 7. Testing Strategy

TDD throughout; red-green-commit per task.

1. **Pure-helper units** (in-module `#[cfg(test)]` in handlers.rs):
   `feedback_messages` — text-plus-tools pass yields Text block then ToolUse
   blocks; textless pass omits the Text block; denial entries carry the fixed
   string with `is_error: true`; arrival order preserved end-to-end.
   `parse_max_rounds` — default, flooring, garbage-input cases as a pure fn.
2. **UDS integration** (`daemon/tests/uds_feedback_loop_tests.rs`; harness copied
   verbatim from `uds_tool_execution_tests.rs`, daemon spawned with
   `BRAIN_MOCK_SCRIPTED_RESPONSES`):
   - *two-round happy path* — strictly consecutive sequences spanning both
     passes; round-2 tokens arrive after the `tool_result` sequence; final
     `response` contains both passes' text; summed usage (mock hardcodes
     `input_tokens: 15` per pass ⇒ 30 for two passes); `finish_reason: "end_turn"`.
   - *deny continues the loop* — refuse the permission; `tool_denied` observed,
     zero `tool_result` frames, round-2 text still streams into `response`.
   - *cap enforcement* — daemon spawned with `BRAIN_TOOL_MAX_ROUNDS=1`, three
     scripted tool rounds; `stream_end` follows round 1's result with
     `finish_reason: "max_tool_rounds"` and no round-2 tokens.
   - *single-pass regression* — plain scripted response produces today's exact
     wire shape.
3. **Shell** — no new suites (no production changes); full existing suite green.
4. **PTY smoke** (`scripts/ptySmokeInc6.py`) against the stub daemon:
   - allow flow: card 1 preview renders, second permission dialog mounts
     (occurrence-count wait), card 2 preview renders, post-loop final text
     renders, done state reached.
   - deny-second-call flow: `Denied bash` notice renders **and** the model's
     post-denial text appears in the same turn.
5. **Gates** — full bun test suite, daemon `cargo test`, canonical build gate,
   vendor scans 0/0, smoke exit 0.

## 8. Project Constraints Carried Forward

- Preserve Brain's architecture, domain model, IPC contracts, runtime, memory,
  retrieval, graph, provenance, agents, adapter boundaries.
- No Claude/Anthropic models, APIs, authentication, pricing, billing, or LLM-
  vendor product concepts.
- Stack unchanged: Bun + React 19 + Ink 7 + yoga-layout shell; Rust daemon.
- Every commit contains only explicitly-added paths (`git add <paths>`); commit
  trailer `Co-Authored-By: Claude <noreply@anthropic.com>`.
- macOS builds need
  `RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks"`.
