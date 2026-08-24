# Brain Shell Increment 4 — Permission Wire Round-Trip Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Inc 3 permission dialog real end-to-end: the daemon emits `tool_permission_requested` during generation and pauses until the shell resolves it over UDS via a new `v1/tool/resolve` action.

**Architecture:** A global permission-waiter registry in the daemon's UDS transport keyed by call ID. The stream handler forwards each `tool_use` frame, emits `tool_permission_requested`, and awaits a oneshot signal (bounded by a timeout that defaults to deny). Because the stream occupies its connection's read loop, resolution arrives on a **separate connection** — which is exactly how the shell already works (`callRpc` opens a fresh short-lived connection per request). On the shell side, `SessionController.resolvePermission` keeps its local UX unchanged and additionally fires the wire call best-effort through a new optional `BrainBackendClient` member.

**Tech Stack:** Rust daemon (`daemon/src/transport/uds/handlers.rs`, tokio oneshot/RwLock), Bun + TypeScript shell (`packages/brain-shell`), Python PTY harness.

**Spec:** `docs/superpowers/specs/2026-08-23-brain-shell-contracts-first-design.md` (§3 "Permission prompts (allow/deny/always)" row; §5 Increment 3 deferred the wire round-trip; this increment closes that deferral).

## Global Constraints

Verbatim from the binding directive:

- Study `/Users/ritikpathania/Developer/claude-code` as **primary UI/interaction reference only**. Observable UX contracts are extracted and re-implemented originally; nothing from that tree is vendored, committed, or redistributed.
- "Preserve Brain's existing architecture, domain model, IPC contracts, runtime, memory, retrieval, graph, provenance, agents, and adapter boundaries."
- "Do not introduce Claude/Anthropic models, APIs, authentication, pricing, billing, or LLM-specific product concepts."
- Stack stays Bun 1.4 + React 19 + stock Ink 7 via `src/compat/index.js` + yoga-layout. No framework changes.
- Small increments, each independently verifiable. TDD: failing test first, then minimal implementation, then green, then commit.
- Every commit contains only explicitly-added paths (`git add <paths>`, never `git add .`). Commit trailer: `Co-Authored-By: Claude <noreply@anthropic.com>`.
- Vendor audit: `git diff <base>..HEAD -- packages/brain-shell/src/ | grep '^+' | grep -icE 'claude|anthropic|vendor'` must be **0** over shipped source (guard assertions inside `src/test/**` negative-asserting those strings are enforcement code and are reported separately).
- Zero NEW test failures vs. the documented baseline (214 pass / 5 fail: visualCellParity ×2, sessionSemanticIntegration, brainMemoryIntegration, brainTurnTransformer).
- No file under `packages/brain-shell/src/adapter/` is modified in this increment.
- Git discipline: ALL git commands run as `git -C /Users/ritikpathania/Developer/PyCharm/brain <cmd>`; bun/cargo invocations wrapped in `bash -c '<cmd>'`. Never rely on `cd` prefixes.

## Wire contract established by this increment

Frames are newline-delimited JSON on the UDS byte stream (unchanged framing).

**Daemon → client (during `v1/generation/stream`, additive):**

```json
{"type":"tool_permission_requested","generation_id":"…","session_id":"…","sequence":N,
 "call_id":"call_mock_1","tool_name":"bash","input":{"command":"ls"},
 "reason":"tool execution requires approval","status":"in_progress"}
```
Emitted immediately after the matching `tool_use` frame, consuming the next sequence number (strict client-side gap detection requires consecutiveness).

```json
{"type":"tool_denied","generation_id":"…","session_id":"…","sequence":M,
 "call_id":"call_mock_1","tool_name":"bash","status":"in_progress"}
```
Emitted only when resolution is deny or timeout, before streaming continues.

**Client → daemon (new action, any connection):**

Request (the shell's `callRpc` shape):
```json
{"id":"req_…","action":"v1/tool/resolve","payload":{"call_id":"call_mock_1","granted":true}}
```

Reply on success: `{"type":"resolved","status":"ok"}`
Reply on unknown/already-resolved call: `{"type":"Error","status":"error","body":"Unknown or already-resolved tool call 'X'"}`

Legacy alias `tool/resolve` accepted alongside `v1/tool/resolve`.

**Determinism hook for tests:** `DeterministicMockProvider` gains a prompt sentinel — a substring `[brain-tool:NAME]` or `[brain-tool:NAME|{json}]` in the last user message makes the mock emit exactly one `ToolUse` chunk (`call_mock_<n>` IDs) and finish with reason `tool_use`. Prompts without the sentinel behave exactly as before.

## Design decisions recorded

- **Resolution rides a second connection, not the stream connection.** The daemon processes UDS frames sequentially per connection and holds the writer across the whole stream branch; same-connection mid-stream requests would deadlock. The shell's stateless per-RPC connections make the second-connection approach zero-cost. Documented in both sides' comments.
- **Timeout = deny.** A client that never answers must not hold the session-busy lock forever. Default 300 s, overridable via `BRAIN_TOOL_PERMISSION_TIMEOUT_SECS`.
- **Optional interface member.** `resolveToolPermission?` is optional on `BrainBackendClient` because dozens of salvage-era test doubles cast partial objects to the interface; the real client implements it fully and `MockBrainBackendClient` records resolutions for assertions. The controller calls it defensively.
- **Scope deferral:** the spec's third option "**always** allow" persistence is NOT built here — every tool call asks. Recording that explicitly keeps this increment small; a later increment can hang policy storage off the resolve action.
- **No executor yet.** Grant currently means "the tool_use frame stands approved and streaming continues"; there is still no daemon-side tool runner (tools are not even sent to providers today: `gen_request.tools` is empty). This increment makes the *permission decision* authoritative on the wire; execution wiring is future work and out of scope.

---

### Task 1: Mock provider sentinel trigger

**Files:**
- Modify: `crates/brain-services/src/model/mock.rs`
- Test: appended `#[cfg(test)] mod sentinel_tests` in the same file

**Interfaces:**
- Consumes: existing `ScriptedResponse { thinking, tokens, tool_calls, error, finish_reason }`, `GenerationRequest`, `ModelChatMessage::text(ChatRole::User, &str)`.
- Produces: prompts containing `[brain-tool:NAME]` / `[brain-tool:NAME|{json}]` yield one `GenerationChunk::ToolUse { id: "call_mock_<n>", name, input }` and `Completed { finish_reason: "tool_use", .. }`. Tasks 2–3 drive this from integration tests.

- [ ] **Step 1: Write the failing test**

Append to the bottom of `crates/brain-services/src/model/mock.rs`:

```rust
#[cfg(test)]
mod sentinel_tests {
    use super::*;
    use brain_core::model::{ChatRole, ModelChatMessage};
    use futures::StreamExt;

    async fn collect(provider: &DeterministicMockProvider, prompt: &str) -> Vec<GenerationChunk> {
        let request = GenerationRequest {
            model: "brain-default".to_string(),
            messages: vec![ModelChatMessage::text(ChatRole::User, prompt)],
            system_prompt: None,
            tools: Vec::new(),
            thinking_budget: None,
        };
        let stream = provider
            .stream_generation(request, CancellationToken::new())
            .await
            .unwrap();
        stream.map(|c| c.unwrap()).collect::<Vec<_>>().await
    }

    #[tokio::test]
    async fn sentinel_prompt_emits_single_tool_call_with_parsed_input() {
        let provider = DeterministicMockProvider::new();
        let chunks = collect(
            &provider,
            "please run [brain-tool:bash|{\"command\":\"ls build\"}] now",
        )
        .await;
        let mut ids = Vec::new();
        let mut names = Vec::new();
        let mut inputs = Vec::new();
        for c in &chunks {
            if let GenerationChunk::ToolUse { id, name, input } = c {
                ids.push(id.clone());
                names.push(name.clone());
                inputs.push(input.clone());
            }
        }
        assert_eq!(names, vec!["bash".to_string()]);
        assert_eq!(inputs, vec![serde_json::json!({"command": "ls build"})]);
        assert!(ids[0].starts_with("call_mock_"));
        match chunks.last().unwrap() {
            GenerationChunk::Completed { finish_reason, .. } => {
                assert_eq!(finish_reason, "tool_use");
            }
            other => panic!("expected Completed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn plain_prompt_emits_no_tool_calls() {
        let provider = DeterministicMockProvider::new();
        let chunks = collect(&provider, "just say hi").await;
        assert!(
            chunks
                .iter()
                .all(|c| !matches!(c, GenerationChunk::ToolUse { .. }))
        );
    }

    #[tokio::test]
    async fn bare_sentinel_without_json_yields_empty_input() {
        let provider = DeterministicMockProvider::new();
        let chunks = collect(&provider, "use [brain-tool:search] please").await;
        let found = chunks.iter().any(|c| matches!(
            c,
            GenerationChunk::ToolUse { name, input, .. }
                if name == "search" && *input == serde_json::json!({})
        ));
        assert!(found);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && cargo test -p brain-services sentinel --lib'`
Expected: FAIL — no `[brain-tool:` handling exists, so the sentinel prompt produces zero ToolUse chunks (`names == ["bash"]` assertion fails).

- [ ] **Step 3: Write minimal implementation**

Three edits to `crates/brain-services/src/model/mock.rs`:

(a) Add the counter field to the struct and both constructors:

```rust
/// Thread-safe deterministic mock model provider.
#[derive(Debug, Clone)]
pub struct DeterministicMockProvider {
    supported_models: Vec<ModelDescriptor>,
    scripted_queue: Arc<Mutex<VecDeque<ScriptedResponse>>>,
    default_response: Arc<Mutex<Option<ScriptedResponse>>>,
    /// Monotonic source for `[brain-tool:]` sentinel call IDs.
    sentinel_counter: Arc<std::sync::atomic::AtomicUsize>,
}
```

In `new()` and `with_models()`, add to the struct literal:

```rust
            sentinel_counter: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
```

(b) Add this free function above `impl ModelProvider for DeterministicMockProvider`:

```rust
/// Extracts a deterministic tool call from a `[brain-tool:NAME]` or
/// `[brain-tool:NAME|{json}]` sentinel embedded in the last user prompt.
fn sentinel_tool_call(
    prompt: &str,
    counter: &std::sync::atomic::AtomicUsize,
) -> Option<(String, String, serde_json::Value)> {
    let start = prompt.find("[brain-tool:")?;
    let rest = &prompt[start + "[brain-tool:".len()..];
    let end = rest.find(']')?;
    let spec = &rest[..end];
    let (name, input) = match spec.split_once('|') {
        Some((n, j)) => (
            n.trim().to_string(),
            serde_json::from_str::<serde_json::Value>(j.trim())
                .unwrap_or(serde_json::json!({})),
        ),
        None => (spec.trim().to_string(), serde_json::json!({})),
    };
    if name.is_empty() {
        return None;
    }
    let n = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    Some((format!("call_mock_{}", n), name, input))
}
```

(c) Restructure the response selection in `stream_generation`. Replace the block from `let scripted = { … };` through the end of the `unwrap_or_else` closure (currently lines ~166–196) with:

```rust
        let last_user_prompt = request
            .messages
            .iter()
            .rev()
            .find(|m| m.role == brain_core::model::ChatRole::User)
            .and_then(|m| {
                m.content.iter().find_map(|c| match c {
                    brain_core::model::MessageContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                })
            })
            .unwrap_or_else(|| "Hello".to_string());

        let scripted = {
            let mut queue = self.scripted_queue.lock();
            queue
                .pop_front()
                .or_else(|| self.default_response.lock().clone())
        };

        // Prompt sentinel takes effect only when nothing was explicitly scripted.
        let scripted = match scripted {
            Some(s) => Some(s),
            None => sentinel_tool_call(&last_user_prompt, &self.sentinel_counter).map(
                |(id, name, input)| ScriptedResponse {
                    thinking: None,
                    tokens: vec![format!("Invoking tool {}.", name)],
                    tool_calls: vec![(id, name, input)],
                    error: None,
                    finish_reason: Some("tool_use".to_string()),
                },
            ),
        };

        let response = scripted.unwrap_or_else(|| ScriptedResponse {
            thinking: Some(
                "Analyzing user request in deterministic mock engine...".to_string(),
            ),
            tokens: vec![format!("Mock response to: {}", last_user_prompt)],
            tool_calls: Vec::new(),
            error: None,
            finish_reason: Some("end_turn".to_string()),
        });
```

(This preserves the previous default response verbatim; only its extraction of `last_user_prompt` moved above the selection so the sentinel path can share it.)

- [ ] **Step 4: Run test to verify it passes**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && cargo test -p brain-services sentinel --lib'`
Expected: PASS (3 tests). If `#[tokio::test]` fails to compile due to missing macros feature in this crate, fall back to wrapping bodies in `futures::executor::block_on(async { … })` — do not add features to Cargo.toml for this.

- [ ] **Step 5: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add crates/brain-services/src/model/mock.rs
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "feat(daemon): deterministic [brain-tool:] sentinel triggers tool calls in mock provider

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Permission waiter registry + `v1/tool/resolve` action

**Files:**
- Modify: `daemon/src/transport/uds/handlers.rs` (registry near line 25–47; resolve branch between the cancel branch ending ~line 1566 and the stream branch starting line 1568)
- Test: `daemon/tests/uds_permission_roundtrip_tests.rs` (create)

**Interfaces:**
- Consumes: existing `if action == "…" { … continue; }` dispatch chain style; `writer.lock().await` reply pattern.
- Produces: `get_permission_waiters() -> &'static Arc<tokio::sync::RwLock<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>`; wire action `v1/tool/resolve` replying `{"type":"resolved","status":"ok"}` or error. Task 3 consumes both.

- [ ] **Step 1: Write the failing test**

Create `daemon/tests/uds_permission_roundtrip_tests.rs` with the daemon harness (copied verbatim from `daemon/tests/uds_generation_tests.rs`) plus the unknown-call test:

```rust
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

struct DaemonProcess {
    child: Child,
    test_dir: PathBuf,
    socket_path: PathBuf,
}

impl Drop for DaemonProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&test_dir_value());
    }
}

// NOTE: keep the Drop simple; the harness below mirrors uds_generation_tests.rs
fn test_dir_value() -> PathBuf {
    PathBuf::from("/tmp") // placeholder replaced in Step 3 refinement
}
```

Do **not** keep that placeholder — instead use this exact, complete harness (this is the file content for Step 1; Step 3 only appends more tests):

```rust
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

struct DaemonProcess {
    child: Child,
    test_dir: PathBuf,
    socket_path: PathBuf,
}

impl Drop for DaemonProcess {
    fn drop(&mut self) {
        let dir = self.test_dir.clone();
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(dir);
    }
}

fn get_temp_dir() -> PathBuf {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    let path = PathBuf::from(format!("/tmp/bd-perm-{}", &uuid_str[0..8]));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn get_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn start_test_daemon() -> DaemonProcess {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let test_dir = get_temp_dir();
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");

    let child = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .env("BRAIN_MOCK_CHUNK_DELAY_MS", "50")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start daemon process");

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

async fn send_frame<T>(writer: &mut tokio::net::tcp::OwnedWriteHalf, frame: &T) where T: serde::Serialize {
    let mut json = serde_json::to_string(frame).unwrap();
    json.push('\n');
    writer.write_all(json.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();
}

async fn read_line_frame(reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>) -> serde_json::Value {
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    serde_json::from_str(line.trim()).unwrap()
}

#[tokio::test]
async fn resolve_unknown_call_is_rejected_as_error() {
    let daemon = start_test_daemon().await;
    let stream = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    send_frame(
        &mut writer,
        &serde_json::json!({
            "id": "req-resolve-bogus",
            "action": "v1/tool/resolve",
            "payload": { "call_id": "no_such_call", "granted": true }
        }),
    )
    .await;

    let reply = read_line_frame(&mut buf_reader).await;
    assert_eq!(reply["status"], "error");
    let body = reply["body"].as_str().unwrap_or_default();
    assert!(
        body.contains("no_such_call"),
        "error should echo the unknown call id, got: {}",
        body
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && cargo test --test uds_permission_roundtrip_tests'`
Expected: FAIL — the daemon replies `Unknown action 'v1/tool/resolve'` (an Error frame whose `body`/`message` differs) or closes; the status/error-content assertions fail.

- [ ] **Step 3: Implement the registry and the resolve branch**

(a) In `daemon/src/transport/uds/handlers.rs`, directly below the `FEEDBACK_REGISTRY` static (~line 32), add:

```rust
static PERMISSION_WAITERS: std::sync::OnceLock<
    Arc<tokio::sync::RwLock<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
> = std::sync::OnceLock::new();

/// Pending tool-permission decisions keyed by tool-use call ID. The stream
/// task parks a oneshot sender here; any connection may deliver the verdict
/// via v1/tool/resolve (resolution intentionally rides a second connection —
/// the stream occupies its own connection's read loop).
fn get_permission_waiters()
-> &'static Arc<tokio::sync::RwLock<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>
{
    PERMISSION_WAITERS.get_or_init(|| Arc::new(tokio::sync::RwLock::new(HashMap::new())))
}
```

(b) Between the closing brace of the `v1/generation/cancel` branch (`continue;` + `}`, immediately before `if action == "v1/generation/stream"` at line 1568) insert:

```rust
        if action == "v1/tool/resolve" || action == "tool/resolve" {
            #[derive(serde::Deserialize)]
            struct ResolvePayload {
                #[serde(rename = "callId", alias = "call_id", default)]
                call_id: Option<String>,
                #[serde(default)]
                granted: bool,
            }

            let resolve_req: ResolvePayload =
                serde_json::from_str(payload).unwrap_or(ResolvePayload {
                    call_id: None,
                    granted: false,
                });

            let outcome = match resolve_req.call_id.clone() {
                Some(call_id) => {
                    let waiter = get_permission_waiters().write().await.remove(&call_id);
                    match waiter {
                        Some(tx) => tx.send(resolve_req.granted).is_ok(),
                        None => false,
                    }
                }
                None => false,
            };

            let response = if outcome {
                serde_json::json!({ "type": "resolved", "status": "ok" })
            } else {
                serde_json::json!({
                    "type": "Error",
                    "status": "error",
                    "body": format!(
                        "Unknown or already-resolved tool call '{}'",
                        resolve_req.call_id.unwrap_or_default()
                    )
                })
            };
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }
```

Note: the shell's `callRpc` treats `status === 'ok'` as success and `status === 'error'` as rejection — these reply shapes are chosen to satisfy that parser.

- [ ] **Step 4: Run test to verify it passes**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && cargo test --test uds_permission_roundtrip_tests'`
Expected: PASS (1 test). Also run `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && cargo test -p brain-services --lib sentinel'` to confirm Task 1 still green.

- [ ] **Step 5: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add daemon/src/transport/uds/handlers.rs daemon/tests/uds_permission_roundtrip_tests.rs
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "feat(daemon): v1/tool/resolve action over a permission-waiter registry

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Stream permission gate (emit → pause → resume/deny)

**Files:**
- Modify: `daemon/src/transport/uds/handlers.rs` (ToolUse arm, lines ~1932–1949)
- Test: `daemon/tests/uds_permission_roundtrip_tests.rs` (append)

**Interfaces:**
- Consumes: `get_permission_waiters()` from Task 2; sentinel trigger from Task 1; frame shapes from the Wire contract section.
- Produces: live daemon behavior — `tool_use` frame followed by `tool_permission_requested`, stream paused until resolution; deny/timeout emits `tool_denied`; grant continues normally. Sequences stay strictly consecutive.

- [ ] **Step 1: Write the failing tests**

Append to `daemon/tests/uds_permission_roundtrip_tests.rs`:

```rust
/// Opens a connection, creates a session, consumes the create reply, and
/// returns (reader, writer, session_id).
/// The session-create reply is consumed internally.
async fn open_and_create_session(socket_path: &std::path::Path)
    -> (BufReader<tokio::net::tcp::OwnedReadHalf>, tokio::net::tcp::OwnedWriteHalf, String)
{
    let stream = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    send_frame(
        &mut writer,
        &serde_json::json!({
            "version": "1.0",
            "type": "Request",
            "id": 1,
            "action": "v1/session/create",
            "body": serde_json::json!({ "title": "perm roundtrip" }).to_string()
        }),
    )
    .await;
    let reply = read_line_frame(&mut buf_reader).await;
    let body: serde_json::Value = if let Some(s) = reply["body"].as_str() {
        serde_json::from_str(s).unwrap()
    } else {
        reply["body"].clone()
    };
    let (reader, _) = buf_reader.into_inner().into_split();
    (
        BufReader::new(reader),
        writer,
        body["session_id"].as_str().unwrap().to_string(),
    )
}

async fn start_generation_with_sentinel(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    session_id: &str,
) {
    send_frame(
        writer,
        &serde_json::json!({
            "id": "req-gen-perm",
            "action": "v1/generation/stream",
            "payload": {
                "sessionId": session_id,
                "generationId": uuid::Uuid::new_v4().to_string(),
                "messages": [
                    { "role": "user", "content": "run [brain-tool:bash|{\"command\":\"ls build\"}] please" }
                ],
                "model": "brain-default"
            }
        }),
    )
    .await;
}

#[tokio::test]
async fn stream_pauses_on_permission_request_and_grant_resumes_without_denial() {
    let daemon = start_test_daemon().await;
    let (mut reader_a, mut writer_a, session_id) =
        open_and_create_session(&daemon.socket_path).await;

    start_generation_with_sentinel(&mut writer_a, &session_id).await;

    // stream_start(0), tool_use(1), tool_permission_requested(2)
    let mut types = Vec::new();
    let mut call_id = String::new();
    for expected_seq in 0u64..3 {
        let frame = read_line_frame(&mut reader_a).await;
        assert_eq!(frame["sequence"].as_u64().unwrap(), expected_seq, "gap in early frames");
        types.push(frame["type"].as_str().unwrap().to_string());
        if frame["type"] == "tool_permission_requested" {
            call_id = frame["call_id"].as_str().unwrap().to_string();
            assert_eq!(frame["tool_name"], "bash");
        }
    }
    assert_eq!(types, vec!["stream_start", "tool_use", "tool_permission_requested"]);

    // Stream must be PAUSED: no further frames within 700 ms.
    let mut probe = String::new();
    let paused = tokio::time::timeout(
        Duration::from_millis(700),
        reader_a.read_line(&mut probe),
    )
    .await;
    assert!(paused.is_err(), "stream continued while permission unresolved");

    // Resolve on a SECOND connection.
    let resolver = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (rreader, mut rwriter) = resolver.into_split();
    let mut rbuf = BufReader::new(rreader);
    send_frame(
        &mut rwriter,
        &serde_json::json!({
            "id": "req-resolve-ok",
            "action": "v1/tool/resolve",
            "payload": { "call_id": call_id, "granted": true }
        }),
    )
    .await;
    let reply = read_line_frame(&mut rbuf).await;
    assert_eq!(reply["status"], "ok");

    // Stream resumes: token frames then finished/completed, NO tool_denied.
    let mut saw_denied = false;
    loop {
        let frame = read_line_frame(&mut reader_a).await;
        match frame["type"].as_str().unwrap() {
            "tool_denied" => saw_denied = true,
            "finished" => {
                assert_eq!(frame["status"], "completed");
                break;
            }
            _ => {}
        }
    }
    assert!(!saw_denied, "grant must not produce tool_denied");
}

#[tokio::test]
async fn denial_emits_tool_denied_then_completes() {
    let daemon = start_test_daemon().await;
    let (mut reader_a, mut writer_a, session_id) =
        open_and_create_session(&daemon.socket_path).await;

    start_generation_with_sentinel(&mut writer_a, &session_id).await;

    let mut call_id = String::new();
    for expected_seq in 0u64..3 {
        let frame = read_line_frame(&mut reader_a).await;
        assert_eq!(frame["sequence"].as_u64().unwrap(), expected_seq);
        if frame["type"] == "tool_permission_requested" {
            call_id = frame["call_id"].as_str().unwrap().to_string();
        }
    }

    let resolver = UnixStream::connect(&daemon.socket_path).await.unwrap();
    let (rreader, mut rwriter) = resolver.into_split();
    let mut rbuf = BufReader::new(rreader);
    send_frame(
        &mut rwriter,
        &serde_json::json!({
            "id": "req-resolve-no",
            "action": "v1/tool/resolve",
            "payload": { "call_id": call_id, "granted": false }
        }),
    )
    .await;
    assert_eq!(read_line_frame(&mut rbuf).await["status"], "ok");

    // Expect tool_denied carrying the call id, then finished completed.
    let mut denied_seen = false;
    loop {
        let frame = read_line_frame(&mut reader_a).await;
        match frame["type"].as_str().unwrap() {
            "tool_denied" => {
                denied_seen = true;
                assert_eq!(frame["call_id"].as_str().unwrap(), call_id);
            }
            "finished" => {
                assert_eq!(frame["status"], "completed");
                break;
            }
            _ => {}
        }
    }
    assert!(denied_seen, "deny must emit tool_denied");
}
```

Final file inventory for Step 1: the harness (`DaemonProcess`, `get_temp_dir`, `get_free_port`, `start_test_daemon`), `send_frame`, `read_line_frame`, `resolve_unknown_call_is_rejected_as_error` (from Task 2), plus `open_and_create_session`, `start_generation_with_sentinel`, and the two new tests. Nothing else.

- [ ] **Step 2: Run tests to verify they fail**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && cargo test --test uds_permission_roundtrip_tests'`
Expected: the two new tests FAIL — today the stream emits no `tool_permission_requested`, so the 0..3 type assertion sees `["stream_start","token","finished"]` (or similar) and panics; the pause assertion never runs.

- [ ] **Step 3: Implement the gate in the ToolUse arm**

Replace the entire `GenerationChunk::ToolUse` arm (lines ~1932–1949) with:

```rust
                                        brain_core::model::GenerationChunk::ToolUse { id, name, input } => {
                                            let call_id = id.clone();
                                            let tool_name = name.clone();

                                            // Forward the tool call itself first.
                                            let packet = serde_json::json!({
                                                "type": "tool_use",
                                                "generation_id": generation_id,
                                                "session_id": session_id_str,
                                                "sequence": seq,
                                                "toolUse": {
                                                    "id": id,
                                                    "name": name,
                                                    "input": input
                                                },
                                                "status": "in_progress"
                                            });
                                            let mut j = serde_json::to_string(&packet)?;
                                            j.push('\n');
                                            writer.write_all(j.as_bytes()).await?;
                                            writer.flush().await?;

                                            // Permission gate: publish the request, then park
                                            // the stream until v1/tool/resolve delivers a verdict
                                            // (on ANY connection) or the timeout denies by default.
                                            seq += 1;
                                            let perm_packet = serde_json::json!({
                                                "type": "tool_permission_requested",
                                                "generation_id": generation_id,
                                                "session_id": session_id_str,
                                                "sequence": seq,
                                                "call_id": call_id,
                                                "tool_name": tool_name,
                                                "input": packet["toolUse"]["input"],
                                                "reason": "tool execution requires approval",
                                                "status": "in_progress"
                                            });
                                            let mut pj = serde_json::to_string(&perm_packet)?;
                                            pj.push('\n');
                                            writer.write_all(pj.as_bytes()).await?;
                                            writer.flush().await?;

                                            let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
                                            get_permission_waiters()
                                                .write()
                                                .await
                                                .insert(call_id.clone(), tx);
                                            let timeout_secs = std::env::var(
                                                "BRAIN_TOOL_PERMISSION_TIMEOUT_SECS",
                                            )
                                            .ok()
                                            .and_then(|v| v.parse::<u64>().ok())
                                            .unwrap_or(300);
                                            let granted = tokio::time::timeout(
                                                std::time::Duration::from_secs(timeout_secs),
                                                rx,
                                            )
                                            .await
                                            .ok()
                                            .and_then(|r| r.ok())
                                            .unwrap_or(false);
                                            get_permission_waiters()
                                                .write()
                                                .await
                                                .remove(&call_id);

                                            if !granted {
                                                seq += 1;
                                                let denied_packet = serde_json::json!({
                                                    "type": "tool_denied",
                                                    "generation_id": generation_id,
                                                    "session_id": session_id_str,
                                                    "sequence": seq,
                                                    "call_id": call_id,
                                                    "tool_name": tool_name,
                                                    "status": "in_progress"
                                                });
                                                let mut dj = serde_json::to_string(&denied_packet)?;
                                                dj.push('\n');
                                                writer.write_all(dj.as_bytes()).await?;
                                                writer.flush().await?;
                                            }
                                        }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && cargo test --test uds_permission_roundtrip_tests'`
Expected: PASS (3 tests). Then the broader daemon suite must stay green:
`bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && cargo test --test uds_generation_tests'`
Expected: PASS — non-sentinel generations never hit the gate (sequences unchanged).

- [ ] **Step 5: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add daemon/src/transport/uds/handlers.rs daemon/tests/uds_permission_roundtrip_tests.rs
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "feat(daemon): pause generation streams on tool_permission_requested until resolved

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Shell client — optional contract member, mock recording, UDS sender

**Files:**
- Modify: `packages/brain-shell/src/client/BrainBackendClient.ts` (interface at lines 410–432; `MockBrainBackendClient` class body starting line 437)
- Modify: `packages/brain-shell/src/client/UdsBrainBackendClient.ts` (add method near the other RPC wrappers)
- Test: `packages/brain-shell/src/test/client/resolvePermissionWire.test.ts` (create)

**Interfaces:**
- Consumes: `callRpc<T>(action, payload)` (private, existing).
- Produces: `BrainBackendClient.resolveToolPermission?(callId: string, granted: boolean): Promise<void>`; `MockBrainBackendClient.permissionResolutions: Array<{ callId: string; granted: boolean }>`; concrete `UdsBrainBackendClient.resolveToolPermission`. Task 5 consumes the first two; the PTY smoke exercises the third against a stub.

- [ ] **Step 1: Write the failing test**

Create `packages/brain-shell/src/test/client/resolvePermissionWire.test.ts`:

```ts
import { describe, expect, test, afterAll } from 'bun:test';
import * as net from 'node:net';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { MockBrainBackendClient } from '../../client/BrainBackendClient.js';
import { UdsBrainBackendClient as LiveUdsClient } from '../../client/UdsBrainBackendClient.js';

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-resolve-wire-'));
const sockPath = path.join(dir, 't.sock');

type Frame = { action?: string; payload?: Record<string, unknown> };
const received: Frame[] = [];
const server = net.createServer((socket) => {
  let buffer = '';
  socket.on('data', (data) => {
    buffer += data.toString('utf8');
    let idx: number;
    while ((idx = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, idx);
      buffer = buffer.slice(idx + 1);
      if (!line.trim()) continue;
      const frame = JSON.parse(line) as Frame;
      received.push(frame);
      const unknownCall =
        frame.action === 'v1/tool/resolve' && frame.payload?.['call_id'] === 'bogus';
      socket.write(
        JSON.stringify(
          unknownCall
            ? { type: 'Error', status: 'error', body: "Unknown or already-resolved tool call 'bogus'" }
            : { type: 'resolved', status: 'ok' },
        ) + '\n',
      );
    }
  });
});
server.listen(sockPath);

afterAll(() => {
  server.close();
  fs.rmSync(dir, { recursive: true, force: true });
});

describe('wire resolution of pending permissions', () => {
  test('mock client records resolutions for assertions', async () => {
    const mock = new MockBrainBackendClient(['ok']);
    await mock.resolveToolPermission!('call_9', true);
    expect(mock.permissionResolutions).toEqual([{ callId: 'call_9', granted: true }]);
  });

  test('live client sends v1/tool/resolve with snake_case payload over UDS', async () => {
    const client = new LiveUdsClient(sockPath);
    await client.resolveToolPermission('call_9', true);
    const frame = received.find((f) => f.action === 'v1/tool/resolve');
    expect(frame).toBeDefined();
    expect(frame!.payload).toMatchObject({ call_id: 'call_9', granted: true });
  });

  test('live client surfaces unknown-call errors as rejections', async () => {
    const client = new LiveUdsClient(sockPath);
    expect(client.resolveToolPermission('bogus', false)).rejects.toThrow(/Unknown or already-resolved/);
    await new Promise((r) => setTimeout(r, 50));
  });
});
```

(`MockBrainBackendClient` is defined in `BrainBackendClient.ts` at line ~437; the live client only in `UdsBrainBackendClient.ts` — hence the aliased import.)

- [ ] **Step 2: Run test to verify it fails**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/client/resolvePermissionWire.test.ts'`
Expected: FAIL — TypeScript strips the optional member at runtime, so `mock.resolveToolPermission!` throws "not a function".

- [ ] **Step 3: Implement**

(a) In `BrainBackendClient.ts`, inside `interface BrainBackendClient extends BrainBackend` (after the `listSessions()` declaration, before the closing brace at line 432):

```ts
  /**
   * Best-effort wire resolution of a pending tool-permission request
   * (v1/tool/resolve). Optional: legacy fakes may omit it; the controller
   * degrades gracefully to local-only UX when absent.
   */
  resolveToolPermission?(callId: string, granted: boolean): Promise<void>;
```

(b) In class `MockBrainBackendClient`, immediately after the constructor (after line ~446):

```ts
  /** Recorded v1/tool/resolve invocations, for controller-level assertions. */
  readonly permissionResolutions: Array<{ callId: string; granted: boolean }> = [];

  async resolveToolPermission(callId: string, granted: boolean): Promise<void> {
    this.permissionResolutions.push({ callId, granted });
  }
```

(c) In `UdsBrainBackendClient.ts`, next to the other RPC wrapper methods (e.g., right above `async retrieveContext`):

```ts
  /**
   * Resolves a pending tool-permission request on its own short-lived UDS
   * connection — the stream occupies the stream connection's read loop, so
   * verdicts intentionally ride a separate connection.
   */
  async resolveToolPermission(callId: string, granted: boolean): Promise<void> {
    await this.callRpc<void>('v1/tool/resolve', {
      call_id: callId,
      granted,
    });
  }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/client/resolvePermissionWire.test.ts'`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add packages/brain-shell/src/client/BrainBackendClient.ts packages/brain-shell/src/client/UdsBrainBackendClient.ts packages/brain-shell/src/test/client/resolvePermissionWire.test.ts
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "feat(brain-shell): resolve tool permissions over the wire via v1/tool/resolve

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: Controller forwards resolutions best-effort

**Files:**
- Modify: `packages/brain-shell/src/state/sessionController.ts` (method `resolvePermission`, lines 77–91)
- Test: modify `packages/brain-shell/src/test/state/sessionControllerPermission.test.ts` (append)

**Interfaces:**
- Consumes: optional `client.resolveToolPermission` from Task 4.
- Produces: user-visible behavior identical to Inc 3 plus a fire-and-forget wire call; failure of the wire call never disturbs local UX.

- [ ] **Step 1: Write the failing tests**

Append inside the existing `describe('SessionController permission requests', …)` block, before its closing brace:

```ts
  test('resolution travels to backends that support the wire call', async () => {
    const base = scriptFake(PERM_SCRIPT) as Record<string, unknown>;
    const resolutions: Array<{ callId: string; granted: boolean }> = [];
    base.resolveToolPermission = (callId: string, granted: boolean) => {
      resolutions.push({ callId, granted });
      return Promise.resolve();
    };
    const ctl = new SessionController(base as unknown as BrainBackendClient);
    await ctl.submit('clean this up');
    ctl.resolvePermission('call_9', true);
    expect(resolutions).toEqual([{ callId: 'call_9', granted: true }]);
    expect(JSON.stringify(ctl.getSnapshot().rows)).toContain('Allowed bash');
  });

  test('backends without wire support degrade to local-only UX', async () => {
    const ctl = new SessionController(scriptFake(PERM_SCRIPT));
    await ctl.submit('clean this up');
    expect(() => ctl.resolvePermission('call_9', true)).not.toThrow();
    expect(JSON.stringify(ctl.getSnapshot().rows)).toContain('Allowed bash');
  });

  test('wire rejection never disturbs the local notice', async () => {
    const base = scriptFake(PERM_SCRIPT) as Record<string, unknown>;
    base.resolveToolPermission = () => Promise.reject(new Error('socket gone'));
    const ctl = new SessionController(base as unknown as BrainBackendClient);
    await ctl.submit('clean this up');
    ctl.resolvePermission('call_9', true);
    await new Promise((r) => setTimeout(r, 10)); // flush the rejected promise
    expect(JSON.stringify(ctl.getSnapshot().rows)).toContain('Allowed bash');
  });
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/state/sessionControllerPermission.test.ts'`
Expected: FAIL — the first new test's `resolutions` stays empty because the controller never calls the client.

- [ ] **Step 3: Implement**

Replace the body of `resolvePermission` in `sessionController.ts` (keeping the signature and doc position):

```ts
  resolvePermission(callId: string, granted: boolean): void {
    if (this.pendingPermission?.callId !== callId) return;
    const toolName = this.pendingPermission.toolName;
    this.pendingPermission = undefined;
    if (!granted) {
      this.rows = this.rows.map((r) =>
        r.kind === 'tool' && r.tool.callId === callId
          ? { ...r, tool: { ...r.tool, status: 'denied' as const } }
          : r,
      );
    }
    // Best-effort wire round-trip: legacy fakes and offline daemons simply
    // omit or reject the call; the local UX above is already settled either way.
    void this.client.resolveToolPermission?.(callId, granted)?.catch(() => {});
    this.notice(`${granted ? 'Allowed' : 'Denied'} ${toolName}`);
  }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/state/sessionControllerPermission.test.ts'`
Expected: PASS (8 tests — 5 existing + 3 new). Then the full suite:
`bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test 2>&1 | tail -4'`
Expected: **217 pass / 5 fail** (baseline + 3), zero new failures.

- [ ] **Step 5: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add packages/brain-shell/src/state/sessionController.ts packages/brain-shell/src/test/state/sessionControllerPermission.test.ts
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "feat(brain-shell): forward permission resolutions to the daemon best-effort

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: PTY smoke — prove the round trip live

**Files:**
- Create: `scripts/ptySmokeInc4.py`
- Create fixtures under: `packages/brain-shell/src/test/fixtures/pty/inc4/` (generated by the run)

**Interfaces:**
- Consumes: everything shipped in Tasks 1–5; the Inc 3 smoke discipline (stub daemon, TIOCSWINSZ before exec, discrete keystroke writes ≥0.3 s apart, ANSI-stripped matching).
- Produces: exit-0 proof that pressing `y`/`n` in the dialog sends `v1/tool/resolve` on the wire (asserted from the request log, not just UI text) and that deny renders a denied card.

- [ ] **Step 1: Write the smoke script**

Create `scripts/ptySmokeInc4.py`, modeled on `scripts/ptySmokeInc3.py`, with these differences (full deltas, everything else copied verbatim):

Header constants:

```python
SOCK = "/tmp/brain-inc4-smoke.sock"
FRAMES_FILE = "/tmp/brain-inc4-smoke-requests.jsonl"
FIXTURE_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/src/test/fixtures/pty/inc4"
```

Stub daemon additions — a module-level waiter map next to the other globals:

```python
PERM_EVENTS = {}    # call_id -> threading.Event
PERM_GRANTED = {}   # call_id -> bool
```

Inside the per-connection handler, replace the single `elif act == "v1/generation/stream":` body with a version that pauses on permission and honors cross-connection resolution (keep the existing session/create, session/list, v1/session/load branches untouched):

```python
                    elif act == "v1/generation/stream":
                        # Turn: tool_use -> permission request -> PAUSE until a
                        # v1/tool/resolve arrives on ANOTHER connection (mirrors
                        # the real daemon, whose stream occupies its read loop).
                        reply({"type": "tool_use", "toolUse": {"id": "call_9",
                               "name": "bash", "input": {"command": "ls build"}},
                               "sequence": 0})
                        time.sleep(0.2)
                        reply({"type": "tool_permission_requested", "call_id": "call_9",
                               "tool_name": "bash", "input": {"command": "ls build"},
                               "reason": "shell access", "sequence": 1})
                        evt = threading.Event()
                        PERM_EVENTS["call_9"] = evt
                        resolved = evt.wait(timeout=10)
                        granted = bool(resolved and PERM_GRANTED.get("call_9"))
                        if granted:
                            reply({"type": "token", "token": "Approved.",
                                   "sequence": 2})
                        else:
                            reply({"type": "tool_denied", "call_id": "call_9",
                                   "tool_name": "bash", "sequence": 2})
                        time.sleep(0.3)
                        reply({"type": "finished", "status": "completed", "sequence": 3})
                    elif act == "v1/tool/resolve":
                        payload = req.get("payload", {})
                        cid = payload.get("call_id")
                        PERM_GRANTED[cid] = bool(payload.get("granted"))
                        ev = PERM_EVENTS.get(cid)
                        if ev is not None:
                            ev.set()
                            reply({"type": "resolved", "status": "ok"})
                        else:
                            reply({"type": "Error", "status": "error",
                                   "body": "Unknown or already-resolved tool call"})
```

Flow section (replaces Inc 3's Flows B/C; Flow A welcome checks kept verbatim from Inc 3):

```python
# ── Flow D1: ALLOW completes the wire round-trip ───────────────────────────
def frames_log():
    try:
        with open(FRAMES_FILE) as f:
            return f.read()
    except Exception:
        return ""

os.write(fd, b"list the build folder")
pump(0.3)
os.write(fd, b"\r")
ok &= expect("tool-card", "bash")
ok &= expect("dialog-header", "Permission required")
snapshot("permissionAllow-pending")
os.write(fd, b"y")                       # allow

# Round-trip criterion: the RESOLUTION reached the wire, not just the UI.
deadline = time.time() + 6
wire_allow = False
while time.time() < deadline:
    pump(0.1)
    if '"v1/tool/resolve"' in frames_log():
        wire_allow = True
        break
print(("PASS" if wire_allow else "FAIL") + " resolve-on-wire")
ok &= wire_allow
ok &= expect("allowed-notice", "Allowed bash")
ok &= expect("approved-token", "Approved.")
snapshot("permissionAllow-done")

# ── Flow D2: DENY marks the card and reports granted=false ─────────────────
os.write(fd, b"now delete it")
pump(0.3)
os.write(fd, b"\r")
ok &= expect("dialog-header-2", "Permission required")
snapshot("permissionDeny-pending")
os.write(fd, b"n")                       # deny

deadline = time.time() + 6
wire_deny = False
while time.time() < deadline:
    pump(0.1)
    if '"granted": false' in frames_log() or '"granted":false' in frames_log():
        wire_deny = True
        break
print(("PASS" if wire_deny else "FAIL") + " deny-on-wire")
ok &= wire_deny
ok &= expect("denied-notice", "Denied bash")
snapshot("permissionDeny-done")
```

Teardown identical to Inc 3 (ctrl+c, SIGKILL, `sys.exit(0 if ok else 1)`).

- [ ] **Step 2: Run the smoke and iterate to green**

Run: `python3 scripts/ptySmokeInc4.py` (Bash tool timeout parameter ≥ 120000 ms).
Expected: all expects PASS including `resolve-on-wire` and `deny-on-wire`; exit 0; four fixture files written under `src/test/fixtures/pty/inc4/`.
Known pitfalls carried from Inc 3: missing imports surface only at runtime (`BUILD_OK` cannot catch JSX/identifier misses — grep your new view usage); one stdin chunk = one keypress; multi-char text may be sent as a single paste chunk.

- [ ] **Step 3: Full gates**

Run, in order, fixing anything NEW that breaks:

```bash
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test 2>&1 | tail -4'
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && cargo test --test uds_permission_roundtrip_tests --test uds_generation_tests'
bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun run build >/dev/null 2>&1 && echo BUILD_OK'
git -C /Users/ritikpathania/Developer/PyCharm/brain diff main..HEAD -- packages/brain-shell/src/ | grep '^+' | grep -icE 'claude|anthropic|vendor'
git -C /Users/ritikpathania/Developer/PyCharm/brain diff main..HEAD -- packages/brain-shell/src/ ':!packages/brain-shell/src/test' | grep '^+' | grep -icE 'claude|anthropic|vendor'
```

Expected: suite **217 pass / 5 baseline fail**; Rust 3+existing PASS; BUILD_OK; vendor scan 0 on source-only variant (guard-test hits documented if present).

- [ ] **Step 4: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add scripts/ptySmokeInc4.py packages/brain-shell/src/test/fixtures/pty/inc4/
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "test(brain-shell): Inc 4 PTY smoke proves permission resolution on the wire

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Self-review record

- **Spec coverage:** §3 permission row — allow/deny now round-trips (Tasks 2–5), dialog UI existed since Inc 3. "Always" explicitly deferred (Design decisions). §7 error handling — timeout-denies, unknown-call errors surfaced, tolerant reception retained. No adapter/ files touched (Constraint 4 preserved).
- **Placeholder scan:** the Task 3 Step 1 draft initially contained a dead helper sketch; instructions say to delete it — final file contents enumerated explicitly. No TBDs remain.
- **Type consistency:** `resolveToolPermission?(callId: string, granted: boolean): Promise<void>` used identically in interface (Task 4a), controller call site (Task 5), and tests. Rust wire keys `call_id`/`granted` consistent between handler branch (Task 2), client payload (Task 4c), smoke stub (Task 6). Sequence arithmetic checked against the strict gap detector: stream_start 0 → tool_use 1 → permission 2 → (denied | token) 3 → stream_end 4 → finished 5, all consecutive.
- **Risk noted:** the shell's generator stops at `stream_end` (before the daemon's trailing `finished` frame) — pre-existing behavior from Inc 1, unaffected by this increment; the smoke asserts UI-visible outcomes only.
