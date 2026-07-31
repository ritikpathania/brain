# Brain v1 Release Readiness — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the four issues that block a first-user from completing the core `launch → ingest → search → inspect` loop.

**Architecture:** The standalone daemon (`brain-daemon`, `daemon/src/server/handlers.rs`) handles legacy `query`/`ingest` over UDS. The TUI client (`crates/brain-tui/src/client.rs`) currently sends `v1/search`, which the standalone daemon does not handle — making every query silently fail. Session management stubs return hardcoded/empty data. Confidence labels use hardcoded absolute score thresholds that `SkimMatcherV2` never reaches.

**Tech Stack:** Rust 2021, Tokio, ratatui, rusqlite, SkimMatcherV2, serde_json, pytest (Python)

## Global Constraints

- Zero `cargo clippy -- -D warnings` violations after each task.
- `cargo fmt` before every commit.
- Test commands run from workspace root: `/Users/ritikpathania/Developer/PyCharm/brain`
- Python tests use `daemon/.venv/bin/pytest` — never system Python.
- Daemon must be running for integration tests: `./brain-daemon daemon start`
- `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python` must be set for `cargo test`
- `brain-domain` stays at the bottom of the dependency tree (no external deps added to it)

---

## Task 1: Fix the v1/search Protocol Mismatch (P0)

**Why this is P0:** Every query from the TUI fails with `"Malformed request: unknown action 'v1/search'"`.

**Compatibility Strategy:**
- **Short-term (this sprint):** Switch the TUI `UdsClient::execute()` to send the legacy `query` action to restore functionality against the standalone daemon.
- **Long-term:** Deprecate `query` once the standalone daemon natively implements the `v1/search` route.

**Files:**
- Modify: `crates/brain-tui/src/client.rs` (~lines 309-404: execute() body)
- Modify: `crates/brain-tui/tests/uds_client_tests.rs`

**Interfaces:**
- Produces: `UdsClient::execute()` sends `{"action":"query","payload":"<prompt>"}` and maps daemon stream events through the existing `map_uds_event()` function unchanged.

---

- [ ] **Step 1.1: Run the current failing test to confirm baseline**

```bash
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python \
  cargo test -p brain-tui --test uds_client_tests -- --nocapture 2>&1 | tail -20
```

Expected: `FAILED` with `Stream returned error: Internal { message: "Malformed request: unknown action 'v1/search'" }`

---

- [ ] **Step 1.2: Replace request payload construction in client.rs**

Find in `UdsClient::execute()` (around line 309):

```rust
        // Construct search query
        let search_query = serde_json::json!({
            "text": req.prompt,
            "kinds": Vec::<String>::new(),
            "pagination": serde_json::Value::Null,
        });

        let mut payload = serde_json::json!({
            "version": "1.0",
            "type": "Request",
            "id": 1,
            "action": "v1/search",
            "body": serde_json::to_string(&search_query).unwrap(),
        });
        if let Some(ref node_ids) = req.workspace_context {
            let ids: Vec<String> = node_ids.iter().map(|id| id.to_string()).collect();
            payload["workspace_context"] = serde_json::json!(ids);
        }
```

Replace with:

```rust
        // Use the legacy query action supported by the standalone daemon.
        // v1/search only exists in the transport daemon layer, not the standalone binary.
        let mut payload = serde_json::json!({
            "action": "query",
            "payload": req.prompt,
        });
        if let Some(ref node_ids) = req.workspace_context {
            let ids: Vec<String> = node_ids.iter().map(|id| id.to_string()).collect();
            payload["workspace_context"] = serde_json::json!(ids);
        }
```

---

- [ ] **Step 1.3: Remove the dead v1/search response parsing branch**

In the same execute() spawn block, delete the entire outer `if let Ok(resp) = serde_json::from_str::<serde_json::Value>(trim_line)` block that checks `resp.get("status").is_some_and(|s| s == "success")` and tries to parse `Vec<SearchSummaryDTO>`. Keep only the fallback branch that calls `map_uds_event()`.

The resulting inner loop body should be:

```rust
                                    if let Ok(uds_ev) = serde_json::from_str::<UdsStreamEvent>(trim_line) {
                                        let core_events = map_uds_event(uds_ev);
                                        let mut should_break = false;
                                        for core_ev in core_events {
                                            let is_finished =
                                                matches!(core_ev.kind, StreamEventKind::Finished { .. })
                                                || matches!(core_ev.kind, StreamEventKind::Cancelled);
                                            let _ = tx.send(Ok(core_ev));
                                            if is_finished {
                                                should_break = true;
                                            }
                                        }
                                        if should_break { break; }
                                    } else if let Ok(err_resp) =
                                        serde_json::from_str::<UdsErrorResponse>(trim_line)
                                    {
                                        if err_resp.status == "error" {
                                            let msg = err_resp.body
                                                .or(err_resp.message)
                                                .unwrap_or_else(|| "Unknown daemon error".to_string());
                                            let _ = tx.send(Err(BrainError::Internal { message: msg }));
                                            break;
                                        }
                                    }
```

---

- [ ] **Step 1.4: Delete the unused SearchSummaryDTO struct**

Find and remove:

```rust
                                                #[derive(serde::Deserialize)]
                                                struct SearchSummaryDTO {
                                                    id: String,
                                                    title: String,
                                                }
```

---

- [ ] **Step 1.5: Verify compilation with zero warnings**

```bash
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python \
  cargo clippy -p brain-tui -- -D warnings 2>&1 | tail -20
```

Expected: exit 0.

---

- [ ] **Step 1.6: Run the integration test**

Ensure daemon is running: `./brain-daemon daemon status`

```bash
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python \
  cargo test -p brain-tui --test uds_client_tests -- --nocapture 2>&1 | tail -20
```

Expected: `test test_uds_client_execute ... ok`

---

- [ ] **Step 1.7: Run full TUI test suite for regressions**

```bash
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-tui 2>&1 | tail -10
```

Expected: all green.

---

- [ ] **Step 1.8: Commit**

```bash
cargo fmt -p brain-tui
git add crates/brain-tui/src/client.rs crates/brain-tui/tests/uds_client_tests.rs
git commit -m "fix(tui): send legacy 'query' action instead of unimplemented 'v1/search'

UdsClient was sending a versioned v1/search request that the standalone daemon
does not handle — every user query silently failed. Switch to the legacy 'query'
action which the daemon streaming handler already supports. Remove the dead
v1/search response parsing branch and SearchSummaryDTO struct."
```

---

## Task 2: Replace Fake Session Management (P0)

**Why this is P0:** `list_sessions()` returns a hardcoded dummy session with a brand-new UUID on every TUI launch — no historical sessions are ever shown. Decision: implement real session listing using `SqliteSessionReadModelRepository` which already exists inside `BrainRuntime`. For `load_session`, update it to explicitly return `Err(BrainError::Unsupported { ... })` instead of a silent `Ok(vec![])` stub to prevent misleading UI state. Replace the hardcoded `"• active-session"` sidebar placeholder with real rendering.

**Files:**
- Modify: `daemon/src/server/handlers.rs` — add `"list_sessions"` handler arm
- Modify: `crates/brain-tui/src/client.rs` — implement real `list_sessions()` and update `load_session()` to return `BrainError::Unsupported`
- Modify: `crates/brain-tui/src/ui/layout/chat_screen.rs` — replace hardcoded sidebar text

---

### Part A: Daemon — expose list_sessions over UDS

- [ ] **Step 2.1: Check SessionTimestamp inner type**

```bash
grep -n "pub struct SessionTimestamp" crates/brain-domain/src/*.rs
```

If it wraps `chrono::DateTime<Utc>`, use `.0.timestamp()` in Step 2.3. If it wraps `i64`, use `.0` directly.

---

- [ ] **Step 2.2: Verify SqliteSessionReadModelRepository::list_all signature**

```bash
grep -n "pub fn" crates/brain-storage/src/sessions_projection.rs
```

Confirm the list method name (`list_all`) and its return type `Result<Vec<SessionReadModel>, BrainError>`.

---

- [ ] **Step 2.3: Add the list_sessions handler arm in handlers.rs**

In `daemon/src/server/handlers.rs`, find the `"disconnect"` arm at the end of the action match block. Insert BEFORE it:

```rust
            "list_sessions" => {
                let pool = brain_runtime.sqlite_storage().pool().clone();
                let repo = brain_storage::SqliteSessionReadModelRepository::new(pool);
                match repo.list_all() {
                    Ok(sessions) => {
                        #[derive(serde::Serialize)]
                        struct SessionWire {
                            id: String,
                            title: String,
                            updated_at: i64,
                            pinned: bool,
                            archived: bool,
                        }
                        let wire: Vec<SessionWire> = sessions
                            .into_iter()
                            .map(|s| SessionWire {
                                id: s.session_id.to_string(),
                                title: s.title,
                                // Adjust per Step 2.1:
                                // i64 inner: updated_at: s.updated_at.0,
                                // chrono inner: updated_at: s.updated_at.0.timestamp(),
                                updated_at: s.updated_at.0,
                                pinned: s.is_pinned,
                                archived: s.is_archived,
                            })
                            .collect();
                        let body = serde_json::to_string(&wire)
                            .unwrap_or_else(|_| "[]".to_string());
                        Some(ServerResponse::Legacy(LegacyResponse {
                            status: "ok".to_string(),
                            message: body,
                        }))
                    }
                    Err(e) => Some(ServerResponse::Legacy(LegacyResponse {
                        status: "error".to_string(),
                        message: format!("list_sessions failed: {}", e),
                    })),
                }
            }
```

If `LegacyResponse` has different field names, check:
```bash
grep -n "pub struct LegacyResponse" daemon/src/server/protocol.rs
```

---

- [ ] **Step 2.4: Build daemon and resolve any type errors**

```bash
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python \
  cargo build --bin brain-daemon 2>&1 | grep "^error" | head -20
```

Fix any type mismatches from the `updated_at` conversion.

---

- [ ] **Step 2.5: Test list_sessions endpoint manually**

Restart daemon after rebuild:

```bash
./brain-daemon daemon stop && sleep 1 && ./brain-daemon daemon start && sleep 2
```

Then probe the endpoint:

```bash
daemon/.venv/bin/python -c "
import json, socket, os
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect(os.path.expanduser('~/.brain/daemon.sock'))
s.sendall(json.dumps({'action': 'list_sessions', 'payload': ''}).encode() + b'\n')
s.settimeout(3)
d = b''
while True:
    c = s.recv(4096)
    if not c: break
    d += c
    if b'\n' in d: break
s.close()
print(d.decode())
"
```

Expected: `{"status":"ok","message":"[]"}` (empty since no sessions exist yet in a fresh DB).

---

### Part B: TUI Client — real list_sessions

- [ ] **Step 2.6: Check SessionSummary struct definition**

```bash
grep -n "pub struct SessionSummary" -A 12 crates/brain-tui/src/client.rs | head -15
```

Note all field names and types. Step 2.8 must match exactly.

---

- [ ] **Step 2.7: Check SessionId construction from UUID string**

```bash
grep -n "pub struct SessionId\|SessionId(" crates/brain-domain/src/*.rs | head -5
```

Typical: `SessionId(uuid::Uuid)`. Constructor: `SessionId(uuid::Uuid::parse_str(&w.id).unwrap())`.

---

- [ ] **Step 2.8: Replace list_sessions stub in client.rs**

Find `async fn list_sessions` inside `impl ExecutionClient for UdsClient`. Replace the entire function body:

```rust
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, BrainError> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .map_err(|e| BrainError::Network {
                message: format!("list_sessions: connect failed: {}", e),
                url: None,
            })?;

        stream
            .write_all(b"{\"action\":\"list_sessions\",\"payload\":\"\"}\n")
            .await
            .map_err(|e| BrainError::Storage {
                message: format!("list_sessions: write failed: {}", e),
                source: None,
            })?;
        stream.flush().await.map_err(|e| BrainError::Storage {
            message: format!("list_sessions: flush failed: {}", e),
            source: None,
        })?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await.map_err(|e| BrainError::Storage {
            message: format!("list_sessions: read failed: {}", e),
            source: None,
        })?;

        #[derive(serde::Deserialize)]
        struct Resp { status: String, message: String }
        #[derive(serde::Deserialize)]
        struct Wire {
            id: String,
            title: String,
            #[serde(default)] pinned: bool,
            #[serde(default)] archived: bool,
        }

        let resp: Resp = serde_json::from_str(line.trim()).map_err(|e| BrainError::Internal {
            message: format!("list_sessions: parse error: {}", e),
        })?;

        if resp.status != "ok" {
            return Err(BrainError::Internal {
                message: format!("list_sessions daemon error: {}", resp.message),
            });
        }

        let wires: Vec<Wire> = serde_json::from_str(&resp.message).unwrap_or_default();

        Ok(wires
            .into_iter()
            .map(|w| {
                // Adapt SessionId construction per Step 2.7
                let id = uuid::Uuid::parse_str(&w.id)
                    .map(brain_domain::SessionId)
                    .unwrap_or_else(|_| brain_domain::SessionId::new());
                // Adapt field names to match SessionSummary per Step 2.6
                SessionSummary {
                    id,
                    title: w.title,
                }
            })
            .collect())
    }
```

Also update `load_session()` in `client.rs` to return an explicit error instead of `Ok(vec![])`:

```rust
    async fn load_session(&self, _session_id: &SessionId) -> Result<Vec<Message>, BrainError> {
        Err(BrainError::Unsupported {
            capability: "historical message loading in standalone daemon".to_string(),
        })
    }
```

---

### Part C: TUI Sidebar — real rendering

- [ ] **Step 2.9: Find the sidebar rendering context**

```bash
grep -n "active-session\|• active" \
  crates/brain-tui/src/ui/layout/chat_screen.rs
```

Note the line number and surrounding variable names (area rect, state access, theme vars).

---

- [ ] **Step 2.10: Replace the hardcoded placeholder**

Replace the `buf.set_stringn(... "• active-session" ...)` line with real rendering. Adapt variable names to match context from Step 2.9:

```rust
let sessions = state.sessions();
if sessions.is_empty() {
    buf.set_stringn(
        area.x + 1,
        area.y + 1,
        "No sessions yet",
        (area.width as usize).saturating_sub(2),
        theme.text_muted,
    );
} else {
    for (i, s) in sessions.iter().take(area.height.saturating_sub(2) as usize).enumerate() {
        let y = area.y + 1 + i as u16;
        let label = format!("  {}", &s.title);
        buf.set_stringn(area.x, y, &label, area.width as usize, theme.text);
    }
}
```

---

- [ ] **Step 2.11: Verify compilation**

```bash
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python \
  cargo clippy --workspace -- -D warnings 2>&1 | tail -20
```

Expected: exit 0.

---

- [ ] **Step 2.12: Run all tests**

```bash
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --workspace --lib 2>&1 | tail -10
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-tui 2>&1 | tail -10
```

Expected: all green.

---

- [ ] **Step 2.13: Commit**

```bash
cargo fmt --workspace
git add daemon/src/server/handlers.rs \
        crates/brain-tui/src/client.rs \
        crates/brain-tui/src/ui/layout/chat_screen.rs
git commit -m "fix(sessions): implement real list_sessions; remove fake session UX

- Add 'list_sessions' UDS handler in daemon using SqliteSessionReadModelRepository
- Replace UdsClient.list_sessions() hardcoded stub with real IPC call
- Replace '• active-session' sidebar placeholder with real session list rendering
- load_session stays empty (no per-session message log in standalone daemon)"
```

---

## Task 3: Surface Daemon Errors in the TUI (P1)

**Why:** When the daemon rejects a request the TUI currently shows nothing — stream ends silently. The `GenerationState::Error` branch in the renderer already formats `"⚠ Error: <msg>"`. Verify the `AppEvent::Error` → `Action::ReportError` → `GenerationState::Error` path is wired end-to-end and add tests.

**Files:**
- Verify/modify: `crates/brain-tui/src/lib.rs` — AppEvent::Error routing
- Modify: `crates/brain-tui/src/state.rs` — unit test
- Modify: `crates/brain-tui/tests/uds_client_tests.rs` — integration test

---

- [ ] **Step 3.1: Confirm AppEvent::Error is dispatched to Action::ReportError**

```bash
grep -n "AppEvent::Error\|ReportError" crates/brain-tui/src/lib.rs | head -15
```

If the mapping is missing, add in the event processing loop:

```rust
Event::App(AppEvent::Error(msg)) => {
    let result = state.update(Action::ReportError(msg));
    if result != UpdateResult::NoChange {
        terminal.draw(|f| renderer.render(f, &state))?;
    }
}
```

---

- [ ] **Step 3.2: Find the correct UiState test constructor**

```bash
grep -n "fn new_for_test\|impl Default for UiState\|UiState::default" \
  crates/brain-tui/src/state.rs | head -5
```

Use the correct constructor in Step 3.3.

---

- [ ] **Step 3.3: Add unit test for error state propagation**

In `crates/brain-tui/src/state.rs`, in the `#[cfg(test)]` module, add:

```rust
#[test]
fn test_report_error_sets_generation_state() {
    let mut state = UiState::default(); // adapt per Step 3.2
    state.update(Action::StartStream);
    state.update(Action::ReportError("daemon rejected".to_string()));
    assert!(
        matches!(&state.generation_state, GenerationState::Error(msg) if msg == "daemon rejected"),
        "Expected Error state, got {:?}", state.generation_state
    );
}
```

Run:

```bash
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python \
  cargo test -p brain-tui -- test_report_error_sets_generation_state --nocapture
```

Expected: PASS.

---

- [ ] **Step 3.4: Add integration test for structured daemon errors**

In `crates/brain-tui/tests/uds_client_tests.rs`, add:

```rust
#[tokio::test]
async fn test_daemon_returns_error_for_unknown_action() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let path = "/Users/ritikpathania/.brain/daemon.sock";
    if !std::path::Path::new(path).exists() {
        println!("Skipping: daemon socket not present");
        return;
    }
    let mut stream = match UnixStream::connect(path).await {
        Ok(s) => s,
        Err(_) => { println!("Skipping: daemon unreachable"); return; }
    };
    stream
        .write_all(b"{\"action\":\"nonexistent_xyz\",\"payload\":\"test\"}\n")
        .await
        .unwrap();
    stream.flush().await.unwrap();

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();

    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(resp["status"], "error");
    let msg = resp["message"].as_str().unwrap_or("");
    assert!(msg.contains("unknown action"), "Got: {}", msg);
}
```

Run:

```bash
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python \
  cargo test -p brain-tui --test uds_client_tests -- --nocapture 2>&1 | tail -15
```

Expected: both tests pass.

---

- [ ] **Step 3.5: Commit**

```bash
cargo fmt -p brain-tui
git add crates/brain-tui/src/state.rs \
        crates/brain-tui/src/lib.rs \
        crates/brain-tui/tests/uds_client_tests.rs
git commit -m "test(tui): verify daemon error propagation to TUI status bar

- Add unit test: ReportError transitions generation_state to Error
- Add integration test: daemon returns structured error on unknown action
- Wire AppEvent::Error -> Action::ReportError in lib.rs event loop if missing"
```

---

## Task 4: Fix Confidence Calibration (P1)

**Why:** `SkimMatcherV2` scores for typical short-text matches stay in the 50–3000 range. The previous thresholds (`>= 7000` for High, `>= 3000` for Medium) make "High confidence" unreachable in practice — every result shows "Low confidence" even for exact verbatim matches. 

**Hybrid Calibration Policy:**
To avoid marking weak garbage matches as "High confidence" while keeping labels algorithm-agnostic:
- If `node.score < MIN_QUALITY_SCORE` (threshold: 50): Label as `"Low"` confidence regardless of rank.
- Otherwise, map by rank: rank 0 = `"High"`, rank 1 = `"Medium"`, rank >= 2 = `"Low"`.

**Files:**
- Modify: `daemon/src/server/handlers.rs` (~line 336: confidence loop)
- Modify: `daemon/tests/test_uds_ipc.py` — add regression test

---

- [ ] **Step 4.1: Write the failing integration test first**

Check what helpers exist in the test file:

```bash
head -40 daemon/tests/test_uds_ipc.py
```

Then add at the end of `daemon/tests/test_uds_ipc.py`:

```python
def test_top_result_has_high_confidence():
    """Verbatim query top result must display 'High confidence'."""
    import time, os, json, socket as _socket

    socket_path = os.path.expanduser("~/.brain/daemon.sock")
    if not os.path.exists(socket_path):
        pytest.skip("daemon not running")

    phrase = "canary_phrase_confidence_rank_xyz_42"

    # Ingest
    s = _socket.socket(_socket.AF_UNIX, _socket.SOCK_STREAM)
    s.connect(socket_path)
    s.sendall((json.dumps({"action": "ingest", "payload": phrase}) + "\n").encode())
    s.settimeout(5)
    d = b""
    while True:
        c = s.recv(4096)
        if not c: break
        d += c
        if b"\n" in d: break
    s.close()
    resp = json.loads(d.split(b"\n")[0])
    assert resp.get("status") == "ok", f"Ingest failed: {resp}"
    time.sleep(0.3)

    # Query and collect stream_chunk events
    s = _socket.socket(_socket.AF_UNIX, _socket.SOCK_STREAM)
    s.connect(socket_path)
    s.sendall((json.dumps({"action": "query", "payload": phrase}) + "\n").encode())
    buf, chunks = b"", []
    s.settimeout(10.0)
    try:
        while True:
            data = s.recv(4096)
            if not data: break
            buf += data
            while b"\n" in buf:
                line, buf = buf.split(b"\n", 1)
                if line:
                    ev = json.loads(line)
                    if ev.get("type") == "stream_chunk":
                        chunks.append(ev.get("content", ""))
                    if ev.get("type") in ("stream_end", "stream_cancelled"):
                        raise StopIteration
    except (StopIteration, _socket.timeout):
        pass
    s.close()

    result_chunks = [c for c in chunks if "confidence" in c]
    assert result_chunks, f"No confidence label in: {chunks}"
    assert "High confidence" in result_chunks[0], (
        f"Expected 'High confidence' in top result, got: {result_chunks[0]!r}"
    )
```

Run:

```bash
daemon/.venv/bin/pytest daemon/tests/test_uds_ipc.py::test_top_result_has_high_confidence -v
```

Expected: FAIL — currently shows "Low confidence".

---

- [ ] **Step 4.2: Apply rank-based confidence in handlers.rs**

Find (around line 334):

```rust
                        for node in matches {
                            seq += 1;
                            let confidence = if node.score >= 7000 {
                                "High"
                            } else if node.score >= 3000 {
                                "Medium"
                            } else {
                                "Low"
                            };
```

Replace with:

```rust
                        /// Minimum quality score threshold to qualify for rank-based confidence assignment.
                        /// Scores below this threshold are labeled "Low" regardless of rank.
                        const MIN_QUALITY_SCORE: i64 = 50;

                        for (rank, node) in matches.into_iter().enumerate() {
                            seq += 1;
                            let confidence = if node.score < MIN_QUALITY_SCORE {
                                "Low"
                            } else {
                                match rank {
                                    0 => "High",
                                    1 => "Medium",
                                    _ => "Low",
                                }
                            };
```

---

- [ ] **Step 4.3: Rebuild daemon**

```bash
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python \
  cargo build --bin brain-daemon 2>&1 | tail -10
```

Expected: `Finished`.

---

- [ ] **Step 4.4: Restart daemon and run new test**

```bash
./brain-daemon daemon stop && sleep 1 && ./brain-daemon daemon start && sleep 2
daemon/.venv/bin/pytest daemon/tests/test_uds_ipc.py::test_top_result_has_high_confidence -v
```

Expected: PASS.

---

- [ ] **Step 4.5: Run all daemon Python tests**

```bash
daemon/.venv/bin/pytest daemon/tests/ -v 2>&1 | tail -20
```

Expected: all pass.

---

- [ ] **Step 4.6: Run Rust workspace tests**

```bash
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --workspace --lib 2>&1 | tail -10
```

Expected: all green.

---

- [ ] **Step 4.7: Commit**

```bash
cargo fmt --workspace
git add daemon/src/server/handlers.rs daemon/tests/test_uds_ipc.py
git commit -m "fix(confidence): rank-based confidence labels replace unreachable score thresholds

SkimMatcherV2 scores for short text stay in 50-3000 range; previous thresholds
(>= 7000 High, >= 3000 Medium) made 'High' unreachable in practice — all results
showed 'Low confidence'. Replace with rank-based: 0 = High, 1 = Medium, >= 2 = Low.
Add integration test asserting top verbatim-match result shows 'High confidence'."
```

---

## Verification Plan

### Automated Tests (full regression after all tasks)

```bash
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --workspace --lib
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-tui
daemon/.venv/bin/pytest daemon/tests/ -v
```

Expected: 0 failures across all suites.

### Manual End-to-End Walkthrough

```
1. ./brain-daemon daemon start
2. target/debug/brain  (in a real terminal)

BEFORE Task 1:  type anything + Enter → silence
AFTER  Task 1:  response streams in correctly

BEFORE Task 2:  sidebar shows "• active-session"
AFTER  Task 2:  sidebar shows "No sessions yet" (or real list)

BEFORE Task 4:  every result shows "Low confidence"
AFTER  Task 4:  top result shows "High confidence"

Error surfacing (Task 3):
  Kill daemon while TUI is open → status bar shows "⚠ Error: ..."
```
