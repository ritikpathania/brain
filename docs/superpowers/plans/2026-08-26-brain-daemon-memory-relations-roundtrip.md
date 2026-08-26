# Brain Daemon Memory Relations Round-Trip Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stored memory relations survive `v1/memory/store` → `v1/memory/search`; excerpts come from stored content, scopes from stored properties — via an internal daemon-side fix with zero IPC/schema/domain changes.

**Architecture:** The application-layer search projection drops all node properties (its `SearchMetadata` enum has no property channel). Rather than widening that domain type, the daemon's UDS `memory/search` handler enriches each hit at the boundary: when a summary id parses to a stored node UUID, relations/excerpt/scope are recovered from `nodes.properties` through three pure helpers with tolerant decode order.

**Tech Stack:** Rust (tokio, serde_json, rusqlite via existing repos); Python PTY harness; bun/TypeScript suites stay green unchanged.

**Spec:** `docs/superpowers/specs/2026-08-26-brain-daemon-memory-relations-roundtrip-design.md`

## Global Constraints

- Every cargo call wrapped: `RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks"` (pyo3 @rpath on this Mac).
- Package is `-p brain-daemon`, never `-p daemon`.
- NEVER `git stash`; never `git add .` — explicit paths only; commit trailer `Co-Authored-By: Claude <noreply@anthropic.com>` on every commit.
- Working tree carries ~3.7k user-WIP dirty paths: preserve byte-for-byte. Before staging ANY shared file run `git diff HEAD -- <file>`; foreign hunks ⇒ hunk-filter/update-index recipe (repo memory #12).
- Hands-off UNTRACKED user-WIP files (never stage/extend/edit): `daemon/tests/integration_uds_session.rs`, `daemon/tests/uds_memory_retrieval_tests.rs`, `daemon/tests/uds_security_audit_tests.rs`, `crates/brain-core/src/{model,context}.rs`, `crates/brain-core/src/reasoning/`.
- Zero diffs allowed: `crates/**`, any `*.tsx/ts` under `packages/brain-shell/src/**` except none (this increment touches NO TypeScript production code), `daemon/src/server/protocol.rs`.
- Known pre-existing failure identity (NOT attributable): untracked `uds_security_audit_tests.rs` codes mismatch. bun shell suite keeps exactly its documented five identities.
- Pushes to origin need explicit user approval each time. Work IN PLACE — no worktrees.
- zsh notes: quote `echo "==="`; `grep -c` exits 1 on zero (append `|| true`).

## File Structure

| Action | Path | Responsibility |
|---|---|---|
| Create | `daemon/src/transport/uds/memory_relations.rs` | Pure relation/excerpt/scope recovery helpers + unit tests |
| Modify | `daemon/src/transport/uds/mod.rs` (+2 lines) | Register the module |
| Modify | `daemon/src/transport/uds/handlers.rs` (~search block :1306-1408) | Enrich hits from stored nodes |
| Create | `daemon/tests/uds_memory_relations_tests.rs` | Wire-level round-trip proof (tracked; sibling-harness pattern) |
| Modify | `scripts/ptySmokeInc21.py` | B3 flips to expect `"Beta Concept"` |

---

### Task 1: Pure recovery helpers (`memory_relations.rs`)

**Files:**
- Create: `daemon/src/transport/uds/memory_relations.rs`
- Modify: `daemon/src/transport/uds/mod.rs`

**Interfaces:**
- Consumes: nothing (pure `serde_json`).
- Produces (Task 2 relies on exact signatures):
  - `pub fn extract_relations(props: Option<&serde_json::Map<String, serde_json::Value>>, metadata_fallback: Option<&str>) -> Vec<serde_json::Value>`
  - `pub fn preferred_excerpt(props: Option<&serde_json::Map<String, serde_json::Value>>, fallback_body: &str) -> String`
  - `pub fn preferred_scope(props: Option<&serde_json::Map<String, serde_json::Value>>, fallback: &str) -> String`

- [ ] **Step 1: Register module**

In `daemon/src/transport/uds/mod.rs`, after the `router` line, add:

```rust
/// Property-recovery helpers for memory/search (relations, excerpt, scope).
pub mod memory_relations;
```

- [ ] **Step 2: Create the file with STUBS + full unit tests**

Create `daemon/src/transport/uds/memory_relations.rs`:

```rust
//! Property recovery for memory/search: the application-layer projection
//! drops node properties (the `SearchMetadata` enum carries none), so
//! relations/excerpt/scope are read back from the stored node here.

use serde_json::{Map, Value};

/// Resolve relations from a stored node's property map, tolerating native
/// JSON arrays and legacy string-encoded JSON, falling back to the summary
/// metadata string when the node carries nothing usable. Non-array garbage
/// falls through; never errors.
pub fn extract_relations(
    props: Option<&Map<String, Value>>,
    metadata_fallback: Option<&str>,
) -> Vec<Value> {
    Vec::new() // TODO: stub — implemented in Step 4
}

/// Prefer the stored `content` property as the excerpt; fall back to whatever
/// body the retrieval pipeline produced (today: the node label).
pub fn preferred_excerpt(props: Option<&Map<String, Value>>, fallback_body: &str) -> String {
    fallback_body.to_string() // TODO: stub — implemented in Step 4
}

/// Resolve scope from properties, falling back to the provided default.
pub fn preferred_scope(props: Option<&Map<String, Value>>, fallback: &str) -> String {
    fallback.to_string() // TODO: stub — implemented in Step 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn relations_from_native_array_property() {
        let props = map(json!({"relations": [{"relation": "supports", "target_id": "beta-1"}]}));
        let rels = extract_relations(Some(&props), None);
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0]["target_id"], "beta-1");
    }

    #[test]
    fn relations_from_legacy_string_encoded_property() {
        let props = map(json!({"relations": "[{\"relation\":\"supports\"}]"}));
        assert_eq!(extract_relations(Some(&props), None).len(), 1);
    }

    #[test]
    fn relations_fall_back_to_summary_metadata_string() {
        let encoded = "[{\"relation\":\"supports\",\"target_id\":\"x\"}]";
        assert_eq!(extract_relations(None, Some(encoded)).len(), 1);
    }

    #[test]
    fn relations_empty_when_nothing_carries_them() {
        let props = map(json!({"content": "just prose"}));
        let rels = extract_relations(Some(&props), Some("not json"));
        assert!(rels.is_empty());
    }

    #[test]
    fn relations_garbage_property_falls_through_to_metadata() {
        let props = map(json!({"relations": 42}));
        let rels = extract_relations(Some(&props), Some("[]"));
        assert!(rels.is_empty());
    }

    #[test]
    fn empty_array_is_preserved_not_treated_as_missing() {
        // A node stored WITH an empty relations list must not silently
        // resurrect entries from the metadata fallback.
        let props = map(json!({"relations": []}));
        assert!(extract_relations(Some(&props), Some("[{\"relation\":\"x\"}]")).is_empty());
    }

    #[test]
    fn excerpt_prefers_content_over_label_body() {
        let props = map(json!({"content": "real prose body"}));
        assert_eq!(preferred_excerpt(Some(&props), "Alpha Label"), "real prose body");
    }

    #[test]
    fn excerpt_blank_content_falls_back() {
        let props = map(json!({"content": "   "}));
        assert_eq!(preferred_excerpt(Some(&props), "Alpha Label"), "Alpha Label");
    }

    #[test]
    fn excerpt_without_props_returns_fallback() {
        assert_eq!(preferred_excerpt(None, "Alpha Label"), "Alpha Label");
    }

    #[test]
    fn scope_prefers_property_then_default() {
        let props = map(json!({"scope": "compiler"}));
        assert_eq!(preferred_scope(Some(&props), "workspace"), "compiler");
        assert_eq!(preferred_scope(None, "workspace"), "workspace");
    }
}
```

- [ ] **Step 3: Run tests — verify genuine RED**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --lib memory_relations 2>&1 | tail -20
```

Expected: compiles; **exactly these four FAIL** (stubs return defaults):
`relations_from_native_array_property`, `relations_from_legacy_string_encoded_property`,
`relations_fall_back_to_summary_metadata_string`, `excerpt_prefers_content_over_label_body`,
`scope_prefers_property_then_default`. (Five expected failures; the vacuous-by-stub
tests pass and stay green throughout.) Record actual failure names from output.

- [ ] **Step 4: Implement real bodies**

Replace the three stub bodies:

```rust
pub fn extract_relations(
    props: Option<&Map<String, Value>>,
    metadata_fallback: Option<&str>,
) -> Vec<Value> {
    let decode = |raw: &Value| -> Option<Vec<Value>> {
        match raw {
            Value::Array(items) => Some(items.clone()),
            Value::String(s) => serde_json::from_str::<Vec<Value>>(s).ok(),
            _ => None,
        }
    };
    if let Some(map) = props {
        if let Some(raw) = map.get("relations") {
            if let Some(rels) = decode(raw) {
                return rels;
            }
        }
    }
    if let Some(encoded) = metadata_fallback {
        if let Ok(rels) = serde_json::from_str::<Vec<Value>>(encoded) {
            return rels;
        }
    }
    Vec::new()
}
```

```rust
pub fn preferred_excerpt(props: Option<&Map<String, Value>>, fallback_body: &str) -> String {
    props
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback_body.to_string())
}
```

```rust
pub fn preferred_scope(props: Option<&Map<String, Value>>, fallback: &str) -> String {
    props
        .and_then(|m| m.get("scope"))
        .and_then(|s| s.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback.to_string())
}
```

(Delete both `// TODO: stub` comments.)

- [ ] **Step 5: Run tests — verify GREEN**

Same command as Step 3. Expected: **11 passed; 0 failed**.

- [ ] **Step 6: Commit**

```bash
git diff HEAD -- daemon/src/transport/uds/mod.rs   # must be empty besides your 2 lines
git add daemon/src/transport/uds/mod.rs daemon/src/transport/uds/memory_relations.rs
git commit -m "feat(daemon): relation/excerpt/scope recovery helpers for memory search

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Handler wiring + wire-level regression suite

**Files:**
- Create: `daemon/tests/uds_memory_relations_tests.rs`
- Modify: `daemon/src/transport/uds/handlers.rs` (search block ~:1319-1367)

**Interfaces:**
- Consumes: Task 1 helpers via `super::memory_relations::{extract_relations, preferred_excerpt, preferred_scope}`; `NodeRepository::find_by_id(storage.as_ref(), &NodeId)` (pattern proven at `crates/brain-services/src/brain_runtime.rs:993-1004`); `uuid` crate (direct dep, `daemon/Cargo.toml:22`, already used in handlers.rs).
- Produces: wire behavior — `MemoryItemDto{relations, excerpt, scope}` populated from stored node properties when `summary.id` is a node UUID; sessions/messages unchanged.

- [ ] **Step 1: Create the failing integration suite**

Create `daemon/tests/uds_memory_relations_tests.rs`. Helpers below are copied VERBATIM from the tracked sibling `uds_session_autotitle_tests.rs` (:15-115, minus generation-stream pieces):

```rust
//! Memory relations round-trip over UDS: what memory/store persists,
//! memory/search must return — including relations, content-derived
//! excerpts, and stored scope.

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
    pid_path: PathBuf,
    db_path: PathBuf,
    analytics_db_path: PathBuf,
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
    let path = PathBuf::from(format!("/tmp/bd-memrel-{}", &uuid_str[0..8]));
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

fn spawn_daemon(test_dir: &PathBuf, socket: &PathBuf, pid: &PathBuf, db: &PathBuf, analytics: &PathBuf) -> Child {
    Command::new(env!("CARGO_BIN_EXE_brain-daemon"))
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", socket)
        .env("BRAIN_PID_PATH", pid)
        .env("BRAIN_DB_PATH", db)
        .env("BRAIN_ANALYTICS_DB_PATH", analytics)
        .env("BRAIN_CONFIG_DIR", test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start daemon process")
}

async fn start_daemon_at(test_dir: PathBuf) -> DaemonProcess {
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");
    let child = spawn_daemon(&test_dir, &socket_path, &pid_path, &db_path, &analytics_db_path);
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
        pid_path,
        db_path,
        analytics_db_path,
    }
}

/// One versioned RPC round-trip; returns the parsed response BODY.
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

#[tokio::test]
async fn stored_relations_round_trip_through_search() {
    let d = start_daemon_at(get_temp_dir()).await;

    let stored = rpc(
        &d.socket_path,
        1,
        "memory/store",
        serde_json::json!({
            "label": "Alpha Cortex Node",
            "content": "Cortex excerpt body for the smoke",
            "scope": "workspace",
            "relations": [
                {"relation": "supports", "target_id": "beta-1", "target_label": "Beta Concept"}
            ],
        }),
    )
    .await;
    assert_eq!(stored["success"], true, "store failed: {stored}");

    let found = rpc(
        &d.socket_path,
        2,
        "memory/search",
        serde_json::json!({"query": "cortex", "limit": 10}),
    )
    .await;
    let memories = found["memories"]
        .as_array()
        .expect("memories array in response body")
        .clone();
    assert!(memories.len() >= 1, "seeded node must be returned: {found}");
    let first = &memories[0];
    assert_eq!(first["label"], "Alpha Cortex Node");
    // Excerpt must be the STORED CONTENT, not the echoed label.
    assert_eq!(first["excerpt"], "Cortex excerpt body for the smoke");
    assert_eq!(first["scope"], "workspace");
    let rels = first["relations"].as_array().expect("relations array").clone();
    assert_eq!(rels.len(), 1, "stored relation must round-trip: {first}");
    assert_eq!(rels[0]["relation"], "supports");
    assert_eq!(rels[0]["target_id"], "beta-1");
    assert_eq!(rels[0]["target_label"], "Beta Concept");
}

#[tokio::test]
async fn store_without_relations_yields_empty_relation_list() {
    let d = start_daemon_at(get_temp_dir()).await;

    let stored = rpc(
        &d.socket_path,
        1,
        "memory/store",
        serde_json::json!({"label": "Plain Node", "content": "plain body"}),
    )
    .await;
    assert_eq!(stored["success"], true);

    let found = rpc(
        &d.socket_path,
        2,
        "memory/search",
        serde_json::json!({"query": "plain", "limit": 5}),
    )
    .await;
    let memories = found["memories"].as_array().expect("memories array");
    assert!(memories.len() >= 1);
    assert_eq!(
        memories[0]["relations"].as_array().expect("relations key").len(),
        0,
        "absent relations must surface as an empty list"
    );
}
```

- [ ] **Step 2: Run suite — verify genuine RED**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --test uds_memory_relations_tests 2>&1 | tee "$CLAUDE_JOB_DIR/tmp/inc22-t2-red.log" | tail -25
```

Expected: builds (slow first time — links the daemon binary); **`stored_relations_round_trip_through_search` FAILS** at the excerpt assertion (today excerpt == label) and/or relations length; **`store_without_relations_yields_empty_relation_list` PASSES** (current behavior already emits `[]`). If the round-trip test unexpectedly passes, STOP — recon premise wrong, re-investigate.

- [ ] **Step 3: Wire the handler**

In `daemon/src/transport/uds/handlers.rs`, inside the `memory/search` block:

(a) After the line `let context = ExecutionContext::default();` insert:

```rust
            let storage = app.runtime().sqlite_storage();
```

(b) Replace this exact existing block:

```rust
                    let relations = summary
                        .metadata
                        .get("relations")
                        .and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(s).ok())
                        .unwrap_or_default();

                    matches.push(crate::server::protocol::MemoryItemDto {
                        node_id: summary.id,
                        label: clean_title,
                        excerpt: summary.body,
                        score,
                        channel: "knowledge_graph".to_string(),
                        timestamp: now,
                        scope: "workspace".to_string(),
                        relations,
                    });
```

with:

```rust
                    // Enrich from the stored node when the summary resolves
                    // to one: the application-layer projection drops node
                    // properties (SearchMetadata carries none), so relations/
                    // excerpt/scope are recovered here at the boundary.
                    // Sessions/messages fail the UUID parse and keep today's
                    // behavior exactly.
                    let node_props = uuid::Uuid::parse_str(summary.id.trim())
                        .ok()
                        .and_then(|u| {
                            let nid = brain_domain::NodeId(u);
                            brain_core::repositories::NodeRepository::find_by_id(
                                storage.as_ref(),
                                &nid,
                            )
                            .ok()
                            .flatten()
                            .map(|n| n.properties)
                        });
                    let relations = super::memory_relations::extract_relations(
                        node_props.as_ref(),
                        summary.metadata.get("relations").map(|s| s.as_str()),
                    );
                    let excerpt = super::memory_relations::preferred_excerpt(
                        node_props.as_ref(),
                        &summary.body,
                    );
                    let scope =
                        super::memory_relations::preferred_scope(node_props.as_ref(), "workspace");

                    matches.push(crate::server::protocol::MemoryItemDto {
                        node_id: summary.id,
                        label: clean_title,
                        excerpt,
                        score,
                        channel: "knowledge_graph".to_string(),
                        timestamp: now,
                        scope,
                        relations,
                    });
```

(Borrow order is load-bearing: `summary.id` is borrowed by the parse, then moved into the DTO last.)

- [ ] **Step 4: Run suite — verify GREEN**

```bash
RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --test uds_memory_relations_tests 2>&1 | tail -5
```

Expected: **2 passed; 0 failed**.

- [ ] **Step 5: Unit tests still green + no cross-suite damage in scoped run**

```bash
RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon memory_relations 2>&1 | tail -4
```

Expected: 11 unit + 2 integration pass (filter matches both targets).

- [ ] **Step 6: Commit**

```bash
git diff HEAD -- daemon/src/transport/uds/handlers.rs   # inspect: ONLY your hunk may stage
# handlers.rs is tracked-clean today; if foreign WIP appears, apply the
# hunk-filter/update-index recipe BEFORE adding.
git add daemon/src/transport/uds/handlers.rs daemon/tests/uds_memory_relations_tests.rs
git commit -m "fix(daemon): recover stored relations/excerpt/scope in memory search

The application-layer projection drops node properties entirely, so
memory/search returned relations:[] and label-as-excerpt for every stored
memory. When a summary id resolves to a stored node, the UDS boundary now
recovers relations/excerpt/scope from nodes.properties through the pure
helpers; sessions/messages are untouched. Zero IPC/schema changes.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Smoke flip + full gates + finish

**Files:**
- Modify: `scripts/ptySmokeInc21.py`

**Interfaces:**
- Consumes: Tasks 1–2 committed behavior.
- Produces: end-to-end UI proof that the seeded relation target renders; gate battery results.

- [ ] **Step 1: Flip the smoke expectation**

In `scripts/ptySmokeInc21.py` replace the docstring lines:

```
  B. Seed one memory via RPC (with a relation), type /memory -> modal opens,
     type "cortex" to filter -> seeded node lists, enter expands the detail
     pane, esc closes with the system notice. (Expansion asserts the
     no-relations row: the daemon's store->search round-trip currently
     returns relations:[] — client-side preservation of POPULATED relations
     is proven separately by test/client/memorySearchWire.test.ts.)
```

with:

```
  B. Seed one memory via RPC (with a relation), type /memory -> modal opens,
     type "cortex" to filter -> seeded node lists, enter expands the detail
     pane showing the stored relation target ("Beta Concept"), esc closes
     with the system notice.
```

and replace:

```python
# Detail pane opens; daemon currently round-trips relations as [], so the
# honest end-to-end expectation is the explicit none-row.
check("B3 expand opens the detail pane", wait_for("(No outgoing relations)", timeout=15))
```

with:

```python
# Detail pane opens; the daemon recovers stored relations at the
# memory/search boundary, so the expanded pane renders the seeded edge target.
check("B3 expand renders the stored relation target", wait_for("Beta Concept", timeout=15))
```

Verify first the file is tracked-clean: `git status --porcelain scripts/ptySmokeInc21.py` → empty.

- [ ] **Step 2: Run the smoke against a REAL daemon — 10/10**

```bash
python3 scripts/ptySmokeInc21.py
```

Expected: every check PASS including `B3 expand renders the stored relation target`; `FAILURES: 0`.

- [ ] **Step 3: Commit the flip**

```bash
git add scripts/ptySmokeInc21.py
git commit -m "test(smoke): expect stored relation target in /memory expansion

Daemon now round-trips stored relations; B3 asserts the seeded edge target
instead of the temporary none-row placeholder.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

- [ ] **Step 4: Gate battery (all from repo root)**

```bash
SEP="==="; echo "$SEP"
# 1. Full daemon suite, unfiltered log capture
RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --no-fail-fast 2>&1 | tee "$CLAUDE_JOB_DIR/tmp/inc22-gate-daemon.log" | grep -E "^test result|running \d+ tests" ; echo "cargo done"
# 2. Vendor scan on added lines across BOTH touched trees (0 expected; || true guards zero-exit)
git diff main..HEAD -- packages/brain-shell/src/ daemon/src/ | grep '^+' | grep -icE 'claude|anthropic|vendor' || true
# 3. crates/ zero-diff gate (empty output expected)
git diff main..HEAD --stat -- crates/
echo "$SEP"
# 4. WIP preservation
git status --porcelain | wc -l
```

Expected: daemon log shows ONLY the known untracked security-audit failure identity among passing suites; vendor scan prints `0`; crates stat prints nothing; WIP count `3716`.

bun side (unchanged code, prove it anyway):

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test 2>&1 | tail -5
```

Expected: `347 pass / 5 fail` with exactly the documented five identities. tsc drift check skipped: zero TypeScript diffs in this increment (gate 3's diff scope proves it).

- [ ] **Step 5: Finish**

Announce and follow superpowers:finishing-a-development-branch on branch
`feature/brain-daemon-memory-relations-roundtrip` (base `main`). Normal repo.
Local merge MUST use the working-tree-safe FF recipe: verify
`merge-base main HEAD == rev-parse main`, then `git fetch . <branch>:main`,
then `git checkout main` (identical trees ⇒ zero writes), spot-check, delete branch.

---

## Plan self-review record

- Spec coverage: §4.1 helpers (T1), §4.2 wiring incl. UUID gate + borrow order (T2), §5.1 unit tests (T1), §5.2 integration suite w/ tracked-file-only rule (T2), §5.3 smoke flip (T3), §6 gates incl. RUSTFLAGS/vendor/crates-zero/WIP (T3). §5.4 bun wire test untouched — proven green by gate.
- Placeholder scan: none — all code literal; smoke expectations pinned.
- Type consistency: helper signatures match between T1 Interfaces and T2 call sites; `rpc()` returns parsed BODY (envelope unwrapped) matching assertion shapes; `NodeId(u)` tuple-constructor pattern mirrors brain_runtime.rs:997; `uuid` confirmed direct dep (daemon/Cargo.toml:22).
