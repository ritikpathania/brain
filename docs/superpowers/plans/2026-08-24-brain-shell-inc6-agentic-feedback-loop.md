# Increment 6 — Agentic Feedback Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Feed resolved tool results back to the provider within one turn — `tokens → tool_use → permission → tool_result → tokens → … → stream_end` — capped at N provider passes per turn.

**Architecture:** An outer `'rounds` loop inside the daemon's existing generation arm (`daemon/src/transport/uds/handlers.rs`). Each pass drains one `stream_generation` stream exactly as Inc 5 shipped; resolved tool calls (executed *or* denied) become `ModelChatMessage`s (assistant ToolUse blocks + user ToolResult blocks) appended to the conversation before the next pass. `stream_end` fires only on the final pass; the terminal `finished` frame and persistence stay outside the loop. Mock multi-round scripting rides a new `BRAIN_MOCK_SCRIPTED_RESPONSES` env var in `DeterministicMockProvider`. Zero brain-core, brain-tools, and brain-shell changes.

**Tech Stack:** Rust daemon (tokio, serde_json), brain-services mock provider, Bun test suite (unchanged), Python PTY smoke harness.

**Spec:** `docs/superpowers/specs/2026-08-24-brain-shell-inc6-agentic-feedback-loop-design.md`

## Global Constraints

- Every `cargo` invocation on this macOS machine MUST be wrapped as
  `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo …'`.
- All git commands: `git -C /Users/ritikpathania/Developer/PyCharm/brain …`. Stage ONLY explicitly-named paths, never `git add .` / `-A`. Commit trailer on every commit: `Co-Authored-By: Claude <noreply@anthropic.com>`.
- Canonical shell build gate: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun build src/main.tsx --outdir dist --target bun'` → expect `BUILD_OK`-style success.
- No Claude/Anthropic models, APIs, auth, pricing, billing, or LLM-vendor product concepts anywhere in committed source (`crates/`, `daemon/`, `packages/`, `scripts/`). Docs may reference the claude-code tree only as UX archaeology.
- Wire invariant: `sequence` numbers stay strictly consecutive across the WHOLE turn (gaps abort the shell stream with "Protocol violation"). `seq` starts at 0 on `stream_start` and increments once per subsequent frame; intermediate `Completed` chunks emit NO frame; `stream_end` reuses the current `seq` value (no increment) exactly like today; the post-loop `finished` terminal frame (Invariant 3, handlers.rs ~2270) increments `seq` and stays untouched.
- PTY discipline: winsize ioctl before exec; discrete keystroke writes ≥0.3 s apart; ANSI-strip matching; occurrence-count waits (`clean(buf).count(needle) >= N`) for repeated UI elements; stub streams must keep sequences consecutive INCLUDING terminal frames.
- Known-harmless noise: `error: daemon terminated` lines around git ops; CRLF warnings on fixture commits.
- Smoke fixtures under `src/test/fixtures/pty/inc6/` mutate on every rerun — restore with `git checkout -- <paths>` before committing.
- Do NOT stage `Cargo.lock` or `dist/` artifacts.

## Key-Symbol Disambiguation (verified against source)

- `brain_core::model::TokenUsage { input_tokens: usize, output_tokens: usize }` — sums across passes.
- `MessageContentBlock` variants (crates/brain-core/src/model.rs:55-86): `Text{text}`, `Thinking{..}`, `ToolUse{id,name,input}`, `ToolResult{tool_use_id,content,is_error}`. `ChatRole::{System,User,Assistant}`. All derive `PartialEq`.
- `ScriptedResponse` field order for `tool_calls` tuples: **(id, name, input)** — `Vec<(String, String, serde_json::Value)>`.
- `DeterministicMockProvider` emits chunks per scripted response in order: Thinking* → **ToolUse chunks FIRST**, then TextDeltas → `Completed{finish_reason, usage:{input_tokens:15, output_tokens:<sum of token byte lengths>}}`.
- `handlers.rs` anchors (as of main@3b57a1fe): gen_request built ~1890; `seq: u64 = 0` ~1898; stream_start emission ~1900-1933; stream fetch ~1935-1939; flags ~1941-1943; drain `match stream_result { Ok(ref mut stream) => loop { tokio::select! …` ~1945; ToolUse arm ~2014-2182; Completed arm ~2183-2215; persistence ~2259-2268; Invariant-3 terminal `finished` ~2270-2301.
- `run_turn_resolving` (copied harness) treats `ftype == "finished" || ftype == "error"` as terminal — `stream_end` precedes `finished` inside the stream and IS collected.
- `start_generation_with_prompt(writer, session_id, prompt)` sends payload keys `sessionId/generationId/messages/model`.

---

### Task 1: Mock multi-response seeding (`BRAIN_MOCK_SCRIPTED_RESPONSES`)

**Files:**
- Modify: `crates/brain-services/src/model/mock.rs`

**Interfaces:**
- Consumes: existing `ScriptedResponse` struct (fields: thinking, tokens, tool_calls, error, finish_reason), existing `scripted_queue: Arc<Mutex<VecDeque<ScriptedResponse>>>`.
- Produces: `fn scripted_queue_from_env_spec(spec: Option<&str>) -> VecDeque<ScriptedResponse>` (private, pure); `ScriptedResponse` gains `Serialize, Deserialize` derives with `#[serde(default)]` on every field (later tasks rely on partial JSON objects deserializing). Constructors seed from `std::env::var("BRAIN_MOCK_SCRIPTED_RESPONSES")`.

- [ ] **Step 1: Write the failing tests**

Append to the existing `#[cfg(test)]` area of `crates/brain-services/src/model/mock.rs` (after `sentinel_tests`):

```rust
#[cfg(test)]
mod scripted_env_tests {
    use super::*;

    #[test]
    fn valid_spec_seeds_queue_in_order() {
        let spec = r#"[
            {"tokens":["Round one text."],"tool_calls":[["call_fb_1","bash",{"command":"echo one"}]],"finish_reason":"tool_use"},
            {"tokens":["Round two wraps up."],"finish_reason":"end_turn"}
        ]"#;
        let queue = scripted_queue_from_env_spec(Some(spec));
        assert_eq!(queue.len(), 2);
        let first = queue[0].clone();
        assert_eq!(first.tool_calls.len(), 1);
        assert_eq!(first.tool_calls[0].0, "call_fb_1");
        assert_eq!(first.tool_calls[0].1, "bash");
        assert_eq!(first.finish_reason.as_deref(), Some("tool_use"));
        assert_eq!(queue[1].tokens, vec!["Round two wraps up.".to_string()]);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let queue = scripted_queue_from_env_spec(Some(r#"[{"tool_calls":[["c","bash",{}]]}]"#));
        assert_eq!(queue.len(), 1);
        assert!(queue[0].thinking.is_none());
        assert!(queue[0].tokens.is_empty());
        assert!(queue[0].error.is_none());
        assert_eq!(queue[0].finish_reason.as_deref(), None);
    }

    #[test]
    fn malformed_spec_yields_empty_queue() {
        assert!(scripted_queue_from_env_spec(Some("{not json")).is_empty());
    }

    #[test]
    fn absent_spec_yields_empty_queue() {
        assert!(scripted_queue_from_env_spec(None).is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-services --lib scripted_env'`
Expected: COMPILE ERROR — `cannot find function scripted_queue_from_env_spec` (plus derive errors once referenced). This is the red state.

- [ ] **Step 3: Implement**

In `crates/brain-services/src/model/mock.rs`:

(a) Extend the imports near the top:

```rust
use serde::{Deserialize, Serialize};
```

(b) Replace the derive line on `ScriptedResponse` (currently `#[derive(Debug, Clone)]`) and add `#[serde(default)]` to each field:

```rust
/// Configuration for a scripted mock response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptedResponse {
    /// Optional thinking tokens.
    #[serde(default)]
    pub thinking: Option<String>,
    /// Text tokens to emit sequentially.
    #[serde(default)]
    pub tokens: Vec<String>,
    /// Optional tool calls to emit.
    #[serde(default)]
    pub tool_calls: Vec<(String, String, serde_json::Value)>,
    /// Optional simulated error to yield.
    #[serde(default)]
    pub error: Option<String>,
    /// Finish reason (defaults to "end_turn").
    #[serde(default)]
    pub finish_reason: Option<String>,
}
```

(The hand-written `impl Default for ScriptedResponse` stays as-is.)

(c) Add the pure seeder directly above `impl DeterministicMockProvider` (after the sentinel fn):

```rust
/// Builds the scripted queue seed from a `BRAIN_MOCK_SCRIPTED_RESPONSES`
/// spec (JSON array of `ScriptedResponse`). Malformed specs warn once and
/// degrade to the default queue so provider behavior never regresses.
fn scripted_queue_from_env_spec(spec: Option<&str>) -> VecDeque<ScriptedResponse> {
    let Some(raw) = spec else {
        return VecDeque::new();
    };
    match serde_json::from_str::<Vec<ScriptedResponse>>(raw) {
        Ok(list) => list.into_iter().collect(),
        Err(e) => {
            tracing::warn!(%e, "ignoring malformed BRAIN_MOCK_SCRIPTED_RESPONSES");
            VecDeque::new()
        }
    }
}
```

(d) In BOTH `new()` and `with_models()`, replace

```rust
scripted_queue: Arc::new(Mutex::new(VecDeque::new())),
```

with

```rust
scripted_queue: Arc::new(Mutex::new(scripted_queue_from_env_spec(
    std::env::var("BRAIN_MOCK_SCRIPTED_RESPONSES").ok().as_deref(),
))),
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-services --lib'`
Expected: all PASS (new scripted_env tests + existing sentinel_tests).

- [ ] **Step 5: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add crates/brain-services/src/model/mock.rs
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "feat(services): seed mock provider from BRAIN_MOCK_SCRIPTED_RESPONSES

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Loop helpers and collectors in handlers.rs

**Files:**
- Modify: `daemon/src/transport/uds/handlers.rs` (append private items + `#[cfg(test)]` module at file bottom)

**Interfaces:**
- Consumes: `brain_core::model::{ChatRole, MessageContentBlock, ModelChatMessage}` (already available via crate path).
- Produces (used verbatim by Task 3):
  - `struct PassToolUse { call_id: String, name: String, input: serde_json::Value }`
  - `struct ToolFeedback { call_id: String, name: String, input: serde_json::Value, output: String, is_error: bool }`
  - `const DENIED_FEEDBACK_TEXT: &str`
  - `fn parse_max_rounds(raw: Option<&str>) -> u32`
  - `fn feedback_messages(pass_text: &str, calls: &[PassToolUse], results: &[ToolFeedback]) -> Vec<brain_core::model::ModelChatMessage>`

- [ ] **Step 1: Write the failing tests**

Append at the very bottom of `daemon/src/transport/uds/handlers.rs`:

```rust
#[cfg(test)]
mod generation_loop_tests {
    use super::*;

    fn call(id: &str) -> PassToolUse {
        PassToolUse {
            call_id: id.to_string(),
            name: "bash".to_string(),
            input: serde_json::json!({"command": "echo hi"}),
        }
    }

    fn executed(id: &str) -> ToolFeedback {
        ToolFeedback {
            call_id: id.to_string(),
            name: "bash".to_string(),
            input: serde_json::json!({"command": "echo hi"}),
            output: "hi\n".to_string(),
            is_error: false,
        }
    }

    #[test]
    fn parse_max_rounds_defaults_on_missing_and_garbage() {
        assert_eq!(parse_max_rounds(None), 8);
        assert_eq!(parse_max_rounds(Some("abc")), 8);
        assert_eq!(parse_max_rounds(Some("")), 8);
    }

    #[test]
    fn parse_max_rounds_parses_and_floors_at_one() {
        assert_eq!(parse_max_rounds(Some("3")), 3);
        assert_eq!(parse_max_rounds(Some("0")), 1);
        assert_eq!(parse_max_rounds(Some("  5  ")), 5);
    }

    #[test]
    fn feedback_messages_order_text_tools_results() {
        let msgs = feedback_messages("Working.", &[call("c1")], &[executed("c1")]);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, brain_core::model::ChatRole::Assistant);
        assert_eq!(
            msgs[0].content[0],
            brain_core::model::MessageContentBlock::Text { text: "Working.".to_string() }
        );
        assert_eq!(
            msgs[0].content[1],
            brain_core::model::MessageContentBlock::ToolUse {
                id: "c1".to_string(),
                name: "bash".to_string(),
                input: serde_json::json!({"command": "echo hi"}),
            }
        );
        assert_eq!(msgs[1].role, brain_core::model::ChatRole::User);
        assert_eq!(
            msgs[1].content[0],
            brain_core::model::MessageContentBlock::ToolResult {
                tool_use_id: "c1".to_string(),
                content: "hi\n".to_string(),
                is_error: false,
            }
        );
    }

    #[test]
    fn feedback_messages_omits_text_block_when_pass_had_no_text() {
        let msgs = feedback_messages("", &[call("c1")], &[executed("c1")]);
        assert_eq!(msgs[0].content.len(), 1);
        assert!(matches!(
            msgs[0].content[0],
            brain_core::model::MessageContentBlock::ToolUse { .. }
        ));
    }

    #[test]
    fn feedback_preserves_multi_call_ordering() {
        let msgs = feedback_messages(
            "",
            &[call("c1"), call("c2")],
            &[executed("c1"), executed("c2")],
        );
        let ids: Vec<String> = msgs[0]
            .content
            .iter()
            .filter_map(|b| match b {
                brain_core::model::MessageContentBlock::ToolUse { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec!["c1".to_string(), "c2".to_string()]);
        let answered: Vec<String> = msgs[1]
            .content
            .iter()
            .filter_map(|b| match b {
                brain_core::model::MessageContentBlock::ToolResult { tool_use_id, .. } => {
                    Some(tool_use_id.clone())
                }
                _ => None,
            })
            .collect();
        assert_eq!(answered, vec!["c1".to_string(), "c2".to_string()]);
    }

    #[test]
    fn denial_feedback_shape_round_trips_through_helper() {
        let denial = ToolFeedback {
            call_id: "c9".to_string(),
            name: "bash".to_string(),
            input: serde_json::json!({}),
            output: DENIED_FEEDBACK_TEXT.to_string(),
            is_error: true,
        };
        let msgs = feedback_messages("", &[call("c9")], &[denial]);
        assert_eq!(
            msgs[1].content[0],
            brain_core::model::MessageContentBlock::ToolResult {
                tool_use_id: "c9".to_string(),
                content: DENIED_FEEDBACK_TEXT.to_string(),
                is_error: true,
            }
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p daemon --lib generation_loop'`
Expected: COMPILE ERROR — cannot find `PassToolUse` / `parse_max_rounds` / `feedback_messages` / `DENIED_FEEDBACK_TEXT`.

- [ ] **Step 3: Implement**

Insert the following block immediately ABOVE the `#[cfg(test)] mod generation_loop_tests` you appended in Step 1:

```rust
/// One tool call observed in the current provider pass, recorded when its
/// ToolUse chunk arrives.
struct PassToolUse {
    call_id: String,
    name: String,
    input: serde_json::Value,
}

/// One resolved tool call from the current pass — executed output or a user
/// denial — destined for the next pass's feedback messages.
struct ToolFeedback {
    call_id: String,
    name: String,
    input: serde_json::Value,
    output: String,
    is_error: bool,
}

/// Fixed content carried by denial feedback entries (spec §4.2).
const DENIED_FEEDBACK_TEXT: &str = "User denied permission for this tool call.";

/// Maximum provider passes per turn when BRAIN_TOOL_MAX_ROUNDS is unset or
/// unparseable (spec §2).
const DEFAULT_MAX_TOOL_ROUNDS: u32 = 8;

/// Parses the per-turn tool-round cap: default 8, floored at 1, garbage ⇒
/// default. Pure so tests never mutate process environment.
fn parse_max_rounds(raw: Option<&str>) -> u32 {
    raw.and_then(|s| s.trim().parse::<u32>().ok())
        .map(|v| v.max(1))
        .unwrap_or(DEFAULT_MAX_TOOL_ROUNDS)
}

/// Builds the provider-visible feedback for a completed pass (spec §4.2): an
/// assistant message carrying the pass text (when non-empty) and ToolUse
/// blocks in arrival order, then a user message carrying one ToolResult per
/// resolved call in the same order.
fn feedback_messages(
    pass_text: &str,
    calls: &[PassToolUse],
    results: &[ToolFeedback],
) -> Vec<brain_core::model::ModelChatMessage> {
    use brain_core::model::{ChatRole, MessageContentBlock};

    let mut assistant_blocks: Vec<MessageContentBlock> = Vec::new();
    if !pass_text.is_empty() {
        assistant_blocks.push(MessageContentBlock::Text {
            text: pass_text.to_string(),
        });
    }
    for c in calls {
        assistant_blocks.push(MessageContentBlock::ToolUse {
            id: c.call_id.clone(),
            name: c.name.clone(),
            input: c.input.clone(),
        });
    }
    let assistant = brain_core::model::ModelChatMessage {
        role: ChatRole::Assistant,
        content: assistant_blocks,
    };

    let user_content = results
        .iter()
        .map(|r| MessageContentBlock::ToolResult {
            tool_use_id: r.call_id.clone(),
            content: r.output.clone(),
            is_error: r.is_error,
        })
        .collect::<Vec<_>>();
    let user = brain_core::model::ModelChatMessage {
        role: ChatRole::User,
        content: user_content,
    };

    vec![assistant, user]
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p daemon --lib'`
Expected: all PASS — new generation_loop_tests plus the existing 30 lib tests.

- [ ] **Step 5: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add daemon/src/transport/uds/handlers.rs
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "feat(daemon): pure helpers for the tool feedback loop

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Rounds loop wired into the generation arm

**Files:**
- Create: `daemon/tests/uds_feedback_loop_tests.rs`
- Modify: `daemon/src/transport/uds/handlers.rs` (generation arm of the `v1/generation/stream` action)

**Interfaces:**
- Consumes: everything from Task 2 (`PassToolUse`, `ToolFeedback`, `DENIED_FEEDBACK_TEXT`, `parse_max_rounds`, `feedback_messages`); Task 1's `BRAIN_MOCK_SCRIPTED_RESPONSES`; the Inc 5 harness shape (`DaemonProcess`, `get_temp_dir`, `get_free_port`, `send_frame`, `read_line_frame`, `open_and_create_session`, `resolve_on_second_connection`, `run_turn_resolving`).
- Produces: the working loop; test-suite helpers `start_test_daemon(extra_env: &[(&str, &str)]) -> DaemonProcess` and `async fn run_turn(reader) -> Vec<Value>` that Task 4 reuses.

- [ ] **Step 1: Write the failing integration test**

Create `daemon/tests/uds_feedback_loop_tests.rs`: copy the FULL contents of `daemon/tests/uds_tool_execution_tests.rs` verbatim, then apply exactly these changes:

(a) Header doc-comment: replace with `//! Increment 6: the agentic feedback loop — tool results feed back into the same turn.`

(b) `get_temp_dir`: prefix `/tmp/bd-toolexec-` → `/tmp/bd-feedback-`.

(c) `start_test_daemon` gains an env parameter and applies it. Replace the whole function with:

```rust
async fn start_test_daemon(extra_env: &[(&str, &str)]) -> DaemonProcess {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let test_dir = get_temp_dir();
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");

    let mut cmd = Command::new(bin_path);
    cmd.arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .env("BRAIN_MOCK_CHUNK_DELAY_MS", "50")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in extra_env {
        cmd.env(key, value);
    }
    let child = cmd.spawn().expect("Failed to start daemon process");

    let mut ready = false;
    for _ in 0..60 {
        if socket_path.exists() && UnixStream::connect(&socket_path).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "Daemon did not bind socket in time");

    DaemonProcess {
        child,
        test_dir,
        socket_path,
    }
}
```

(d) Keep `send_frame`, `read_line_frame`, `open_and_create_session`, `start_generation_with_prompt`, `resolve_on_second_connection`, `run_turn_resolving` EXACTLY as copied (they compile standalone; unused prompt consts may be deleted along with their four `const *_PROMPT` declarations to avoid dead-code warnings).

(e) Add the scripted-turn constants and the two-round happy-path test at the bottom:

```rust
/// Round 1 emits text + one bash call (finish "tool_use"); round 2 sees the
/// fed-back result and finishes cleanly. Byte lengths: "Round one text." ==
/// 15, "Round two wraps up." == 19; mock hardcodes input_tokens 15 per pass.
const TWO_ROUND_SCRIPT: &str = r#"[{"tokens":["Round one text."],"tool_calls":[["call_fb_1","bash",{"command":"echo feedback-round-one"}]],"finish_reason":"tool_use"},{"tokens":["Round two wraps up."],"finish_reason":"end_turn"}]"#;

#[tokio::test]
async fn two_round_turn_feeds_result_back_and_finishes_cleanly() {
    let proc = start_test_daemon(&[("BRAIN_MOCK_SCRIPTED_RESPONSES", TWO_ROUND_SCRIPT)]).await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&proc.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, "run the scripted loop").await;

    let frames = run_turn_resolving(&mut reader, &proc.socket_path, true).await;

    let types: Vec<&str> = frames
        .iter()
        .filter_map(|f| f["type"].as_str())
        .collect();
    // Strictly consecutive sequences across BOTH passes — every frame,
    // including the terminal stream_end, owns exactly one slot.
    for (i, f) in frames.iter().enumerate() {
        assert_eq!(
            f["sequence"].as_u64().unwrap_or_else(|| panic!("frame {i} missing sequence")),
            i as u64,
            "frame {i} ({}) broke consecutiveness",
            types[i]
        );
    }

    // Exactly one executed result; nothing denied.
    assert_eq!(types.iter().filter(|t| **t == "tool_result").count(), 1);
    assert!(!types.contains(&"tool_denied"));

    // Round-2 text arrives AFTER the tool_result frame (fed-back continuation).
    let result_idx = types.iter().position(|t| *t == "tool_result").unwrap();
    let round_two_idx = frames
        .iter()
        .position(|f| {
            f["type"] == "token"
                && f["token"].as_str().unwrap_or("").contains("Round two wraps up.")
        })
        .unwrap();
    assert!(round_two_idx > result_idx);

    // stream_end carries both passes' text, summed usage, clean finish.
    let end = frames
        .iter()
        .find(|f| f["type"] == "stream_end")
        .expect("stream_end present");
    let response = end["response"].as_str().unwrap();
    assert!(response.contains("Round one text."), "response: {response}");
    assert!(response.contains("Round two wraps up."), "response: {response}");
    assert_eq!(end["finish_reason"], "end_turn");
    assert_eq!(end["metadata"]["inputTokens"], 30);
    assert_eq!(end["metadata"]["outputTokens"], 34);
    assert_eq!(types.last(), Some(&"finished"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p daemon --test uds_feedback_loop_tests'`
Expected: FAIL — today the turn ends after pass 1's `Completed`, so `response` lacks "Round two wraps up.", `inputTokens` is 15 (not 30), and the round-two-token-after-result assertion fails. (If anything PASSES unexpectedly, stop and investigate — the red state is load-bearing.)

- [ ] **Step 3: Implement the loop**

All edits in `daemon/src/transport/uds/handlers.rs`, generation arm of `v1/generation/stream`:

**(A) Delete the pre-loop request build** (currently at ~1890-1896):

```rust
            let gen_request = brain_core::model::GenerationRequest {
                model: resolved_model_desc.id.clone(),
                messages: model_messages,
                system_prompt: combined_system_prompt,
                tools: Vec::new(),
                thinking_budget: None,
            };
```

The stream_start emission that follows stays untouched.

**(B) Replace the pre-loop stream fetch** (currently ~1935-1939):

```rust
            // Obtain stream from ModelGateway
            let mut stream_result = app
                .model_gateway()
                .stream_generation(gen_request, cancellation_token.clone())
                .await;
```

with the labeled loop opening (request build + fetch move INSIDE; per-pass collectors declared):

```rust
            'rounds: for round_index in 0..max_rounds {
                if cancellation_token.is_cancelled() {
                    is_cancelled = true;
                    break 'rounds;
                }
                let gen_request = brain_core::model::GenerationRequest {
                    model: resolved_model_desc.id.clone(),
                    messages: model_messages.clone(),
                    system_prompt: combined_system_prompt.clone(),
                    tools: Vec::new(),
                    thinking_budget: None,
                };
                let mut stream_result = app
                    .model_gateway()
                    .stream_generation(gen_request, cancellation_token.clone())
                    .await;

                let mut pass_calls: Vec<PassToolUse> = Vec::new();
                let mut feedback: Vec<ToolFeedback> = Vec::new();
                let mut pass_text = String::new();
                let mut round_completed: Option<(String, brain_core::model::TokenUsage)> = None;

                match stream_result {
```

(The trailing `match stream_result {` splices onto the existing `Ok(ref mut stream) => loop {` block.)

**(C) Declare the turn-level cap and usage accumulator.** Immediately after the existing flag lines (~1941-1943):

```rust
            let mut accumulated_response = String::new();
            let mut is_completed_successfully = false;
            let mut is_cancelled = false;
```

append:

```rust
            let max_rounds =
                parse_max_rounds(std::env::var("BRAIN_TOOL_MAX_ROUNDS").ok().as_deref());
            let mut total_usage =
                brain_core::model::TokenUsage { input_tokens: 0, output_tokens: 0 };
```

(This sits lexically before the `'rounds` label opened in (B) — place it between the flag block and the `// Frame 0: stream_start…` comment if the compiler ordering requires; `max_rounds` must be bound before the `'rounds` loop statement executes. Concretely: put these two bindings right AFTER the flag lines and BEFORE the loop-opening replacement site.)

**(D) TextDelta arm** — alongside the existing `accumulated_response.push_str(&text);` add:

```rust
                                            pass_text.push_str(&text);
```

**(E) ToolUse arm** — immediately after `let tool_name = name.clone();` add (before the packet construction that moves `id/name/input`):

```rust
                                            pass_calls.push(PassToolUse {
                                                call_id: call_id.clone(),
                                                name: tool_name.clone(),
                                                input: input.clone(),
                                            });
```

**(F) Grant branch** — after `let (out_text, is_err, exit_code) = match execution { … };` and BEFORE `let result_packet = …` add:

```rust
                                                feedback.push(ToolFeedback {
                                                    call_id: call_id.clone(),
                                                    name: tool_name.clone(),
                                                    input: packet["toolUse"]["input"].clone(),
                                                    output: out_text.clone(),
                                                    is_error: is_err,
                                                });
```

**(G) Deny branch** — at the end of the `if !granted { … }` block (after `denied_packet` is written) add:

```rust
                                                feedback.push(ToolFeedback {
                                                    call_id: call_id.clone(),
                                                    name: tool_name.clone(),
                                                    input: packet["toolUse"]["input"].clone(),
                                                    output: DENIED_FEEDBACK_TEXT.to_string(),
                                                    is_error: true,
                                                });
```

**(H) Completed arm** — replace the ENTIRE arm (currently ~2183-2215: sets `is_completed_successfully`, builds/writes `end_packet`, `break`) with:

```rust
                                        brain_core::model::GenerationChunk::Completed { finish_reason, usage } => {
                                            // Silent on the wire: the loop decides
                                            // whether this pass continues or emits
                                            // stream_end (spec §4.4). Restore the
                                            // sequence slot the shared chunk-entry
                                            // increment consumed — a burned slot here
                                            // would open a wire gap and abort the
                                            // shell's stream guard.
                                            seq -= 1;
                                            round_completed = Some((finish_reason, usage));
                                            break;
                                        }
```

Without the `seq -= 1` restoration, every absorbed mid-loop `Completed` burns a sequence number and the resulting gap trips the shell's Protocol-violation guard. The counterpart rule lives in step (K): the TERMINAL stream_end takes a fresh `seq += 1` — historically the Completed consumption WAS stream_end's slot (the legacy `uds_generation_tests` strict-monotonic assertion pins this), so restoration mid-loop plus fresh-slot-at-termination preserves exact legacy numbering.

**(I) Error-in-stream arm** — in `Some(Err(err)) => { … break; }` change `break;` to `break 'rounds;` (keep the error-frame write).

**(J) Stream-end-without-completed arm** — in `None => { is_completed_successfully = true; break; }` change `break;` to `break 'rounds;`.

**(K) Post-match continuation decision.** The outer match currently ends with:

```rust
                Err(err) => {
                    seq += 1;
                    let err_packet = serde_json::json!({ … });
                    let mut err_json = serde_json::to_string(&err_packet)?;
                    err_json.push('\n');
                    writer.write_all(err_json.as_bytes()).await?;
                    writer.flush().await?;
                }
            }
```

Immediately after that closing brace of `match stream_result`, insert:

```rust
                if is_cancelled || round_completed.is_none() {
                    break 'rounds;
                }

                let (finish_reason, usage) = round_completed.take().unwrap();
                total_usage.input_tokens += usage.input_tokens;
                total_usage.output_tokens += usage.output_tokens;

                let terminate_reason: Option<String> = if feedback.is_empty() {
                    Some(finish_reason)
                } else if round_index + 1 >= max_rounds {
                    Some("max_tool_rounds".to_string())
                } else {
                    model_messages.extend(feedback_messages(
                        &pass_text,
                        &pass_calls,
                        &feedback,
                    ));
                    None
                };

                if let Some(reason) = terminate_reason {
                    is_completed_successfully = true;
                    // Terminal stream_end owns a fresh sequence slot (the
                    // legacy strict-monotonic contract); mid-loop Completions
                    // restored theirs above.
                    seq += 1;
                    let total_duration_ms = gen_start_time.elapsed().as_millis() as u64;
                    let end_packet = serde_json::json!({
                        "type": "stream_end",
                        "generation_id": generation_id,
                        "session_id": session_id_str,
                        "sequence": seq,
                        "status": "completed",
                        "response": accumulated_response,
                        "finish_reason": reason,
                        "metadata": {
                            "inputTokens": total_usage.input_tokens,
                            "outputTokens": total_usage.output_tokens,
                            "telemetry": {
                                "generation_id": generation_id,
                                "session_id": session_id_str,
                                "retrieval_epoch_id": context_snapshot.epoch_id,
                                "candidates_retrieved": context_snapshot.provenance.count,
                                "memories_assembled": context_snapshot.items.len(),
                                "context_tokens_used": context_snapshot.token_count,
                                "assembly_latency_ms": assembly_latency_ms,
                                "total_duration_ms": total_duration_ms,
                                "finish_reason": reason,
                            }
                        }
                    });
                    let mut end_json = serde_json::to_string(&end_packet)?;
                    end_json.push('\n');
                    writer.write_all(end_json.as_bytes()).await?;
                    writer.flush().await?;
                    break 'rounds;
                }
            } // 'rounds
```

Everything downstream (persistence block, Invariant-3 terminal `finished`, `gen_guard.defuse()`, `continue`) remains untouched.

**(L) Compile-order note:** if rustc reports `max_rounds`/`total_usage` used-before-declared given the (A)/(B)/(C) splice points, move the two `let` bindings from (C) to just ABOVE the `'rounds:` label — they have no dependency on anything introduced in between.

- [ ] **Step 4: Run the new test AND the full daemon suite**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p daemon'`
Expected: `two_round_turn_feeds_result_back_and_finishes_cleanly` PASSes; ALL prior suites still PASS (`uds_tool_execution_tests` 4/4 proves single-pass wire compatibility byte-for-byte).

- [ ] **Step 5: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add daemon/src/transport/uds/handlers.rs daemon/tests/uds_feedback_loop_tests.rs
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "feat(daemon): agentic feedback loop across provider passes

Resolved tool calls (executed or denied) are appended as assistant/user
messages and stream_generation is re-invoked within the turn, capped by
BRAIN_TOOL_MAX_ROUNDS (default 8). stream_end defers to the final pass.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Pin deny-continues, cap enforcement, single-pass regression

**Files:**
- Modify: `daemon/tests/uds_feedback_loop_tests.rs` (append tests + one helper)

**Interfaces:**
- Consumes: `start_test_daemon(&[(&str, &str)])`, `open_and_create_session`, `start_generation_with_prompt`, `run_turn_resolving` from Task 3's file.
- Produces: `async fn run_turn(reader) -> Vec<Value>` (no-resolution collector used by the regression test).

These tests pin behavior Task 3 shipped. Expected outcome is PASS on first run; ANY failure exposes a real defect — stop and fix before committing.

- [ ] **Step 1: Add the helper and three tests**

Append to `daemon/tests/uds_feedback_loop_tests.rs`:

```rust
/// Collects frames until the terminal event WITHOUT resolving permissions
/// (turns that request nothing never park).
async fn run_turn(
    reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
) -> Vec<serde_json::Value> {
    let mut frames = Vec::new();
    loop {
        let f = tokio::time::timeout(Duration::from_secs(10), read_line_frame(reader))
            .await
            .expect("frame timeout");
        let ftype = f["type"].as_str().unwrap_or("").to_string();
        frames.push(f);
        if ftype == "finished" || ftype == "error" {
            return frames;
        }
    }
}

const DENY_SCRIPT: &str = TWO_ROUND_SCRIPT;

const THREE_ROUND_SCRIPT: &str = r#"[{"tool_calls":[["call_cap_1","bash",{"command":"echo cap-one"}]],"finish_reason":"tool_use"},{"tokens":["ROUND-TWO-MARKER"],"tool_calls":[["call_cap_2","bash",{"command":"echo cap-two"}]],"finish_reason":"tool_use"},{"tokens":["ROUND-THREE-MARKER"],"finish_reason":"end_turn"}]"#;

const SINGLE_PASS_SCRIPT: &str =
    r#"[{"tokens":["Plain single pass."],"finish_reason":"end_turn"}]"#;

#[tokio::test]
async fn denied_call_feeds_back_and_loop_continues() {
    let proc = start_test_daemon(&[("BRAIN_MOCK_SCRIPTED_RESPONSES", DENY_SCRIPT)]).await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&proc.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, "refuse the scripted call").await;

    let frames = run_turn_resolving(&mut reader, &proc.socket_path, false).await;
    let types: Vec<&str> = frames.iter().filter_map(|f| f["type"].as_str()).collect();

    assert!(types.contains(&"tool_denied"), "types: {types:?}");
    assert!(!types.contains(&"tool_result"), "denied turns never execute");
    // THE contract: the model still produced round-2 text after the denial.
    let end = frames.iter().find(|f| f["type"] == "stream_end").unwrap();
    assert!(end["response"]
        .as_str()
        .unwrap()
        .contains("Round two wraps up."));
    assert_eq!(end["finish_reason"], "end_turn");
    assert_eq!(end["metadata"]["inputTokens"], 30);
}

#[tokio::test]
async fn round_cap_stops_the_loop_gracefully() {
    let proc = start_test_daemon(&[
        ("BRAIN_MOCK_SCRIPTED_RESPONSES", THREE_ROUND_SCRIPT),
        ("BRAIN_TOOL_MAX_ROUNDS", "1"),
    ])
    .await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&proc.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, "cap me").await;

    let frames = run_turn_resolving(&mut reader, &proc.socket_path, true).await;
    let types: Vec<&str> = frames.iter().filter_map(|f| f["type"].as_str()).collect();

    assert_eq!(types.iter().filter(|t| **t == "tool_result").count(), 1);
    let end = frames.iter().find(|f| f["type"] == "stream_end").unwrap();
    assert_eq!(end["finish_reason"], "max_tool_rounds");
    assert!(!end["response"].as_str().unwrap().contains("ROUND-TWO-MARKER"));
    assert_eq!(end["metadata"]["inputTokens"], 15); // exactly one pass ran
}

#[tokio::test]
async fn plain_single_pass_wire_shape_is_unchanged() {
    let proc = start_test_daemon(&[("BRAIN_MOCK_SCRIPTED_RESPONSES", SINGLE_PASS_SCRIPT)]).await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&proc.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, "just talk").await;

    let frames = run_turn(&mut reader).await;
    let types: Vec<&str> = frames.iter().filter_map(|f| f["type"].as_str()).collect();

    assert_eq!(types.first(), Some(&"stream_start"));
    assert_eq!(types.last(), Some(&"finished"));
    assert!(!types.contains(&"tool_use"));
    assert!(!types.contains(&"tool_result"));
    for (i, f) in frames.iter().enumerate() {
        assert_eq!(f["sequence"].as_u64().unwrap(), i as u64);
    }
    let end = frames.iter().find(|f| f["type"] == "stream_end").unwrap();
    assert_eq!(end["response"], "Plain single pass.");
    assert_eq!(end["finish_reason"], "end_turn");
    assert_eq!(end["metadata"]["inputTokens"], 15);
}
```

- [ ] **Step 2: Run the suite**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p daemon --test uds_feedback_loop_tests'`
Expected: 4/4 PASS. Investigate any failure as a real bug in Task 3's loop — fix in `handlers.rs`, rerun, and include the fix in this task's commit.

- [ ] **Step 3: Run the full daemon suite**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p daemon'`
Expected: all suites PASS.

- [ ] **Step 4: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add daemon/tests/uds_feedback_loop_tests.rs
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "test(daemon): pin feedback-loop deny, cap, and single-pass contracts

Co-Authored-By: Claude <noreply@anthropic.com>"
```

(If Step 2 forced a `handlers.rs` fix, add that path to the `git add` line.)

---

### Task 5: PTY smoke — live TUI proof of the loop

**Files:**
- Create: `scripts/ptySmokeInc6.py`
- Create (runtime-mutating fixtures, committed once): `packages/brain-shell/src/test/fixtures/pty/inc6/*.txt`

**Interfaces:**
- Consumes: the Inc 5 smoke's discipline block (winsize ioctl, pump/expect/expect_count/snapshot helpers) — copied verbatim with Inc 6 paths.
- Produces: exit 0 = the shell renders a two-dialog, two-card loop and a denial continuation without any shell-side changes.

Stub wire scripts (sequences STRICTLY consecutive, terminal frame included — a gap aborts the shell):

Turn A (allow twice): `tool_use(call_a, echo round-one-stub)(0)` → `perm(1)` → [wait resolve] → granted: `tool_result "round-one-stub\n"(2)` → `tool_use(call_b, echo round-two-stub)(3)` → `perm(4)` → [wait resolve] → granted: `tool_result "round-two-stub\n"(5)` → `token "Loop closed."(6)` → `finished completed(7)`.
Turn B (same stub branch, deny path): `tool_use(call_a, echo round-one-stub)(0)` → `perm(1)` → [wait] → denied: `tool_denied(2)` → `token "Understood, moving on."(3)` → `finished completed(4)`. Sequence numbering restarts per turn; both paths stay strictly consecutive.

- [ ] **Step 1: Write the script**

Create `scripts/ptySmokeInc6.py` — copy `scripts/ptySmokeInc5.py` verbatim, then apply exactly these changes:

(a) Constants: `SOCK = "/tmp/brain-inc6-smoke.sock"`, `FRAMES_FILE = "/tmp/brain-inc6-smoke-requests.jsonl"`, `FIXTURE_DIR = ".../src/test/fixtures/pty/inc6"`, `CONFIG_FILE = "/tmp/brain-inc6-smoke-config.json"`, session id string in create-reply `"stub-session-6"`. Docstring: Increment 6 purpose.

(b) Replace the entire `elif act == "v1/generation/stream":` branch with:

```python
                    elif act == "v1/generation/stream":
                        # Two-round turn: result feeds back, model issues a
                        # SECOND call, then closes with text. Mirrors the Inc 6
                        # daemon loop on the wire.
                        reply({"type": "tool_use", "toolUse": {"id": "call_a",
                               "name": "bash", "input": {"command": "echo round-one-stub"}},
                               "sequence": 0})
                        time.sleep(0.2)
                        reply({"type": "tool_permission_requested", "call_id": "call_a",
                               "tool_name": "bash", "input": {"command": "echo round-one-stub"},
                               "reason": "shell access", "sequence": 1})
                        evt = threading.Event(); PERM_EVENTS["call_a"] = evt
                        granted_a = bool(evt.wait(timeout=10) and PERM_GRANTED.get("call_a"))
                        if granted_a:
                            reply({"type": "tool_result", "call_id": "call_a",
                                   "tool_name": "bash", "output": "round-one-stub\n",
                                   "is_error": False, "exit_code": 0, "sequence": 2})
                            time.sleep(0.2)
                            reply({"type": "tool_use", "toolUse": {"id": "call_b",
                                   "name": "bash", "input": {"command": "echo round-two-stub"}},
                                   "sequence": 3})
                            time.sleep(0.2)
                            reply({"type": "tool_permission_requested", "call_id": "call_b",
                                   "tool_name": "bash", "input": {"command": "echo round-two-stub"},
                                   "reason": "shell access", "sequence": 4})
                            evt_b = threading.Event(); PERM_EVENTS["call_b"] = evt_b
                            granted_b = bool(evt_b.wait(timeout=10) and PERM_GRANTED.get("call_b"))
                            if granted_b:
                                reply({"type": "tool_result", "call_id": "call_b",
                                       "tool_name": "bash", "output": "round-two-stub\n",
                                       "is_error": False, "exit_code": 0, "sequence": 5})
                                time.sleep(0.2)
                                reply({"type": "token", "token": "Loop closed.", "sequence": 6})
                                time.sleep(0.3)
                                reply({"type": "finished", "status": "completed", "sequence": 7})
                            else:
                                reply({"type": "tool_denied", "call_id": "call_b",
                                       "tool_name": "bash", "sequence": 5})
                                time.sleep(0.2)
                                reply({"type": "token", "token": "Second call refused.",
                                       "sequence": 6})
                                time.sleep(0.3)
                                reply({"type": "finished", "status": "completed", "sequence": 7})
                        else:
                            # Turn B path shares this branch: denial feeds back,
                            # the model keeps talking, turn completes normally.
                            reply({"type": "tool_denied", "call_id": "call_a",
                                   "tool_name": "bash", "sequence": 2})
                            time.sleep(0.2)
                            reply({"type": "token", "token": "Understood, moving on.",
                                   "sequence": 3})
                            time.sleep(0.3)
                            reply({"type": "finished", "status": "completed", "sequence": 4})
```

(c) Replace BOTH smoke flows after Flow A (welcome assertions unchanged) with:

```python
# ── Flow B: two allowed rounds render two cards and closing text ───────────
def frames_log():
    try:
        with open(FRAMES_FILE) as f:
            return f.read()
    except Exception:
        return ""

os.write(fd, b"run the stub loop")
pump(0.3)
os.write(fd, b"\r")
ok &= expect("card-one", "round-one-stub")
ok &= expect_count("dialog-one", "Permission required", 1)
os.write(fd, b"y"); pump(0.5)          # allow call_a (settle before next key)
ok &= expect_count("dialog-two", "Permission required", 2)
pump(1.2)                               # settle: dialog must mount before key
snapshot("loop-second-permission")
os.write(fd, b"y"); pump(0.5)          # allow call_b
ok &= expect("card-two-output", "round-two-stub")
ok &= expect("closing-text", "Loop closed.")
deadline = time.time() + 6
wire_two = frames_log().count('"v1/tool/resolve"') >= 2
print(("PASS" if wire_two else "FAIL") + " two-resolves-on-wire")
ok &= wire_two
snapshot("loop-complete")

# ── Flow C: denying the first call still lets the model finish ─────────────
pump(0.8)   # let the previous turn settle so submit isn't swallowed
frames_before = frames_log()
os.write(fd, b"now refuse it")
pump(0.3)
os.write(fd, b"\r")
ok &= expect_count("dialog-three", "Permission required", 3)
pump(1.2)
snapshot("deny-pending")
os.write(fd, b"n"); pump(0.5)
ok &= expect("denied-notice", "Denied bash")
ok &= expect("post-deny-text", "Understood, moving on.")
# Wire truth: this turn emitted NO tool_result — only the denial rode back.
frames_delta = frames_log()[len(frames_before):]
no_result = "tool_result" not in frames_delta
print(("PASS" if no_result else "FAIL") + " denied-turn-executes-nothing")
ok &= no_result
snapshot("deny-continuation")
```

(d) Keep teardown (Ctrl-C, SIGKILL, `sys.exit(0 if ok else 1)`) exactly as copied. Delete Flow D1/D2 remnants entirely — nothing from the old flows survives except Flow A.

- [ ] **Step 2: Run the smoke**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && python3 scripts/ptySmokeInc6.py'`
Expected: every step prints PASS; exit code 0. If a keystroke lands in the composer instead of a fresh dialog, extend the settle `pump()` before that key (Inc 5 lesson) — never shorten waits to make it pass.

- [ ] **Step 3: Restore mutated fixtures, then commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain checkout -- packages/brain-shell/src/test/fixtures/pty/inc6/
git -C /Users/ritikpathania/Developer/PyCharm/brain add scripts/ptySmokeInc6.py packages/brain-shell/src/test/fixtures/pty/inc6
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "test(smoke): inc6 PTY proof of the two-round loop and deny continuation

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: Full gates on the finished increment

**Files:**
- None created. Verification only; commit only if a gate forced a fix.

- [ ] **Step 1: Shell suite**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test'`
Expected: same counts as Inc 5 baseline — 231+ passing, the SAME 5 documented failures (MemoryContextTransformer API drift). Shell production files were untouched; any NEW failure stops the increment.

- [ ] **Step 2: Rust suites (workspace slice)**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-services -p daemon'`
Expected: 0 failures. Roughly: brain-services lib (sentinel 3 + scripted-env 4), daemon lib (prior 30 + generation_loop 6), suites generation 3 + permission 3 + tool-execution 4 + feedback-loop 4.

- [ ] **Step 3: Build gate**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun build src/main.tsx --outdir dist --target bun'`
Expected: successful bundle. Do not commit `dist/`.

- [ ] **Step 4: Vendor-concept scan over increment-touched sources**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && git diff --name-only 3b57a1fe..HEAD -- crates daemon packages scripts | xargs grep -l -i -E "anthropic|api\.anthropic|claude" 2>/dev/null'`
Expected: empty output (docs/ excluded deliberately).

- [ ] **Step 5: Re-run smoke from a clean tree**

Run: `git -C /Users/ritikpathania/Developer/PyCharm/brain status --porcelain` (fix stray mutations first), then `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && python3 scripts/ptySmokeInc6.py'; echo EXIT=$?`
Expected: EXIT=0. Restore fixtures again afterward if they show dirty.

- [ ] **Step 6: Report**

Summarize: commits landed (short hashes + subjects), test counts per suite, smoke result, and any deviations from this plan. If every gate is green, the increment is complete pending the finishing-a-development-branch flow.

---

## Self-Review Record

- **Spec coverage:** §2 decisions → Tasks 2/3 (cap parse, deny feedback); §3 architecture → Task 3 edits (A)-(K); §4.1 collectors → Task 2 structs + Task 3 pushes (E)-(G); §4.2 helpers → Task 2; §4.3 mock seeding → Task 1; §4.4 wire rules → Task 3 test assertions (consecutive sequences, silent intermediate Completed, summed usage, deferred stream_end) + Task 4 regression; §5 errors → covered by unchanged arms ((I)/(J) preserve semantics) + cap termination (K); §6 non-goals → no tasks touch those areas; §7 testing → Tasks 2-5 mirror it 1:1; §8 constraints → Global Constraints.
- **Placeholder scan:** none — every code step carries full, compilable source.
- **Type consistency:** `PassToolUse`/`ToolFeedback` field names identical across Tasks 2 and 3; `feedback_messages(pass_text, calls, results)` signature matches call site; `start_test_daemon(&[(&str, &str)])` matches all four test usages; `run_turn` defined in Task 4 before its single use in the same task.
