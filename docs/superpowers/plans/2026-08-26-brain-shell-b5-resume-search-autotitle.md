# B5 / Inc 20 — Resume Search & Auto-Titles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `/resume` usable past a handful of sessions: daemon auto-titles sessions from their first user prompt; the picker gains type-away fuzzy search and a current-session marker.

**Architecture:** A pure domain rule (`Session::autotitle`) backfills default titles at the three existing user-message persist sites in the daemon's UDS handler — no schema or RPC change. On the shell, two new keybinding-table rows (a `printable` pseudo-key and `overlay:backspace`) route typing into a pure query reducer; `resumeChoices` gains an optional query parameter that switches it from recency-ordering to score-ranked fuzzy filtering. The view adds a query line, a `●` marker on the live session, and a no-matches state.

**Tech Stack:** Rust (brain-domain, brain-daemon) · Bun + React 19 + Ink 7 (packages/brain-shell) · Python PTY harness

**Spec:** `docs/superpowers/specs/2026-08-26-brain-shell-b5-resume-search-autotitle-design.md`

## Global Constraints

- Preserve Brain's architecture, domain model, IPC contracts, runtime, memory, retrieval, graph, provenance, agents, adapter boundaries.
- No IPC contract, storage-schema, or `session/list` response changes.
- No Claude/Anthropic-derived concepts anywhere.
- Stack: Bun + React 19 + Ink 7 + yoga-layout + Rust daemon.
- Working tree carries ~3.7k dirty user-WIP paths: NEVER `git stash`, never `git add .`/`git add -A`, never revert unstaged changes. Every commit stages ONLY the paths listed in its task (`git add <explicit paths>`), and touched files may carry pre-existing WIP hunks — after staging, run `git diff --cached` and confirm ONLY this task's hunks are staged; if foreign hunks ride along, unstage and hunk-split instead of committing them.
- Every commit message ends with trailer: `Co-Authored-By: Claude <noreply@anthropic.com>`
- EVERY cargo invocation uses the macOS wrapper:
  ```bash
  bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo <subcommand>'
  ```
- Sole permitted workspace test failure: `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers`. Known parallel-load flake family (rerun standalone to prove): `uds_feedback_loop_tests::shell_exec_runs_command_and_persists_standalone_turn`, SIGKILL crash-recovery timing.
- Daemon package name is `brain-daemon` (`cargo test -p brain-daemon ...`).
- Shell suite absolute counts drift; gate by failure-identity match against the documented five.
- Run long gates with UNFILTERED log capture (no grep pipelines on the cargo command itself).

---

### Task 1: Domain rule — `Session::autotitle`

**Files:**
- Create: `crates/brain-domain/tests/session_autotitle_tests.rs`
- Modify: `crates/brain-domain/src/entities.rs` (inside `impl Session`, which starts at line 815)

**Interfaces:**
- Consumes: existing `Session { title: SessionTitle, messages: Vec<Message>, .. }`, `MessageRole::User`, `MessageRole::Assistant`, `SessionTitle::default()` == `"New Session"` (`identifiers.rs:67`), `Message::new(MessageId, MessageRole, String)`.
- Produces: `pub fn autotitle(&mut self)` on `Session`; private free fn `fn derive_session_title(text: &str) -> Option<String>` in `entities.rs`. Later tasks call only `autotitle()`.

- [ ] **Step 1: Write the failing tests**

Create `crates/brain-domain/tests/session_autotitle_tests.rs`:

```rust
//! B5: one-time default-title backfill from the first user message.

use brain_domain::{Message, MessageId, MessageRole, Session, SessionId, SessionTimestamp, SessionTitle};

fn fresh_session() -> Session {
    Session::new(SessionId::new(), SessionTitle::default(), SessionTimestamp(0))
}

fn push(session: &mut Session, role: MessageRole, content: &str) {
    session
        .messages
        .push(Message::new(MessageId::new(), role, content.to_string()));
}

#[test]
fn derives_from_first_user_message_when_default() {
    let mut s = fresh_session();
    push(&mut s, MessageRole::User, "Help me debug the login flow");
    s.autotitle();
    assert_eq!(s.title, SessionTitle("Help me debug the login flow".to_string()));
}

#[test]
fn leaves_non_default_titles_untouched() {
    let mut s = Session::new(
        SessionId::new(),
        SessionTitle("Custom".to_string()),
        SessionTimestamp(0),
    );
    push(&mut s, MessageRole::User, "Help me debug the login flow");
    s.autotitle();
    assert_eq!(s.title, SessionTitle("Custom".to_string()));
}

#[test]
fn keeps_default_without_user_messages() {
    let mut s = fresh_session();
    push(&mut s, MessageRole::Assistant, "An answer");
    s.autotitle();
    assert_eq!(s.title, SessionTitle::default());
}

#[test]
fn derives_from_user_even_after_assistant_messages() {
    let mut s = fresh_session();
    push(&mut s, MessageRole::Assistant, "welcome");
    push(&mut s, MessageRole::User, "the real prompt");
    s.autotitle();
    assert_eq!(s.title, SessionTitle("the real prompt".to_string()));
}

#[test]
fn multiline_takes_first_nonempty_line_collapsed() {
    let mut s = fresh_session();
    push(&mut s, MessageRole::User, "\n   \nFix the   login\tbug\nsecond line");
    s.autotitle();
    assert_eq!(s.title, SessionTitle("Fix the login bug".to_string()));
}

#[test]
fn long_line_capped_at_43_with_ellipsis() {
    let mut s = fresh_session();
    let fifty = "abcdefghij".repeat(5); // 50 chars, single word
    push(&mut s, MessageRole::User, &fifty);
    s.autotitle();
    let t = s.title.0;
    assert_eq!(t.chars().count(), 44); // 43 + ellipsis
    assert!(t.ends_with('…'));
    assert!(t.starts_with("abcdefghijklm"));
}

#[test]
fn exactly_43_chars_stays_untruncated() {
    let mut s = fresh_session();
    let forty_three = "a".repeat(43);
    push(&mut s, MessageRole::User, &forty_three);
    s.autotitle();
    assert_eq!(s.title, SessionTitle(forty_three));
}

#[test]
fn bang_command_is_a_valid_source() {
    let mut s = fresh_session();
    push(&mut s, MessageRole::User, "! cargo test --workspace");
    s.autotitle();
    assert_eq!(s.title, SessionTitle("! cargo test --workspace".to_string()));
}

#[test]
fn whitespace_only_prompt_keeps_default() {
    let mut s = fresh_session();
    push(&mut s, MessageRole::User, "   \n\t  ");
    s.autotitle();
    assert_eq!(s.title, SessionTitle::default());
}

#[test]
fn second_call_after_derivation_is_noop() {
    let mut s = fresh_session();
    push(&mut s, MessageRole::User, "first prompt");
    s.autotitle();
    push(&mut s, MessageRole::User, "second prompt");
    s.autotitle();
    assert_eq!(s.title, SessionTitle("first prompt".to_string()));
}
```

- [ ] **Step 2: Run to verify failure**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-domain --test session_autotitle_tests'
```
Expected: COMPILE ERROR — `no method named 'autotitle'` (tests reference it before implementation).

- [ ] **Step 3: Implement**

In `crates/brain-domain/src/entities.rs`, add inside `impl Session` (after `reconstruct`'s block or any existing method — order within the impl is free):

```rust
    /// One-time default-title backfill (B5): if the title is still the
    /// default and any user message exists, rename the session from that
    /// message's first line. Idempotent — after one derivation the default
    /// check fails forever, so renamed sessions are never touched again.
    pub fn autotitle(&mut self) {
        if self.title != SessionTitle::default() {
            return;
        }
        let source = self
            .messages
            .iter()
            .find(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone());
        if let Some(derived) = source.as_deref().and_then(derive_session_title) {
            self.title = SessionTitle(derived);
        }
    }
```

And as a private free function at the end of `entities.rs` (outside any impl):

```rust
/// B5: derive a session title from user text — first non-empty line,
/// internal whitespace collapsed to single spaces, capped at 43 chars
/// plus an ellipsis (46-column budget shared with the shell's picker row).
fn derive_session_title(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    let mut collapsed = String::with_capacity(line.len());
    let mut prev_space = false;
    for ch in line.chars() {
        let is_space = ch.is_whitespace();
        if is_space {
            if !prev_space && !collapsed.is_empty() {
                collapsed.push(' ');
            }
        } else {
            collapsed.push(ch);
        }
        prev_space = is_space;
    }
    let trimmed = collapsed.trim_end();
    if trimmed.is_empty() {
        return None;
    }
    const CAP: usize = 43;
    if trimmed.chars().count() <= CAP {
        Some(trimmed.to_string())
    } else {
        let head: String = trimmed.chars().take(CAP).collect();
        Some(format!("{head}…"))
    }
}
```

Note: `entities.rs` is inside crate `brain_domain`, so refer to types unqualified (`MessageRole::User`, `SessionTitle`). Check the top of the file for how `SessionTitle` is brought into scope (it lives in `identifiers.rs`; the file likely already has `use crate::identifiers::*;` or similar — follow the existing pattern; if neither `MessageRole` nor `SessionTitle` is imported, add `MessageRole` / `SessionTitle` to that existing use list rather than adding new `use` lines).

- [ ] **Step 4: Run to verify pass**

```bash
bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-domain --test session_autotitle_tests'
```
Expected: `10 passed; 0 failed`.

- [ ] **Step 5: Verify no regressions in the domain crate**

```bash
bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-domain'
```
Expected: all green (existing suites untouched).

- [ ] **Step 6: Commit**

```bash
git diff --cached   # must be empty before staging
git add crates/brain-domain/src/entities.rs crates/brain-domain/tests/session_autotitle_tests.rs
git diff --cached   # confirm ONLY autotitle hunks; entities.rs may carry WIP — see Global Constraints
git commit -m "feat(domain): Session::autotitle backfills default titles

Pure aggregate rule (B5): when the title is still SessionTitle::default()
and a user message exists, derive the title from its first non-empty
line — whitespace-collapsed, capped at 43 chars plus an ellipsis to fit
the resume picker's 46-column row budget. Idempotent; renamed sessions
are never rewritten.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Wire the three persist sites + UDS integration proof

**Files:**
- Modify: `daemon/src/transport/uds/handlers.rs` (three one-line insertions)
- Create: `daemon/tests/uds_session_autotitle_tests.rs`

**Interfaces:**
- Consumes: `Session::autotitle()` (Task 1); existing daemon test-daemon spawn pattern (`daemon/tests/uds_load_stress_tests.rs` `DaemonProcess`/restart helpers are the model).
- Produces: end-to-end behavior — untitled session streamed a turn persists a derived title readable via `v1/session/load`.

- [ ] **Step 1: Write the failing integration test**

Create `daemon/tests/uds_session_autotitle_tests.rs`. This mirrors the established daemon e2e scaffolding (spawn real binary on temp socket/db, speak newline-delimited JSON over UDS):

```rust
//! B5: auto-title backfill end-to-end over UDS, including daemon restart.

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

struct DaemonProcess {
    child: Child,
    test_dir: PathBuf,
    socket_path: PathBuf,
    db_path: PathBuf,
    analytics_db_path: PathBuf,
    pid_path: PathBuf,
    port: u16,
}

impl Drop for DaemonProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.test_dir);
    }
}

fn get_temp_dir() -> PathBuf {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    let path = PathBuf::from(format!("/tmp/bd-autotitle-{}", &uuid_str[0..8]));
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

async fn spawn_daemon(test_dir: &PathBuf, socket: &PathBuf, pid: &PathBuf, db: &PathBuf, analytics: &PathBuf, port: u16) -> Child {
    Command::new(env!("CARGO_BIN_EXE_brain-daemon"))
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", socket)
        .env("BRAIN_PID_PATH", pid)
        .env("BRAIN_DB_PATH", db)
        .env("BRAIN_ANALYTICS_DB_PATH", analytics)
        .env("BRAIN_CONFIG_DIR", test_dir)
        .env("BRAIN_HEALTH_PORT", port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start daemon process")
}

```

Assemble directly inside each test like the sibling suites do — ONE constructor used by both tests:

```rust
async fn start_daemon_at(test_dir: PathBuf) -> DaemonProcess {
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");
    let port = get_free_port();
    let child = spawn_daemon(&test_dir, &socket_path, &pid_path, &db_path, &analytics_db_path, port).await;
    let mut ready = false;
    for _ in 0..60 {
        if socket_path.exists() && UnixStream::connect(&socket_path).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(ready, "Daemon did not bind socket in time");
    DaemonProcess {
        child,
        test_dir,
        socket_path,
        db_path,
        analytics_db_path,
        pid_path,
        port,
    }
}
```

Shared RPC helpers (`rpc` mirrors the sibling suites' string-or-object `body` handling; the generation envelope is a RAW frame — not wrapped in the versioned Request envelope — exactly as in `uds_generation_tests.rs:113`):

```rust
async fn rpc(socket_path: &PathBuf, id: u64, action: &str, body: serde_json::Value) -> serde_json::Value {
    let stream = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let req = serde_json::json!({
        "version": "1.0",
        "type": "Request",
        "id": id,
        "action": action,
        "body": serde_json::to_string(&body).unwrap()
    });
    let mut j = serde_json::to_string(&req).unwrap();
    j.push('\n');
    writer.write_all(j.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();
    let mut line = String::new();
    buf_reader.read_line(&mut line).await.unwrap();
    let frame: serde_json::Value = serde_json::from_str(&line).unwrap();
    if let Some(s) = frame["body"].as_str() {
        serde_json::from_str(s).unwrap()
    } else {
        frame["body"].clone()
    }
}

async fn stream_one_turn(socket_path: &PathBuf, session_id: &str, prompt: &str) {
    let stream = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let gen_req = serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "action": "v1/generation/stream",
        "payload": {
            "sessionId": session_id,
            "generationId": uuid::Uuid::new_v4().to_string(),
            "messages": [{ "role": "user", "content": prompt }],
            "model": "brain-default"
        }
    });
    let mut j = serde_json::to_string(&gen_req).unwrap();
    j.push('\n');
    writer.write_all(j.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();
    loop {
        let mut line = String::new();
        if buf_reader.read_line(&mut line).await.unwrap() == 0 {
            break;
        }
        if let Ok(frame) = serde_json::from_str::<serde_json::Value>(line.trim()) {
            if frame["type"] == "finished" || frame["type"] == "error" {
                break;
            }
        }
    }
}
```

The two tests:

```rust
#[tokio::test]
async fn test_autotitle_derives_from_first_turn_and_persists() {
    let daemon = start_daemon_at(get_temp_dir()).await;

    // Untitled session via v1/session/create (title omitted -> "New Session").
    let body = rpc(&daemon.socket_path, 1, "v1/session/create", serde_json::json!({})).await;
    let sid = body["session_id"].as_str().unwrap().to_string();

    stream_one_turn(&daemon.socket_path, &sid, "Help me debug the login flow").await;

    let loaded = rpc(
        &daemon.socket_path,
        2,
        "v1/session/load",
        serde_json::json!({ "session_id": sid }),
    )
    .await;
    assert_eq!(
        loaded["session"]["title"], "Help me debug the login flow",
        "untitled session must adopt its first prompt as title"
    );
}

#[tokio::test]
async fn test_autotitled_title_survives_daemon_restart() {
    let mut daemon = start_daemon_at(get_temp_dir()).await;
    let body = rpc(&daemon.socket_path, 1, "v1/session/create", serde_json::json!({})).await;
    let sid = body["session_id"].as_str().unwrap().to_string();

    stream_one_turn(&daemon.socket_path, &sid, "Refactor the ingest pipeline").await;

    // Hard-restart on the same DB (pattern from uds_load_stress_tests.rs).
    let _ = daemon.child.kill();
    let _ = daemon.child.wait();
    let port = get_free_port();
    daemon.port = port;
    daemon.child = spawn_daemon(
        &daemon.test_dir,
        &daemon.socket_path,
        &daemon.pid_path,
        &daemon.db_path,
        &daemon.analytics_db_path,
        port,
    )
    .await;
    let mut ready = false;
    for _ in 0..60 {
        if UnixStream::connect(&daemon.socket_path).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(ready, "Restarted daemon did not bind socket in time");

    let loaded = rpc(
        &daemon.socket_path,
        2,
        "v1/session/load",
        serde_json::json!({ "session_id": sid }),
    )
    .await;
    assert_eq!(loaded["session"]["title"], "Refactor the ingest pipeline");

    // A titled session is never retitled by later turns.
    stream_one_turn(&daemon.socket_path, &sid, "Second unrelated topic").await;
    let reloaded = rpc(
        &daemon.socket_path,
        3,
        "v1/session/load",
        serde_json::json!({ "session_id": sid }),
    )
    .await;
    assert_eq!(reloaded["session"]["title"], "Refactor the ingest pipeline");
}
```

NOTE before running: check whether the mock generation engine requires an env var in this repo's tests (grep `uds_generation_tests.rs` for `BRAIN_MOCK` env setup — the sibling suites set none, so none is needed).

- [ ] **Step 2: Run to verify failure**

```bash
bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --test uds_session_autotitle_tests'
```
Expected: FAIL — `loaded["session"]["title"]` equals `"New Session"`, not the prompt (rule exists but no call site fires yet).

- [ ] **Step 3: Insert the three call sites**

In `daemon/src/transport/uds/handlers.rs`, immediately BEFORE each of these existing saves, add `autotitle()` on the aggregate variable in scope:

Site 1 — bang-command persist (~line 840):
```rust
            session_aggregate.autotitle();
            let _ = storage.save_session(&parsed_session_id, &session_aggregate);
```

Site 2 — append-turn save (~line 1131; local variable is `session` here):
```rust
                    session.autotitle();
                    let _ = storage.save_session(&parsed_sid, &session);
```

Site 3 — generation-stream user persist ("Invariant 4", ~line 2039):
```rust
            session_aggregate.autotitle();
            let _ = storage.save_session(&parsed_session_id, &session_aggregate);
```

Each insertion is ONE line plus nothing else. Do not reorder or touch surrounding lines.

- [ ] **Step 4: Run to verify pass**

```bash
bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --test uds_session_autotitle_tests'
```
Expected: `2 passed; 0 failed`.

- [ ] **Step 5: Regression-check the daemon suites that count messages**

The change adds no messages, but run the nearest suites to be sure:

```bash
bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --test uds_generation_tests --test uds_generation_adversarial_tests'
```
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git diff --cached   # must be empty before staging
git add daemon/src/transport/uds/handlers.rs daemon/tests/uds_session_autotitle_tests.rs
git diff --cached   # handlers.rs carries WIP — verify ONLY the three autotitle lines staged (see Global Constraints; if foreign hunks appear: git restore --staged daemon/src/transport/uds/handlers.rs, then stage the file, then git reset the foreign hunks via a filtered patch: git diff --cached > /tmp/x.patch, edit out foreign hunks, git reset, git apply --cached /tmp/x.patch)
git commit -m "feat(daemon): auto-title sessions at the three user-persist sites

Calls Session::autotitle() before each existing best-effort save_session
that follows a persisted user message: the generation-stream Invariant 4
persist, the bang-command twin, and session/append_turn. Adds the
end-to-end UDS proof: an untitled session adopts its first prompt as its
persisted title, which survives a hard daemon restart, and titled
sessions are never rewritten by later turns.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Keybinding resolver — `printable` pseudo-key + overlay backspace

**Files:**
- Modify: `packages/brain-shell/src/keybindings/resolve.ts`
- Test: `packages/brain-shell/src/test/keybindings/resolve.test.ts`

**Interfaces:**
- Consumes: existing `DEFAULT_BINDINGS`, `strokeToKey(input, key)` (printables already fall through to the literal char, `resolve.ts:54`), `resolveAction(bindings, contexts, keyId)`.
- Produces: actions `'overlay:insert'` and `'overlay:backspace'` resolvable in context `'overlay'`; AppShell (Task 5) consumes them. `resolveAction` signature unchanged.

- [ ] **Step 1: Write the failing tests**

Extend `packages/brain-shell/src/test/keybindings/resolve.test.ts` (append a new describe; reuse whatever import style the file already has):

```ts
describe('overlay printable capture (B5)', () => {
  const bindings = DEFAULT_BINDINGS;

  test('single plain character resolves to overlay:insert in overlay context', () => {
    expect(resolveAction(bindings, ['overlay'], 'h')).toBe('overlay:insert');
    expect(resolveAction(bindings, ['overlay'], ' ')).toBe('overlay:insert');
    expect(resolveAction(bindings, ['overlay'], 'A')).toBe('overlay:insert');
  });

  test('backspace resolves to overlay:backspace in overlay context', () => {
    expect(resolveAction(bindings, ['overlay'], 'backspace')).toBe('overlay:backspace');
  });

  test('exact bindings win over printable', () => {
    expect(resolveAction(bindings, ['overlay'], 'return')).toBe('overlay:commit');
    expect(resolveAction(bindings, ['overlay'], 'escape')).toBe('overlay:cancel');
    expect(resolveAction(bindings, ['overlay'], 'up')).toBe('overlay:up');
  });

  test('contexts without the printable row are unchanged', () => {
    expect(resolveAction(bindings, ['dialog'], 'h')).toBeNull(); // dialog binds y/a/n only
    expect(resolveAction(bindings, ['palette'], 'h')).toBeNull();
    expect(resolveAction(bindings, ['composer'], 'h')).toBeNull();
  });

  test('multi-char and control strokes never match printable', () => {
    expect(resolveAction(bindings, ['overlay'], 'ctrl+a')).toBeNull();
    expect(resolveAction(bindings, ['overlay'], '')).toBeNull();
  });
});
```

Check first how existing tests construct bindings — if they use `DEFAULT_BINDINGS` imported from `../../../keybindings/resolve.js`, match that. If the file builds custom tables inline, mirror its fixtures instead so the new rows come from the real `DEFAULT_BINDINGS`.

- [ ] **Step 2: Run to verify failure**

```bash
cd packages/brain-shell && bun test ./src/test/keybindings/resolve.test.ts
```
Expected: FAIL — `overlay:insert` cases get null (rows/probe don't exist yet).

- [ ] **Step 3: Implement**

In `resolve.ts`:

Append to `DEFAULT_BINDINGS` (after the dialog rows; comment updated):

```ts
  // B5 resume search: overlays opt into plain-character capture via the
  // 'printable' pseudo-key; exact bindings always win over it.
  { action: 'overlay:insert', context: 'overlay', key: 'printable' },
  { action: 'overlay:backspace', context: 'overlay', key: 'backspace' },
```

Replace `resolveAction` with:

```ts
export function resolveAction(
  bindings: readonly BindingRule[],
  contexts: readonly KeybindingContextName[],
  keyId: string,
): string | null {
  if (keyId.length === 0) return null;
  const order: KeybindingContextName[] = [...contexts, 'global'];
  for (const ctx of order) {
    const hit = bindings.find((b) => b.context === ctx && b.key === keyId);
    if (hit !== undefined) return hit.action;
  }
  // B5: overlays can opt into plain-character capture ('printable'
  // pseudo-key). Exact bindings above always win; only single plain
  // characters fall through here — strokeToKey already canonicalized
  // modifier chords and named keys ahead of the literal char.
  if (keyId.length === 1 && keyId >= ' ') {
    for (const ctx of order) {
      const hit = bindings.find((b) => b.context === ctx && b.key === 'printable');
      if (hit !== undefined) return hit.action;
    }
  }
  return null;
}
```

`strokeToKey` unchanged.

- [ ] **Step 4: Run to verify pass**

```bash
bun test ./src/test/keybindings/resolve.test.ts && bun test ./src/test/keybindings/useBoundInput.test.tsx
```
Expected: all green (both files).

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git diff --cached   # must be empty
git add packages/brain-shell/src/keybindings/resolve.ts packages/brain-shell/src/test/keybindings/resolve.test.ts
git diff --cached
git commit -m "feat(shell): printable pseudo-key + overlay backspace in resolver

Overlays opt into type-away capture with one binding-table row; the
resolver probes 'printable' only for single plain characters and only
after the exact-match walk misses, so modifier chords, named keys, and
contexts without the row behave exactly as before.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Picker pure logic — `fuzzyScore`, `applyQueryEdit`, `resumeChoices(query)`

**Files:**
- Modify: `packages/brain-shell/src/ui/overlays/resumePickerLogic.ts`
- Test: `packages/brain-shell/src/test/ui/overlays/resumePickerLogic.test.ts`

**Interfaces:**
- Consumes: existing `ResumeVM`, `RESUME_MAX_ITEMS`, `formatAge`.
- Produces (Task 5 consumes all three):
  - `export function fuzzyScore(query: string, text: string): number | null`
  - `export function applyQueryEdit(query: string, action: string, input: string): string`
  - `resumeChoices(summaries, nowMs, query?)` — third optional parameter, default `''`.

- [ ] **Step 1: Write the failing tests**

Append to `resumePickerLogic.test.ts` (imports gain `applyQueryEdit, fuzzyScore`):

```ts
describe('fuzzyScore', () => {
  test('empty query matches everything neutrally', () => {
    expect(fuzzyScore('', 'anything')).toBe(0);
  });

  test('exact prefix beats scattered', () => {
    const exact = fuzzyScore('alp', 'Alpha Groove');
    const scattered = fuzzyScore('alp', 'xaxlxp other');
    expect(exact).not.toBeNull();
    expect(scattered).not.toBeNull();
    expect(exact! > scattered!).toBe(true);
  });

  test('word-boundary hits outrank mid-word hits', () => {
    const boundary = fuzzyScore('g', 'Alpha Groove');
    const midword = fuzzyScore('g', 'agenda');
    expect(boundary! > midword!).toBe(true);
  });

  test('non-subsequence returns null', () => {
    expect(fuzzyScore('zx', 'Alpha Groove')).toBeNull();
    expect(fuzzyScore('alpha', 'groove')).toBeNull(); // case-insensitive subsequence only
  });

  test('match is case-insensitive', () => {
    expect(fuzzyScore('AG', 'alpha groove')).not.toBeNull();
  });
});

describe('applyQueryEdit', () => {
  test('insert appends the typed character', () => {
    expect(applyQueryEdit('al', 'overlay:insert', 'p')).toBe('alp');
  });

  test('backspace removes the last character', () => {
    expect(applyQueryEdit('alp', 'overlay:backspace', '')).toBe('al');
    expect(applyQueryEdit('', 'overlay:backspace', '')).toBe('');
  });

  test('unknown actions leave the query unchanged', () => {
    expect(applyQueryEdit('al', 'overlay:up', '')).toBe('al');
    expect(applyQueryEdit('al', 'overlay:commit', '')).toBe('al');
  });
});

describe('resumeChoices with query (B5)', () => {
  test('empty query reproduces legacy ordering byte-for-byte', () => {
    const list = [
      s({ id: 'a', updatedAtMs: NOW - DAY }),
      s({ id: 'b', archived: true }),
      s({ id: 'c', pinned: true, updatedAtMs: NOW - 5 * DAY }),
      s({ id: 'd', updatedAtMs: NOW - MIN }),
    ];
    const withArg = resumeChoices(list, NOW, '');
    const withoutArg = resumeChoices(list, NOW);
    expect(withArg).toEqual(withoutArg);
    expect(withArg.map((v) => v.id)).toEqual(['c', 'd', 'a']);
  });

  test('filters across ALL sessions by fuzzy score, ranked', () => {
    const list = [
      s({ id: 'old-groove', title: 'Groove old', updatedAtMs: NOW - 9 * DAY }),  // beyond top-8 recency window
      s({ id: 'unrelated', title: 'Totally different', updatedAtMs: NOW - MIN }),
      s({ id: 'best', title: 'Alpha Groove', updatedAtMs: NOW - HOUR }),
    ];
    const out = resumeChoices(list, NOW, 'groove');
    expect(out.map((v) => v.id)).toEqual(['best', 'old-groove']);
  });

  test('archived excluded and cap still applies while searching', () => {
    const many = Array.from({ length: 12 }, (_, i) =>
      s({ id: `m${i}`, title: `needle item ${i}`, updatedAtMs: NOW - i * MIN }),
    );
    many.push(s({ id: 'arch', title: 'needle archived', archived: true }));
    const out = resumeChoices(many, NOW, 'needle');
    expect(out).toHaveLength(8);
    expect(out.some((v) => v.id === 'arch')).toBe(false);
  });

  test('score ties break by recency', () => {
    const list = [
      s({ id: 'older', title: 'Beta Session', updatedAtMs: NOW - 2 * HOUR }),
      s({ id: 'newer', title: 'Beta Session', updatedAtMs: NOW - MIN }),
    ];
    expect(resumeChoices(list, NOW, 'beta').map((v) => v.id)).toEqual(['newer', 'older']);
  });

  test('no matches yields empty array', () => {
    expect(resumeChoices([s({ id: 'a', title: 'T' })], NOW, 'zzz')).toEqual([]);
  });
});
```

- [ ] **Step 2: Run to verify failure**

```bash
cd packages/brain-shell && bun test ./src/test/ui/overlays/resumePickerLogic.test.ts
```
Expected: COMPILE/IMPORT FAILURE — `fuzzyScore`/`applyQueryEdit` not exported.

- [ ] **Step 3: Implement**

In `resumePickerLogic.ts`:

```ts
/**
 * B5: case-insensitive greedy subsequence score. Returns null when query
 * isn't a subsequence of text. Score rewards contiguous runs (+3 vs +1)
 * and word-boundary starts (+2) so "alp" prefers "Alpha Groove".
 */
export function fuzzyScore(query: string, text: string): number | null {
  if (query.length === 0) return 0;
  const q = query.toLowerCase();
  const t = text.toLowerCase();
  let score = 0;
  let searchFrom = 0;
  let prevIdx = -2;
  for (let qi = 0; qi < q.length; qi++) {
    const idx = t.indexOf(q[qi], searchFrom);
    if (idx === -1) return null;
    score += idx === prevIdx + 1 ? 3 : 1;
    if (idx === 0 || /[\s\-_/]/.test(t[idx - 1])) score += 2;
    prevIdx = idx;
    searchFrom = idx + 1;
  }
  return score;
}

/** B5: pure reducer turning overlay keyboard actions into query edits. */
export function applyQueryEdit(query: string, action: string, input: string): string {
  if (action === 'overlay:insert') return query + input;
  if (action === 'overlay:backspace') return query.slice(0, -1);
  return query;
}
```

Replace `resumeChoices` (keep `ResumeVM`, `RESUME_MAX_ITEMS`, `formatAge` untouched):

```ts
function toVm(nowMs: number) {
  return (s: BrainSessionSummary): ResumeVM => ({
    id: s.id,
    title: s.title,
    age: formatAge(nowMs, s.updatedAtMs),
    pinned: s.pinned,
  });
}

const byPinnedThenRecency = (a: BrainSessionSummary, b: BrainSessionSummary): number =>
  (b.pinned ? 1 : 0) - (a.pinned ? 1 : 0) || b.updatedAtMs - a.updatedAtMs;

export function resumeChoices(
  summaries: BrainSessionSummary[],
  nowMs: number,
  query: string = '',
): ResumeVM[] {
  const active = summaries.filter((s) => !s.archived);
  if (query.length === 0) {
    return active.sort(byPinnedThenRecency).slice(0, RESUME_MAX_ITEMS).map(toVm(nowMs));
  }
  return active
    .flatMap((s) => {
      const score = fuzzyScore(query, s.title);
      return score === null ? [] : [{ s, score }];
    })
    .sort((a, b) => b.score - a.score || b.s.updatedAtMs - a.s.updatedAtMs)
    .slice(0, RESUME_MAX_ITEMS)
    .map(({ s }) => toVm(nowMs)(s));
}
```

- [ ] **Step 4: Run to verify pass**

```bash
bun test ./src/test/ui/overlays/resumePickerLogic.test.ts
```
Expected: all green including pre-existing describes (empty-query path is behaviorally identical).

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git add packages/brain-shell/src/ui/overlays/resumePickerLogic.ts packages/brain-shell/src/test/ui/overlays/resumePickerLogic.test.ts
git commit -m "feat(shell): fuzzy scoring + typed-query filtering for resume picker

fuzzyScore is a case-insensitive subsequence walk rewarding contiguous
runs and word-boundary hits; applyQueryEdit is the pure keystroke
reducer; resumeChoices grows an optional query — empty query reproduces
today's pinned/recency top-8 exactly, non-empty ranks every non-archived
session score-first with recency tie-break under the same 8-row cap.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: View + AppShell wiring + controller getter

**Files:**
- Modify: `packages/brain-shell/src/ui/overlays/ResumePicker.tsx`
- Modify: `packages/brain-shell/src/ui/shell/AppShell.tsx` (resume overlay section, ~lines 53-55, 98-111, 161-172)
- Modify: `packages/brain-shell/src/state/sessionController.ts` (one getter)
- Test: Create `packages/brain-shell/src/test/ui/overlays/resumePickerView.test.tsx`
- Test: extend `packages/brain-shell/src/test/state/sessionControllerResume.test.ts`

**Interfaces:**
- Consumes: Task 3 actions (`overlay:insert`, `overlay:backspace` via `onAction(action, input, key)` — second arg is the raw input), Task 4 functions, `useBoundInput`'s existing `(action, input, key)` callback signature, `PALETTES.dark` from `state/palettes.js`, `MockBrainBackendClient` from `client/BrainBackendClient.js`.
- Produces: `ResumePickerView` props grow `{ query: string; currentSessionId?: string }`; controller grows `get activeSessionId(): string | undefined`.

- [ ] **Step 1: Write the failing view test**

Create `packages/brain-shell/src/test/ui/overlays/resumePickerView.test.tsx` following the house direct-function-call pattern (as in `thinkingRowView.test.tsx`: call the view function, don't JSX-mount it; walk rendered text):

```tsx
import { describe, expect, test } from 'bun:test';
import React from 'react';
import { ResumePickerView } from '../../../ui/overlays/ResumePicker.js';
import { PALETTES } from '../../../state/palettes.js';
import type { ResumeVM } from '../../../ui/overlays/resumePickerLogic.js';

function textOf(node: React.ReactNode): string {
  if (node === null || node === undefined || node === false || node === true) return '';
  if (typeof node === 'string' || typeof node === 'number') return String(node);
  if (Array.isArray(node)) return node.map(textOf).join('');
  const el = node as { props?: { children?: React.ReactNode } };
  return el.props ? textOf(el.props.children) : '';
}

const vm = (id: string, title: string): ResumeVM => ({ id, title, age: '2m ago', pinned: false });
const tokens = PALETTES.dark;

describe('ResumePickerView (B5)', () => {
  test('renders the live query line', () => {
    const out = ResumePickerView({
      items: [vm('a', 'Alpha')],
      selectedIndex: 0,
      tokens,
      query: 'alp',
      currentSessionId: undefined,
    });
    expect(textOf(out)).toContain('› alp');
  });

  test('marks the current session row with ●', () => {
    const out = ResumePickerView({
      items: [vm('live', 'Current'), vm('other', 'Other')],
      selectedIndex: 0,
      tokens,
      query: '',
      currentSessionId: 'live',
    });
    expect(textOf(out)).toContain('●');
    expect(textOf(out)).toContain('Current');
  });

  test('no marker when currentSessionId is absent or unmatched', () => {
    const plain = textOf(
      ResumePickerView({
        items: [vm('a', 'Alpha')],
        selectedIndex: 0,
        tokens,
        query: '',
      }),
    );
    expect(plain).not.toContain('●');
  });

  test('empty result renders the no-match line', () => {
    const out = ResumePickerView({
      items: [],
      selectedIndex: 0,
      tokens,
      query: 'zzz',
      currentSessionId: undefined,
    });
    expect(textOf(out)).toContain('No sessions match.');
  });

  test('hint mentions type-to-filter', () => {
    const out = ResumePickerView({
      items: [vm('a', 'Alpha')],
      selectedIndex: 0,
      tokens,
      query: '',
    });
    expect(textOf(out)).toContain('type to filter');
  });
});
```

Also extend `sessionControllerResume.test.ts` (mirror its existing fixture setup for constructing a controller with `MockBrainBackendClient`; add one test):

```ts
test('activeSessionId exposes the adopted session for the B5 marker', async () => {
  // Reuse this file's existing controller+client fixture construction.
  expect(controller.activeSessionId).toBeUndefined();
  await controller.resumeSession('<id-of-a-session-the-fixture-client-knows>');
  expect(controller.activeSessionId).toBe('<id-of-a-session-the-fixture-client-knows>');
});
```

(When writing this step, open the file and copy ITS fixture idioms verbatim — client stub, controller construction, and how existing resume tests seed loadable sessions.)

- [ ] **Step 2: Run to verify failure**

```bash
cd packages/brain-shell && bun test ./src/test/ui/overlays/resumePickerView.test.tsx ./src/test/state/sessionControllerResume.test.ts
```
Expected: FAIL — view doesn't accept `query`/`currentSessionId`; controller lacks the getter.

- [ ] **Step 3: Implement the view**

Rewrite `ResumePicker.tsx`:

```tsx
/** /resume overlay: prior sessions, pinned first, relative ages, typed filter. */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { ResumeVM } from './resumePickerLogic.js';

export function ResumePickerView(props: {
  items: readonly ResumeVM[];
  selectedIndex: number;
  tokens: BrainTokens;
  query?: string;
  currentSessionId?: string;
}): React.ReactElement {
  const sel = Math.min(props.selectedIndex, Math.max(0, props.items.length - 1));
  return (
    <Box flexDirection="column" borderStyle="round" borderColor={props.tokens.promptBorder} paddingX={1}>
      <Text bold>Resume session</Text>
      <Text>› {props.query ?? ''}▏</Text>
      {props.items.length === 0 ? (
        <Text dimColor>No sessions match.</Text>
      ) : (
        props.items.map((it, i) => (
          <Text key={it.id} inverse={i === sel}>
            {(i === sel ? '❯ ' : '  ') + (it.pinned ? '★ ' : '')}
            {it.id === props.currentSessionId ? (
              <Text dimColor>● </Text>
            ) : null}
            {`${it.title.slice(0, 46)} — ${it.age}`}
          </Text>
        ))
      )}
      <Text dimColor>↑↓ navigate · enter resume · esc cancel · type to filter</Text>
    </Box>
  );
}
```

(`query`/`currentSessionId` optional keeps any other callers compiling; AppShell passes both explicitly.)

- [ ] **Step 4: Add the controller getter**

In `sessionController.ts`, near `getSnapshot` (~line 106):

```ts
  /** B5: adopted session id, for the resume picker's current-session marker. */
  get activeSessionId(): string | undefined {
    return this.sessionId;
  }
```

- [ ] **Step 5: Wire AppShell**

In `AppShell.tsx`:

(a) Replace the `resumeItems` state with summaries + derived items. Change state declarations (~lines 53-55):

```tsx
  const [resumeOpen, setResumeOpen] = React.useState(false);
  const [resumeSummaries, setResumeSummaries] = React.useState<BrainSessionSummary[]>([]);
  const [resumeSelected, setResumeSelected] = React.useState(0);
  const [resumeQuery, setResumeQuery] = React.useState('');
  const resumeItems = React.useMemo(
    () => resumeChoices(resumeSummaries, Date.now(), resumeQuery),
    [resumeSummaries, resumeQuery],
  );
```

Import `BrainSessionSummary` type alongside the existing client-type imports, and add `applyQueryEdit` to the existing `resumePickerLogic.js` import list.

(b) In the resume `useBoundInput` handler (~line 99), add the two edit cases AFTER the existing decision branches (decision returns passthrough for unknown actions, so no conflict):

```tsx
    onAction: (action, input) => {
      const d = resumeListDecision(action, resumeSelected, resumeItems.length);
      if (d.type === 'move') {
        setResumeSelected(d.index);
      } else if (d.type === 'commit') {
        setResumeOpen(false);
        const chosen = resumeItems[d.index];
        if (chosen) void controller.resumeSession(chosen.id);
      } else if (d.type === 'cancel') {
        setResumeOpen(false);
      } else if (action === 'overlay:insert') {
        setResumeQuery((q) => applyQueryEdit(q, action, input));
      } else if (action === 'overlay:backspace') {
        setResumeQuery((q) => applyQueryEdit(q, action, input));
      }
    },
```

(c) Keep selection clamped when the filtered list shrinks — add alongside the permission effect (~line 58):

```tsx
  React.useEffect(() => {
    setResumeSelected((i) => Math.min(i, Math.max(0, resumeItems.length - 1)));
  }, [resumeItems.length]);
```

(d) In the `/resume` command branch (~line 161), store summaries and reset the query:

```tsx
    } else if (chosen.name === 'resume') {
      if (snapshot.busy) {
        controller.notice('Busy — wait for the current turn to finish.');
        return;
      }
      void controller.listSessions().then((all) => {
        if (resumeChoices(all, Date.now()).length === 0) {
          controller.notice('No previous sessions found.');
          return;
        }
        setResumeSummaries(all);
        setResumeQuery('');
        setResumeSelected(0);
        setResumeOpen(true);
      });
    }
```

(Preserve whatever `setResumeOpen(true)` placement the current code uses — read the block first; the point is: fetch once, store raw summaries, clear query/selection.)

(e) Pass the new props where the view renders:

```tsx
      <ResumePickerView
        items={resumeItems}
        selectedIndex={resumeSelected}
        tokens={tokens}
        query={resumeQuery}
        currentSessionId={controller.activeSessionId}
      />
```

(Match the actual render-site prop style — `tokens` variable name may differ in AppShell; use whatever expression supplies `BrainTokens` today.)

- [ ] **Step 6: Run to verify pass + full shell suite identity check**

```bash
bun test ./src/test/ui/overlays/resumePickerView.test.tsx ./src/test/state/sessionControllerResume.test.ts
bun test 2>&1 | tail -15
```
Expected: new files green; whole-suite failures remain EXACTLY the documented five identities (visualCellParity ×2, sessionSemanticIntegration UdsBrainBackendClient session RPC, brainMemoryIntegration Gate 5, brainTurnTransformer Scenario 8) — compare by test NAME, not counts.

- [ ] **Step 7: Typecheck the touched files**

```bash
bunx tsc --noEmit 2>&1 | sed -r 's/\x1b\[[0-9;]*m//g' | grep -E "error TS" | wc -l
```
Compare against the pristine-main count captured BEFORE starting this task (run the same command on a stash-free main checkout — practically: record the count from the Inc 19 finishing window, 433; the delta must be zero NEW errors in files this task touched).

- [ ] **Step 8: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git add packages/brain-shell/src/ui/overlays/ResumePicker.tsx packages/brain-shell/src/ui/shell/AppShell.tsx packages/brain-shell/src/state/sessionController.ts packages/brain-shell/src/test/ui/overlays/resumePickerView.test.tsx packages/brain-shell/src/test/state/sessionControllerResume.test.ts
git diff --cached
git commit -m "feat(shell): type-away search + current-session marker in /resume

AppShell keeps the query state and recomputes choices from stored
summaries per keystroke; the view renders the query line, a dimmed ●
marker on the adopted session, and a no-match empty state. Controller
exposes activeSessionId for the marker. Selection clamps as results
shrink; opening the picker resets query and selection.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: PTY smoke + full gates

**Files:**
- Create: `scripts/ptySmokeInc20.py`

**Interfaces:**
- Consumes: completed Tasks 1–5; smoke conventions from `scripts/ptySmokeInc19.py` (ANSI strip, occurrence-count waits, per-keystroke writes with ~0.3s pumps); REAL daemon binary this time (auto-titles need the Rust stack).
- Produces: end-to-end proof artifact for the increment.

- [ ] **Step 1: Write `scripts/ptySmokeInc20.py`**

Structure (full script; adjust constants only if the environment demands it):

```python
#!/usr/bin/env python3
"""Increment 20 PTY smoke: /resume fuzzy search + auto-titles against a REAL daemon.

Flows:
  A. Seed two titled sessions via RPC ("Alpha Groove Notes", "Beta Ledger"),
     open /resume, type-away a fuzzy fragment ("agrv") -> only Alpha remains,
     enter -> resumed transcript replays.
  B. Marker: after adopting a session, reopen /resume -> the ● sits on the
     live session's row.
  C. Auto-title: create an UNTITLED session via RPC, drive one turn through
     the SAME daemon via RPC, then open /resume -> the row shows the derived
     prompt title, not "New Session".
"""
import fcntl, json, os, pty, re, select, shutil, signal, socket, struct, subprocess, sys, termios, time, uuid

ROWS, COLS = 30, 100
REPO = "/Users/ritikpathania/Developer/PyCharm/brain"
PKG_DIR = f"{REPO}/packages/brain-shell"
TMP = "/tmp/brain-inc20-smoke"
SOCK = f"{TMP}/brain.sock"
CONFIG_FILE = f"{TMP}/config.json"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

ALPHA_TITLE = "Alpha Groove Notes"
BETA_TITLE = "Beta Ledger"

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

os.makedirs(TMP, exist_ok=True)
with open(CONFIG_FILE, "w") as f:
    json.dump({"theme": "auto"}, f)

# ── Real daemon on a private socket/db ────────────────────────────────────
for p in (SOCK, f"{TMP}/brain.pid"):
    if os.path.exists(p):
        os.remove(p)
env = dict(os.environ)
env.update({
    "BRAIN_SOCKET_PATH": SOCK,
    "BRAIN_PID_PATH": f"{TMP}/brain.pid",
    "BRAIN_DB_PATH": f"{TMP}/brain.db",
    "BRAIN_ANALYTICS_DB_PATH": f"{TMP}/analytics.db",
    "BRAIN_CONFIG_DIR": TMP,
    "BRAIN_HEALTH_PORT": "0",
})
daemon = subprocess.Popen(
    ["target/debug/brain-daemon", "daemon", "run"], cwd=REPO, env=env,
    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
)
deadline = time.time() + 30
while time.time() < deadline:
    if os.path.exists(SOCK):
        try:
            probe = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            probe.connect(SOCK)
            probe.close()
            break
        except OSError:
            pass
    time.sleep(0.2)
else:
    sys.exit("FAIL: daemon never bound the socket")

def rpc(action, body, timeout=10.0):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(timeout)
    s.connect(SOCK)
    fobj = s.makefile("rw")
    req = {"version": "1.0", "type": "Request", "id": f"smoke-{uuid.uuid4().hex[:8]}",
           "action": action, "body": json.dumps(body)}
    fobj.write(json.dumps(req) + "\n"); fobj.flush()
    line = fobj.readline()
    resp = json.loads(line)
    s.close()
    return json.loads(resp["body"])

def stream_one_turn(sid, prompt):
    """One generation turn over the raw-frame envelope; drains to finished."""
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(15.0)
    s.connect(SOCK)
    fobj = s.makefile("rw")
    gen = {"id": f"gen-{uuid.uuid4().hex[:8]}", "action": "v1/generation/stream",
           "payload": {"sessionId": sid,
                       "messages": [{"role": "user", "content": prompt}],
                       "model": "brain-default"}}
    fobj.write(json.dumps(gen) + "\n"); fobj.flush()
    for line in fobj:
        fr = json.loads(line)
        if fr.get("type") in ("finished", "error"):
            break
    s.close()

def seed(title):
    body = rpc("v1/session/create", {"title": title})
    sid = body["session_id"]
    # One turn so updatedAt ordering and content exist.
    stream_one_turn(sid, f"seed turn for {title}")
    return sid

ALPHA_SID = seed(ALPHA_TITLE)
BETA_SID = seed(BETA_TITLE)

# Untitled session + one turn through the SAME daemon: the auto-title rule
# must fire server-side and rename it before any picker is opened.
UNTITLED_SID = rpc("v1/session/create", {})["session_id"]
stream_one_turn(UNTITLED_SID, "Plan the quarterly offsite")

failures = []
def check(name, cond):
    print(("PASS " if cond else "FAIL ") + name)
    if not cond:
        failures.append(name)

# ── Shell under PTY ───────────────────────────────────────────────────────
pid, fd = pty.fork()
if pid == 0:
    os.chdir(PKG_DIR)
    os.environ["BRAIN_SOCKET_PATH"] = SOCK
    os.environ["NODE_ENV"] = "production"
    os.execvp("bun", ["bun", "run", "src/main.tsx"])
fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

buf = bytearray()
def pump(seconds=0.4):
    end = time.time() + seconds
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.05)
        if r:
            try:
                chunk = os.read(fd, 65536)
                if not chunk:
                    return
                buf.extend(chunk)
            except OSError:
                return

def send_key(ch, delay=0.35):
    os.write(fd, ch.encode())
    pump(delay)

def wait_for(needle, timeout=25.0, count=1):
    end = time.time() + timeout
    while time.time() < end:
        if clean(bytes(buf)).count(needle) >= count:
            return True
        pump(0.2)
    return False

wait_for("Ready", timeout=40)  # composer welcome; adjust needle to the real welcome line if different
```

Flow A — type-away filter:

```python
send_key("/")                       # open command palette
pump(0.4)
for ch in "resume":
    send_key(ch, 0.12)
check("A1 palette shows resume", wait_for("resume", count=2))  # palette entry + hint
send_key("\r")                      # submit palette entry
check("A2 picker opens", wait_for("Resume session"))
check("A3 both rows visible", wait_for(ALPHA_TITLE) and clean(bytes(buf)).count(BETA_TITLE) >= 1)
for ch in "agrv":
    send_key(ch, 0.3)
# Judge only the tail AFTER the last query-line repaint: stale earlier frames
# still contain both titles, so whole-buffer matching would false-pass/fail.
tail = clean(bytes(buf)).rsplit("agrv", 1)[-1]
check("A4 alpha survives filter", ALPHA_TITLE in tail)
check("A5 beta filtered out", BETA_TITLE not in tail)
send_key("\r")                      # resume Alpha
check("A6 resumed transcript replays", wait_for(f"seed turn for {ALPHA_TITLE}", timeout=30))
```

Flow B — marker (reopen picker after adoption):

```python
send_key("/")
for ch in "resume":
    send_key(ch, 0.12)
send_key("\r")
check("B1 picker reopens", wait_for("Resume session"))
# Scan REVERSED: the buffer holds every repaint; the LAST matching line is
# from the most recent paint, which is the one carrying the live marker.
screen_now = clean(bytes(buf))
alpha_row = next((ln for ln in reversed(screen_now.splitlines()) if ALPHA_TITLE in ln), "")
check("B2 marker on adopted row", "●" in alpha_row)
send_key("\x1b")                    # esc closes without changing session
```

Flow C — auto-titled row visible and searchable in the picker. (Do NOT assert
`"New Session"` is absent from the whole screen: the shell's own boot session
is legitimately titled "New Session". The honest proof is that the untitled
session's row now shows — and fuzzy-matches — its derived prompt title.)

```python
send_key("/")
for ch in "resume":
    send_key(ch, 0.12)
send_key("\r")
check("C1 picker shows derived title", wait_for("Plan the quarterly offsite"))
for ch in "offsite":
    send_key(ch, 0.3)
ctail = clean(bytes(buf)).rsplit("offsite", 1)[-1]
check("C2 derived title is fuzzy-searchable", "Plan the quarterly offsite" in ctail)
send_key("\x1b")
```

Teardown:

```python
os.write(fd, b"\x03")  # ctrl+c exit shell
pump(0.5)
try:
    os.kill(pid, signal.SIGTERM)
except ProcessLookupError:
    pass
try:
    daemon.terminate()
    daemon.wait(timeout=10)
except subprocess.TimeoutExpired:
    daemon.kill()
print("FAILURES:", len(failures))
sys.exit(1 if failures else 0)
```

IMPORTANT calibration notes for whoever runs this:
- The welcome-line needle in `wait_for("Ready", …)` MUST be checked against what the shell actually prints at boot (run once interactively, look at the buffer) — Inc 19's smoke used concrete needles from the real UI, never guessed ones.
- Per-keystroke writes + ≥0.3s pumps are mandatory (Ink coalesces chunks into paste).
- If the palette flow differs (e.g., `/` isn't the palette chord in this build), read `AppShell.tsx`'s command dispatch to find the real entry gesture and use it — do NOT weaken assertions to make a wrong gesture pass.
- Flow A5's split-on-last-fragment guards against stale-buffer matches; if flaky, widen to occurrence-count logic per memory note 8.

- [ ] **Step 2: Build the daemon binary the script needs**

```bash
bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo build -p brain-daemon'
ls -la target/debug/brain-daemon
```
Expected: binary exists.

- [ ] **Step 3: Run the smoke**

```bash
python3 scripts/ptySmokeInc20.py
```
Expected: `PASS` on every check line, `FAILURES: 0`, exit 0. Iterate ONLY per the calibration notes above — never by loosening what is asserted.

- [ ] **Step 4: Full workspace gate**

```bash
bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test --workspace --no-fail-fast' > /tmp/inc20-workspace.log 2>&1; echo "exit=$?"
grep '\.\.\. FAILED' /tmp/inc20-workspace.log || echo "(no failing tests)"
```
Expected: failure set ⊆ {`uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers`} ∪ {documented flake family}. Any other failure → investigate, do not proceed.

- [ ] **Step 5: Vendor scan**

```bash
git diff 664d76e4..HEAD -- packages/brain-shell/src crates daemon scripts | grep '^+' | grep -icE 'claude|anthropic|vendor' || echo 0
```
Expected: 0.

- [ ] **Step 6: Commit**

```bash
git add scripts/ptySmokeInc20.py
git commit -m "test(smoke): Inc 20 wire-level resume search & auto-title proof

Real-daemon PTY flows: type-away fuzzy narrowing to the matching session
with resumed replay, the ● marker on the adopted row, and an untitled
session showing its derived prompt title in the picker.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Completion

Announce the finishing-a-development-branch skill; present the standard 3-option menu (base branch: `main`). Merge mechanics per memory note 13: fast-forward via `git fetch . feature/brain-shell-inc20-resume-search-titles:main` then `git checkout main` — never a working-tree-touching merge in this repo. Push only on explicit user approval.
