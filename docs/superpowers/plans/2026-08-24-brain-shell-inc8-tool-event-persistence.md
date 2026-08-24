# Tool-Event Persistence (Inc 8) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist every agentic-loop tool outcome (executed and denied) into the session transcript as `MessageRole::Tool` messages carrying a typed JSON envelope — no wire, sequence, or permission changes.

**Architecture:** One new `MessageRole` variant in brain-domain; a pure 4KB truncation helper in the daemon; two insertion points in the rounds loop's granted/denied branches that write through the existing `session_aggregate.add_message` + `storage.save_session` idiom, immediately per outcome, best-effort. Integration tests read records back over UDS via `v1/session/load`.

**Tech Stack:** Rust (brain-domain, brain-daemon crates), tokio, serde_json; Bun/Ink shell untouched.

**Spec:** `docs/superpowers/specs/2026-08-24-brain-shell-inc8-tool-event-persistence-design.md`

## Global Constraints

- Branch `feature/brain-shell-inc8-tool-event-persistence` forked from current `main` (`b37db67e`, which contains the Inc 7 merge `f9ae5c35` plus this spec).
- Every cargo invocation on this Mac needs the rpath wrapper:
  `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo ...'`
- The daemon package is named **`brain-daemon`**, not `daemon`.
- The working tree carries ~1k files of pre-existing user WIP. NEVER stage anything except explicitly-named paths; NEVER stash, never wholesale-checkout, never discard Cargo.lock.
- Commits: explicit-path `git add <paths>` only; trailer `Co-Authored-By: Claude <noreply@anthropic.com>`.
- Known-harmless noise: `error: daemon terminated` around git ops; CRLF warnings.
- Advertisement never authorizes execution; persistence observes outcomes and never gates anything. The permission round-trip stays the sole execution authority.
- Baselines before this increment: daemon lib **39 passed / 0 failed**; UDS feedback **4**, generation 3, adversarial 6, tool-execution 4, permission 3, memory 5, load 4, product 6, lifecycle 9, soak 3; brain-tools integration 6; brain-services lib 44/0; shell suite **231 pass / 5 documented fails**; PTY smoke 14/14. Sole permitted failure anywhere remains the pre-existing untracked `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers`.
- Vendor-concept scan greps only ADDED lines since the spec commit:
  `git diff b37db67e..HEAD -- crates daemon packages scripts | grep '^+' | grep -icE "anthropic|api\.anthropic|claude"` → expect `0`.

---

### Task 1: `MessageRole::Tool` variant + exhaustive-match fixes

**Files:**
- Test: `crates/brain-domain/tests/message_role_tool_tests.rs` (new)
- Modify: `crates/brain-domain/src/entities.rs:17-45` (variant + Display + FromStr)
- Modify: `daemon/src/transport/uds/handlers.rs:416-420` (exhaustive match arm)
- Modify: `crates/brain-python/src/api.rs:275-280` (exhaustive match arm)

**Interfaces:**
- Consumes: existing `MessageRole` (`#[derive(... Copy ..., PartialEq, Serialize, Deserialize)] #[serde(rename_all = "lowercase")]`) — serde emits `"tool"` for a `Tool` variant with zero extra code.
- Produces: `MessageRole::Tool` usable by Tasks 3–4 as `brain_domain::MessageRole::Tool`; wire role string `"tool"` produced by `v1/session/load` (Tasks 3–4 assertions depend on it).

- [ ] **Step 1: Write the failing test**

Create `crates/brain-domain/tests/message_role_tool_tests.rs`:

```rust
//! Inc 8: the Tool message role persists agentic-loop tool outcomes into
//! session transcripts.
use brain_domain::MessageRole;
use std::str::FromStr;

#[test]
fn tool_variant_displays_as_lowercase_tool() {
    assert_eq!(MessageRole::Tool.to_string(), "tool");
}

#[test]
fn tool_variant_serializes_and_deserializes_as_tool() {
    let json = serde_json::to_string(&MessageRole::Tool).unwrap();
    assert_eq!(json, r#""tool""#);
    let back: MessageRole = serde_json::from_str(&json).unwrap();
    assert_eq!(back, MessageRole::Tool);
}

#[test]
fn tool_variant_parses_from_str() {
    assert_eq!(MessageRole::from_str("tool").unwrap(), MessageRole::Tool);
}
```

(`serde_json` is already a main dependency of brain-domain — no Cargo.toml change.)

- [ ] **Step 2: Run to verify it fails**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-domain --test message_role_tool_tests 2>&1 | tail -5'`
Expected: FAIL to compile with `no variant or associated item named 'Tool' found` (red-by-typecheck is the correct red for an enum addition).

- [ ] **Step 3: Add the variant and both match arms**

In `crates/brain-domain/src/entities.rs`:

(a) Enum — after the `System` variant:

```rust
    /// Message containing system prompt instructions.
    System,
    /// Agentic-loop tool outcome persisted as part of the transcript (Inc 8).
    Tool,
}
```

(b) `Display` — after the `System` arm:

```rust
            Self::System => write!(f, "system"),
            Self::Tool => write!(f, "tool"),
```

(c) `FromStr` — add an explicit arm ahead of the catch-all:

```rust
            "system" => Self::System,
            "tool" => Self::Tool,
```

In `daemon/src/transport/uds/handlers.rs` (~:416-420, inside `v1/session/load`), extend the exhaustive match:

```rust
                                    let role_str = match m.role {
                                        brain_domain::MessageRole::User => "user",
                                        brain_domain::MessageRole::Assistant => "assistant",
                                        brain_domain::MessageRole::System => "system",
                                        brain_domain::MessageRole::Tool => "tool",
                                    };
```

In `crates/brain-python/src/api.rs` (~:275-280), same shape:

```rust
                match msg.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                    MessageRole::Tool => "tool",
                }
```

- [ ] **Step 4: Run to verify green across every touched crate**

Run:
```bash
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-domain --test message_role_tool_tests 2>&1 | grep "^test result" && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo check -p brain-daemon -p brain-python 2>&1 | grep -E "^error" ; echo CHECK_DONE'
```
Expected: `test result: ok. 3 passed; 0 failed`; no compile errors from either crate (`CHECK_DONE` printed).

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/tests/message_role_tool_tests.rs crates/brain-domain/src/entities.rs daemon/src/transport/uds/handlers.rs crates/brain-python/src/api.rs
git commit -m "feat(domain): MessageRole::Tool variant for transcript tool events

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Pure truncation helper + unit tests

**Files:**
- Modify: `daemon/src/transport/uds/handlers.rs` (helper just above the trailing test modules at ~:2927; new `tool_event_tests` module appended at end of file)

**Interfaces:**
- Consumes: nothing (pure function).
- Produces: `fn truncate_tool_output(output: &str) -> String` and `const TOOL_EVENT_OUTPUT_LIMIT_BYTES: usize = 4096;` — Task 3 calls exactly `truncate_tool_output(&out_text)`.

- [ ] **Step 1: Write the failing tests**

Append at the very end of `daemon/src/transport/uds/handlers.rs`:

```rust

#[cfg(test)]
mod tool_event_tests {
    use super::*;

    #[test]
    fn under_and_at_limit_pass_through_unchanged() {
        let small = "short output".to_string();
        assert_eq!(truncate_tool_output(&small), small);
        let exact = "a".repeat(TOOL_EVENT_OUTPUT_LIMIT_BYTES);
        assert_eq!(truncate_tool_output(&exact), exact);
        assert!(!truncate_tool_output(&exact).contains("[truncated]"));
    }

    #[test]
    fn over_limit_ascii_is_cut_with_marker() {
        let big = "b".repeat(TOOL_EVENT_OUTPUT_LIMIT_BYTES + 100);
        let cut = truncate_tool_output(&big);
        assert!(cut.ends_with("\n…[truncated]"));
        // Body holds at most the limit bytes; the marker is appended after.
        assert!(cut.len() < TOOL_EVENT_OUTPUT_LIMIT_BYTES + "\n…[truncated]".len() + 8);
    }

    #[test]
    fn multibyte_output_cuts_on_char_boundary_without_panicking() {
        // 'é' is 2 bytes; 3000 copies = 6000 bytes > 4096, odd cut candidates land mid-char.
        let big = "é".repeat(3000);
        let cut = truncate_tool_output(&big);
        assert!(cut.ends_with("\n…[truncated]"));
        assert!(cut.is_char_boundary(cut.len() - "\n…[truncated]".len()));
    }

    #[test]
    fn empty_output_stays_empty() {
        assert_eq!(truncate_tool_output(""), "");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --lib tool_event_tests 2>&1 | tail -5'`
Expected: FAIL to compile — `cannot find function 'truncate_tool_output'` / `cannot find value 'TOOL_EVENT_OUTPUT_LIMIT_BYTES'`.

- [ ] **Step 3: Implement the helper**

Immediately ABOVE the `#[cfg(test)]` line of `mod generation_loop_tests` (~:2927, i.e., between the last production item and that module), insert:

```rust
/// Inc 8: persisted tool-event outputs are bounded so sessions stay small;
/// wire frames keep full text. Mirrors BashTool's marker idiom.
const TOOL_EVENT_OUTPUT_LIMIT_BYTES: usize = 4096;

fn truncate_tool_output(output: &str) -> String {
    if output.len() <= TOOL_EVENT_OUTPUT_LIMIT_BYTES {
        return output.to_string();
    }
    let mut cut = TOOL_EVENT_OUTPUT_LIMIT_BYTES;
    while cut > 0 && !output.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut out = output[..cut].to_string();
    out.push_str("\n…[truncated]");
    out
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --lib 2>&1 | grep -E "^error|^warning: unused|test result"'`
Expected: **43 passed / 0 failed** (39 baseline + 4 new), no new warnings.

- [ ] **Step 5: Commit**

```bash
git add daemon/src/transport/uds/handlers.rs
git commit -m "feat(daemon): bounded-output helper for persisted tool events

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Executed-path persistence + integration test

**Files:**
- Modify: `daemon/src/transport/uds/handlers.rs` (granted branch, ~:2117-2202)
- Test: `daemon/tests/uds_feedback_loop_tests.rs` (append one test)

**Interfaces:**
- Consumes: `truncate_tool_output(&str) -> String` (Task 2); `brain_domain::MessageRole::Tool` (Task 1); in-scope bindings `session_aggregate` / `storage` / `parsed_session_id` / `call_id` / `tool_name` / `packet` / `out_text` / `is_err` / `exit_code` (all live in the granted branch today).
- Produces: executed outcomes persist as one `role:"tool"` transcript record per call, readable via `v1/session/load` with `body.session.messages[].content` holding the §3.1 envelope.

- [ ] **Step 1: Write the failing integration test**

Append at the end of `daemon/tests/uds_feedback_loop_tests.rs`:

```rust
/// Loads the session back over UDS and returns the raw messages array from
/// the v1/session/load reply body.
async fn load_session_messages(
    socket_path: &std::path::Path,
    session_id: &str,
) -> Vec<serde_json::Value> {
    let loader = UnixStream::connect(socket_path).await.unwrap();
    let (lreader, mut lwriter) = loader.into_split();
    let mut lbuf = BufReader::new(lreader);
    send_frame(
        &mut lwriter,
        &serde_json::json!({
            "version": "1.0",
            "type": "Request",
            "id": 900,
            "action": "v1/session/load",
            "body": serde_json::json!({ "sessionId": session_id }).to_string()
        }),
    )
    .await;
    let reply = read_line_frame(&mut lbuf).await;
    let body: serde_json::Value = if let Some(s) = reply["body"].as_str() {
        serde_json::from_str(s).unwrap()
    } else {
        reply["body"].clone()
    };
    body["session"]["messages"]
        .as_array()
        .expect("messages array")
        .clone()
}

#[tokio::test]
async fn executed_tool_outcome_is_persisted_as_session_tool_message() {
    let proc = start_test_daemon(&[("BRAIN_MOCK_SCRIPTED_RESPONSES", TWO_ROUND_SCRIPT)]).await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&proc.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, "persist me").await;
    run_turn_resolving(&mut reader, &proc.socket_path, true).await;

    let messages = load_session_messages(&proc.socket_path, &session_id).await;

    // Transcript order: user prompt first, assistant completion last.
    assert_eq!(messages.first().unwrap()["role"], "user");
    assert_eq!(messages.last().unwrap()["role"], "assistant");

    // Exactly one tool event, shaped per spec §3.1.
    let tools: Vec<&serde_json::Value> =
        messages.iter().filter(|m| m["role"] == "tool").collect();
    assert_eq!(tools.len(), 1, "messages: {messages:?}");
    let env: serde_json::Value =
        serde_json::from_str(tools[0]["content"].as_str().unwrap()).unwrap();
    assert_eq!(env["type"], "tool_event");
    assert_eq!(env["v"], 1);
    assert_eq!(env["name"], "bash");
    assert_eq!(env["input"]["command"], "echo feedback-round-one");
    assert_eq!(env["outcome"], "executed");
    assert_eq!(env["is_error"], false);
    assert_eq!(env["exit_code"], 0);
    assert_eq!(env["output"], "feedback-round-one\n");
    assert!(env["duration_ms"].is_u64());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --test uds_feedback_loop_tests executed_tool_outcome 2>&1 | tail -8'`
Expected: FAIL at `assert_eq!(tools.len(), 1, ...)` with `"messages: [...]"` showing zero `role:"tool"` entries (no compile errors).

- [ ] **Step 3: Wire the granted path**

In `handlers.rs`, granted branch. Three edits:

(a) Timing capture — immediately BEFORE the existing lines (~:2145):

```rust
                                                seq += 1;
                                                let execution =
```

insert:

```rust
                                                let tool_exec_started = std::time::Instant::now();
```

(so the timer starts before `executor.execute` resolves).

(b) Envelope build — immediately AFTER the existing `feedback.push(ToolFeedback { ... });` block (~:2179-2185), BEFORE the `let result_packet = serde_json::json!({` line:

```rust
                                                // Inc 8: persist this outcome into
                                                // the session transcript. Best-effort:
                                                // persistence never blocks generation.
                                                let envelope = serde_json::json!({
                                                    "type": "tool_event",
                                                    "v": 1,
                                                    "call_id": call_id.clone(),
                                                    "name": tool_name.clone(),
                                                    "input": packet["toolUse"]["input"].clone(),
                                                    "outcome": "executed",
                                                    "is_error": is_err,
                                                    "exit_code": exit_code,
                                                    "output": truncate_tool_output(&out_text),
                                                    "duration_ms": tool_exec_started.elapsed().as_millis() as u64,
                                                });
```

(c) Persist — immediately AFTER the existing `writer.flush().await?;` that emits `result_packet` (~:2198-2201), still inside the granted block:

```rust
                                                session_aggregate.add_message(
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
```

- [ ] **Step 4: Run to verify it passes**

Run: same command as Step 2.
Expected: PASS. Then the whole feedback suite: replace the filter with `cargo test -p brain-daemon --test uds_feedback_loop_tests 2>&1 | grep "test result"` → **5 passed / 0 failed** (4 baseline + 1 new).

- [ ] **Step 5: Regression check on neighboring suites**

Run:
```bash
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --lib --test uds_permission_roundtrip_tests --test uds_generation_tests --test uds_tool_execution_tests 2>&1 | grep "test result"'
```
Expected: 43/0, 3/3, 3/3, 4/4 — identical to baseline-plus-Task-2 counts.

- [ ] **Step 6: Commit**

```bash
git add daemon/src/transport/uds/handlers.rs daemon/tests/uds_feedback_loop_tests.rs
git commit -m "feat(daemon): persist executed tool outcomes to session transcript

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Denied-path persistence + integration test

**Files:**
- Modify: `daemon/src/transport/uds/handlers.rs` (denied branch, ~:2204-2226)
- Test: `daemon/tests/uds_feedback_loop_tests.rs` (append one test)

**Interfaces:**
- Consumes: same in-scope bindings and helpers as Task 3; `DENIED_FEEDBACK_TEXT` (existing constant used by the denied feedback push).
- Produces: denied outcomes persist as one `role:"tool"` record with `outcome:"denied"` and NO execution fields.

- [ ] **Step 1: Write the failing integration test**

Append at the end of `daemon/tests/uds_feedback_loop_tests.rs`:

```rust
#[tokio::test]
async fn denied_tool_outcome_is_persisted_as_session_tool_message() {
    let proc = start_test_daemon(&[("BRAIN_MOCK_SCRIPTED_RESPONSES", DENY_SCRIPT)]).await;
    let (mut reader, mut writer, session_id) =
        open_and_create_session(&proc.socket_path).await;
    start_generation_with_prompt(&mut writer, &session_id, "refuse and persist").await;
    run_turn_resolving(&mut reader, &proc.socket_path, false).await;

    let messages = load_session_messages(&proc.socket_path, &session_id).await;

    assert_eq!(messages.first().unwrap()["role"], "user");
    assert_eq!(messages.last().unwrap()["role"], "assistant");

    let tools: Vec<&serde_json::Value> =
        messages.iter().filter(|m| m["role"] == "tool").collect();
    assert_eq!(tools.len(), 1, "messages: {messages:?}");
    let env: serde_json::Value =
        serde_json::from_str(tools[0]["content"].as_str().unwrap()).unwrap();
    assert_eq!(env["type"], "tool_event");
    assert_eq!(env["v"], 1);
    assert_eq!(env["name"], "bash");
    assert_eq!(env["input"]["command"], "echo feedback-round-one");
    assert_eq!(env["outcome"], "denied");
    // Nothing ran: no execution fields exist on denied envelopes.
    assert!(env.get("is_error").is_none());
    assert!(env.get("exit_code").is_none());
    assert!(env.get("output").is_none());
    assert!(env.get("duration_ms").is_none());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --test uds_feedback_loop_tests denied_tool_outcome 2>&1 | tail -6'`
Expected: FAIL at `assert_eq!(tools.len(), 1, ...)` — denied turns currently persist nothing.

- [ ] **Step 3: Wire the denied path**

In `handlers.rs`, inside `if !granted { ... }`, immediately AFTER the existing `feedback.push(ToolFeedback { ... DENIED_FEEDBACK_TEXT ... });` block (~:2219-2225), BEFORE the block's closing brace:

```rust
                                                // Inc 8: refusals are part of the
                                                // honest transcript too.
                                                let envelope = serde_json::json!({
                                                    "type": "tool_event",
                                                    "v": 1,
                                                    "call_id": call_id.clone(),
                                                    "name": tool_name.clone(),
                                                    "input": packet["toolUse"]["input"].clone(),
                                                    "outcome": "denied",
                                                });
                                                session_aggregate.add_message(
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
```

- [ ] **Step 4: Run to verify it passes**

Run: same command as Step 2, then the whole suite:
`cargo test -p brain-daemon --test uds_feedback_loop_tests 2>&1 | grep "test result"` → **6 passed / 0 failed** (5 + 1 new).

- [ ] **Step 5: Commit**

```bash
git add daemon/src/transport/uds/handlers.rs daemon/tests/uds_feedback_loop_tests.rs
git commit -m "feat(daemon): persist permission-denied tool attempts to transcript

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: Full gates on the finished increment

**Files:**
- None created. Verification only; commit only if a gate forces a fix.

- [ ] **Step 1: Shell suite**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test 2>&1 | tail -3'`
Expected: 231 pass / 5 fail — the documented baseline. Shell untouched; any NEW failure stops the increment.

- [ ] **Step 2: Rust workspace slice**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-core -p brain-tools -p brain-services -p brain-daemon 2>&1 | grep -E "Running|test result|FAILED"'`
Expected: 0 failures EXCEPT the single documented security-audit test. If cargo halts there first, rerun remaining targets explicitly:
`cargo test -p brain-daemon --test uds_soak_and_operational_tests && cargo test -p brain-tools -p brain-services`.
Expected additions vs pre-increment baselines: daemon lib **43/0**, feedback **6**, domain gains its own 3-test binary; everything else unchanged.

- [ ] **Step 3: Build gate**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun build src/main.tsx --outdir dist --target bun'`
Expected: successful bundle. Never commit `dist/`.

- [ ] **Step 4: Vendor-concept scan (added lines only)**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && git diff b37db67e..HEAD -- crates daemon packages scripts | grep "^+" | grep -icE "anthropic|api\.anthropic|claude"'`
Expected: prints `0`.

- [ ] **Step 5: PTY regression smoke**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && python3 scripts/ptySmokeInc6.py'; echo EXIT=$?`
Expected: 14/14 PASS, EXIT=0 (pure regression — nothing user-visible changed). Restore afterward if fixtures dirty: `git checkout -- packages/brain-shell/src/test/fixtures/pty/inc6/`.

- [ ] **Step 6: Report**

Summarize: commits landed (short hashes + subjects), per-suite counts vs baselines, gates, deviations. Then proceed to the finishing-a-development-branch skill (base branch: main).

---

## Self-Review Record

- **Spec coverage:** §1 decisions → Approach A realized across Tasks 1–4; executed+denied coverage → Tasks 3+4; 4KB bound → Task 2; best-effort writes → Tasks 3(c)/4 Step 3 warn-and-continue; §2 architecture → wiring edits sit exactly at the two outcome sites with unchanged wire frames; §3.1 envelope → byte-for-byte field sets in both tasks; §3.2 truncation → Task 2 signature matches Task 3 call; §3.3 timing → Task 3(a); §3.5 conversation.rs non-change → no task touches it; §4 error rows → warn-on-fail code present at both sites, truncation/multibye tests in Task 2; §5 testing items 1–6 → Tasks 1–5 respectively; §6 non-goals → no task violates them; §7 constraints → Global Constraints.
- **Placeholder scan:** none — every code step carries complete compilable source; anchors cite exact surrounding lines.
- **Type consistency:** `truncate_tool_output(output: &str) -> String` defined Task 2, called identically in Task 3(b); `TOOL_EVENT_OUTPUT_LIMIT_BYTES` referenced only within handlers.rs; `MessageRole::Tool` spelled `brain_domain::MessageRole::Tool` at both wiring sites matching Task 1's export; envelope keys identical between spec §3.1, Tasks 3–4 code, and both integration tests; expected suite counts derive arithmetically from stated baselines (39+4=43 lib; 4+1+1=6 feedback).
