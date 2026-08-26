# Blank-Query Memory Listing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a blank `/memory` query list the stored Concept nodes (newest first, honoring `limit`) instead of returning a false-empty page.

**Architecture:** Internal handler-level branch in the daemon's `memory/search` UDS action: when the trimmed query is empty, list `Concept` nodes directly through `NodeRepository::list_all` and enrich them with the Inc 22 recovery helpers; otherwise run the existing retrieval pipeline byte-identically. Wire DTO, client, shell, overlay: untouched.

**Tech Stack:** Rust daemon (tokio UDS transport, brain-core repositories, brain-domain entities), Python PTY smoke harness, Bun/Ink shell (regression-only).

**Spec:** `docs/superpowers/specs/2026-08-26-brain-daemon-memory-blank-query-listing-design.md`

## Global Constraints

- Every cargo invocation wrapped: `RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks"`; package flag is `-p brain-daemon` (never `-p daemon`).
- Cargo test logs captured UNFILTERED to `$CLAUDE_JOB_DIR/tmp/*.log` and read from the file — pipe tails mask exit codes.
- Vendor scan on ADDED lines of every commit: `git diff main...HEAD -- daemon/src packages/brain-shell/src | grep '^+' | grep -in 'claude\|anthropic'` → must print nothing.
- Zero-diff gates vs `main`: `crates/**`, `packages/brain-shell/src/**`, `daemon/src/server/protocol.rs`.
- Working tree carries ~3.7k dirty user-WIP paths: NEVER `git add .`, NEVER `git stash`, NEVER revert. Stage only explicit paths listed per task. Before staging a shared tracked file, run `git diff <file>` and confirm every hunk is ours (hunk-filter via `update-index` recipe if foreign WIP appears mid-file).
- Commit trailer on every commit: `Co-Authored-By: Claude <noreply@anthropic.com>`.
- Work IN PLACE off branch `feature/brain-daemon-memory-blank-query-listing` created from `main` @ `1fe58dff`. No worktrees. No pushes without explicit user approval.
- Known pre-existing failure identities allowed in batteries: daemon security-audit suite mismatch (untracked test), bun suite's five documented failure identities. Nothing else.

---

### Task 1: Handler branch — blank queries list stored concepts

**Files:**
- Modify: `daemon/tests/uds_memory_relations_tests.rs` (append two tests)
- Modify: `daemon/src/transport/uds/handlers.rs:1322-1392` (insert branch, wrap typed path in `else`)

**Interfaces:**
- Consumes: `super::memory_relations::{extract_relations, preferred_excerpt, preferred_scope}` — exact signatures `extract_relations(Option<&HashMap<String,Value>>, Option<&str>) -> Vec<Value>`, `preferred_excerpt(Option<&HashMap<String,Value>>, &str) -> String`, `preferred_scope(Option<&HashMap<String,Value>>, &str) -> String`; trait `brain_core::repositories::NodeRepository::list_all(&self) -> Result<Vec<brain_domain::Node>, BrainError>`; `brain_domain::Node` fields `id: NodeId, label: String, node_type: NodeType /* alias of NodeKind */, properties: HashMap<String, Value>, updated_at: u64`.
- Produces: wire behavior — `{"query": "", "limit": N}` to `memory/search`/`v1/memory/search` returns `memories[]` of stored Concepts, newest-first (tie-break label asc), each with recovered relations/excerpt/scope; typed queries unchanged.

- [ ] **Step 1: Create the branch**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git checkout -b feature/brain-daemon-memory-blank-query-listing main
```

Expected: `Switched to a new branch …` tracking `1fe58dff`; working-tree WIP count unchanged (~3721).

- [ ] **Step 2: Write the failing wire-level tests**

Append to `daemon/tests/uds_memory_relations_tests.rs` (verbatim harness helpers `get_temp_dir`/`start_daemon_at`/`rpc` already exist in the file):

```rust

#[tokio::test]
async fn blank_query_lists_stored_concepts_newest_first() {
    let d = start_daemon_at(get_temp_dir()).await;

    rpc(
        &d.socket_path,
        1,
        "memory/store",
        serde_json::json!({
            "label": "Older Plain Node",
            "content": "older body",
        }),
    )
    .await;

    rpc(
        &d.socket_path,
        2,
        "memory/store",
        serde_json::json!({
            "label": "Newer Related Node",
            "content": "newer body",
            "scope": "compiler",
            "relations": [{"relation": "supports", "target_id": "beta-1"}],
        }),
    )
    .await;

    let found = rpc(
        &d.socket_path,
        3,
        "memory/search",
        serde_json::json!({"query": "", "limit": 10}),
    )
    .await;
    let m = found["memories"].as_array().expect("memories array");
    assert_eq!(m.len(), 2, "both stored concepts listed: {found}");
    assert_eq!(m[0]["label"], "Newer Related Node", "newest first");
    assert_eq!(m[0]["excerpt"], "newer body");
    assert_eq!(m[0]["scope"], "compiler");
    assert_eq!(m[0]["relations"][0]["relation"], "supports");
    assert_eq!(m[1]["label"], "Older Plain Node");
    assert_eq!(m[1]["relations"].as_array().unwrap().len(), 0);

    let ws = rpc(
        &d.socket_path,
        4,
        "memory/search",
        serde_json::json!({"query": "   ", "limit": 10}),
    )
    .await;
    assert_eq!(
        ws["memories"].as_array().unwrap().len(),
        2,
        "whitespace-only query behaves as blank"
    );
}

#[tokio::test]
async fn blank_query_honors_limit_and_typed_queries_unchanged() {
    let d = start_daemon_at(get_temp_dir()).await;

    for i in 1..=3 {
        rpc(
            &d.socket_path,
            i,
            "memory/store",
            serde_json::json!({"label": format!("Node{i}"), "content": "c"}),
        )
        .await;
    }

    let limited = rpc(
        &d.socket_path,
        10,
        "memory/search",
        serde_json::json!({"query": "", "limit": 2}),
    )
    .await;
    assert_eq!(
        limited["memories"].as_array().unwrap().len(),
        2,
        "blank listing honors limit: {limited}"
    );

    let typed = rpc(
        &d.socket_path,
        11,
        "memory/search",
        serde_json::json!({"query": "Node2", "limit": 10}),
    )
    .await;
    let tm = typed["memories"].as_array().unwrap();
    assert!(
        tm.iter().any(|x| x["label"] == "Node2"),
        "single-token typed query still finds the stored node: {typed}"
    );
}
```

(Note: labels are single tokens — `"Node2"` — because the typed path's fallback projector tokenizes on whitespace; multi-word probes would be ambiguous.)

- [ ] **Step 3: Run the suite to verify the new tests FAIL**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" \
  cargo test -p brain-daemon --test uds_memory_relations_tests \
  > "$CLAUDE_JOB_DIR/tmp/inc23-red.log" 2>&1; echo "rc=$?"
grep -E "^test |panicked at|assertion|left:|right:" "$CLAUDE_JOB_DIR/tmp/inc23-red.log"
```

Expected rc nonzero; exactly two failures:
- `blank_query_lists_stored_concepts_newest_first` — panicked at `assert_eq!(m.len(), 2, …)` with left `0` right `2`
- `blank_query_honors_limit_and_typed_queries_unchanged` — panicked at the limit assert, left `0` right `2`

The two pre-existing tests (`stored_relations_round_trip_through_search`, `store_without_relations_yields_empty_relation_list`) must PASS.

- [ ] **Step 4: Implement the handler branch**

In `daemon/src/transport/uds/handlers.rs`, the `memory/search` block runs `:1306`–`:1452` (response assembly follows at `:1393`). Replace the ENTIRE span from the line after `let storage = app.runtime().sqlite_storage();` through the closing brace of the `if let Ok(results)` block — i.e. current `:1322`–`:1392` — with the following. Old text (exact, current file):

```rust
            let search_query = brain_integrations::dto::v1::SearchQuery {
                text: query_text.clone(),
                kinds: None,
                pagination: None,
            };

            let now = chrono::Utc::now().timestamp_millis();
            if let Ok(results) = app.search(search_query, &context).await {
                for summary in results.into_iter().take(limit) {
                    let clean_title = if summary.title.trim().starts_with('{') {
                        if let Ok(v) =
                            serde_json::from_str::<serde_json::Value>(summary.title.trim())
                        {
                            v.get("content")
                                .and_then(|c| c.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or(summary.title.clone())
                        } else {
                            summary.title.clone()
                        }
                    } else {
                        summary.title.clone()
                    };

                    let score = summary
                        .metadata
                        .get("score")
                        .and_then(|s| s.parse::<i64>().ok())
                        .unwrap_or(100);

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
                }
            }
```

New text (branch + typed path re-indented one level inside `else`; semantics of the else arm byte-equivalent to the old text):

```rust
            if query_text.trim().is_empty() {
                // Blank queries mean "show me what's there": the retrieval
                // pipeline short-circuits blanks, so list stored concepts
                // directly (newest first) instead of returning a misleading
                // empty page.
                let mut nodes =
                    brain_core::repositories::NodeRepository::list_all(storage.as_ref())
                        .unwrap_or_default();
                nodes.retain(|n| matches!(n.node_type, brain_domain::NodeKind::Concept));
                nodes.sort_by(|a, b| {
                    b.updated_at
                        .cmp(&a.updated_at)
                        .then_with(|| a.label.cmp(&b.label))
                });
                for node in nodes.into_iter().take(limit) {
                    // Hoist helper results BEFORE moving node fields into the
                    // DTO — same borrow-order discipline as the typed arm.
                    let props = node.properties;
                    let excerpt =
                        super::memory_relations::preferred_excerpt(Some(&props), &node.label);
                    let scope =
                        super::memory_relations::preferred_scope(Some(&props), "workspace");
                    let relations =
                        super::memory_relations::extract_relations(Some(&props), None);
                    matches.push(crate::server::protocol::MemoryItemDto {
                        node_id: node.id.to_string(),
                        label: node.label,
                        excerpt,
                        score: 100,
                        channel: "knowledge_graph".to_string(),
                        timestamp: (node.updated_at * 1000) as i64,
                        scope,
                        relations,
                    });
                }
            } else {
                let search_query = brain_integrations::dto::v1::SearchQuery {
                    text: query_text.clone(),
                    kinds: None,
                    pagination: None,
                };

                let now = chrono::Utc::now().timestamp_millis();
                if let Ok(results) = app.search(search_query, &context).await {
                    for summary in results.into_iter().take(limit) {
                        let clean_title = if summary.title.trim().starts_with('{') {
                            if let Ok(v) =
                                serde_json::from_str::<serde_json::Value>(summary.title.trim())
                            {
                                v.get("content")
                                    .and_then(|c| c.as_str())
                                    .map(|s| s.to_string())
                                    .unwrap_or(summary.title.clone())
                            } else {
                                summary.title.clone()
                            }
                        } else {
                            summary.title.clone()
                        };

                        let score = summary
                            .metadata
                            .get("score")
                            .and_then(|s| s.parse::<i64>().ok())
                            .unwrap_or(100);

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
                        let scope = super::memory_relations::preferred_scope(
                            node_props.as_ref(),
                            "workspace",
                        );

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
                    }
                }
            }
```

Everything below (`let serialized_context = matches …` onward) stays untouched — it serves both arms.

If the live file's exact bytes differ from the old text above (e.g. whitespace drift), re-read `handlers.rs:1317-1400` and adapt the anchors — the semantic contract is: branch on `query_text.trim().is_empty()` immediately after the `storage` binding; the entire existing retrieval-and-map flow becomes the `else` body unchanged apart from indentation.

- [ ] **Step 5: Run the suite to verify GREEN**

Same command as Step 3 (log to `inc23-green.log`). Expected: all four tests in `uds_memory_relations_tests` pass, rc=0.

- [ ] **Step 6: Full daemon battery**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" \
  cargo test -p brain-daemon --no-fail-fast \
  > "$CLAUDE_JOB_DIR/tmp/inc23-battery.log" 2>&1; echo "rc=$?"
grep -E "^test result:" "$CLAUDE_JOB_DIR/tmp/inc23-battery.log"
grep -E "^failures:$|^    \w+" "$CLAUDE_JOB_DIR/tmp/inc23-battery.log" | sort -u | head -20
```

Expected: every suite passes EXCEPT the documented pre-existing security-audit identity. Any other failure → stop, investigate (systematic-debugging), no fix-forward.

- [ ] **Step 7: Vendor scan + zero-diff gates**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git diff main -- daemon/src | grep '^+' | grep -in 'claude\|anthropic'; echo "vendor-scan rc=$?"
git diff main --stat -- crates/ packages/brain-shell/src daemon/src/server/protocol.rs | wc -c
git status --porcelain -uall | wc -l
```

Expected: vendor scan prints NOTHING (grep rc=1); zero-diff stat byte count `0`; WIP count ≈ 3721.

- [ ] **Step 8: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git diff daemon/src/transport/uds/handlers.rs | head -80   # confirm only our hunks
git add daemon/src/transport/uds/handlers.rs daemon/tests/uds_memory_relations_tests.rs
git commit -m "feat(daemon): blank memory queries list stored concepts newest-first

Blank trimmed queries short-circuit to zero hits at the FTS sanitizer and
the fallback projector, so /memory's opening fetch rendered a false 'No
concepts recorded yet' state. Branch at the handler boundary: empty query
lists Concept nodes via NodeRepository::list_all (recency desc, label asc,
honoring limit), enriched through the memory_relations recovery helpers;
the typed pipeline moves verbatim into the else arm. Wire DTO, client,
and shell untouched.

Co-Authored-By: Claude <noreply@anthropic.com>"
git show --stat HEAD | head -8
```

Expected: exactly 2 files changed.

---

### Task 2: PTY smoke proves the initial listing; regression gates

**Files:**
- Modify: `scripts/ptySmokeInc21.py` (docstring + comment + new `B0` check)

**Interfaces:**
- Consumes: Task 1's daemon behavior (blank query returns seeded concept) — smoke runs the debug binary rebuilt in Task 1.
- Produces: behavioral proof that opening `/memory` lists stored concepts before any keystroke; full regression evidence bundle.

- [ ] **Step 1: Flip the smoke to assert the initial listing**

Edit `scripts/ptySmokeInc21.py` — three precise replacements.

Replacement 1 — docstring flow B (old):

```python
  B. Seed one memory via RPC (with a relation), type /memory -> modal opens,
     type "cortex" to filter -> seeded node lists, enter expands the detail
     pane showing the stored relation target ("Beta Concept"), esc closes
     with the system notice.
```

new:

```python
  B. Seed one memory via RPC (with a relation), type /memory -> modal opens
     AND the seeded node lists immediately (blank queries list stored
     concepts), typing "cortex" keeps it filtered, enter expands the detail
     pane showing the stored relation target ("Beta Concept"), esc closes
     with the system notice.
```

Replacement 2 — pre-flow-B comment (old):

```python
# ── Flow B: /memory ───────────────────────────────────────────────────────
# The overlay's initial empty-query fetch is deliberately not asserted:
# server-side behavior for query:'' is unspecified. Instead we type a token
# the sole seeded node contains ("cortex") — the private tmp DB guarantees
# it ranks first — and prove listing + expansion behaviorally.
run_slash("memory")
check("B1 memory modal opens", wait_for("Relational Knowledge & Memory"))
```

new:

```python
# ── Flow B: /memory ───────────────────────────────────────────────────────
# The overlay opens with an empty query; the daemon lists stored concepts
# for blank queries, so the sole seeded node must appear BEFORE any
# keystrokes. We then prove filtering ("cortex") and expansion behaviorally.
run_slash("memory")
check("B1 memory modal opens", wait_for("Relational Knowledge & Memory"))
check("B0 initial listing shows the seeded concept", wait_for(LABEL))
```

- [ ] **Step 2: Rebuild the debug binary and run the smoke**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" \
  cargo build -p brain-daemon > "$CLAUDE_JOB_DIR/tmp/inc23-build.log" 2>&1; echo "build rc=$?"
python3 scripts/ptySmokeInc21.py
```

Expected: build rc=0; smoke prints PASS for `boot banner`, `A1-A4`, `B1`, `B0 initial listing shows the seeded concept`, `B2`, `B3 expand renders the stored relation target`, `B4` — `FAILURES: 0`, exit 0.

- [ ] **Step 3: Regression smokes and shell identities**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
python3 scripts/ptySmokeInc2.py > "$CLAUDE_JOB_DIR/tmp/inc23-smoke2.log" 2>&1; echo "rc=$?"; tail -3 "$CLAUDE_JOB_DIR/tmp/inc23-smoke2.log"
cd packages/brain-shell && bun test > "$CLAUDE_JOB_DIR/tmp/inc23-bun.log" 2>&1; echo "rc=$?"
grep -E "^test result|failures" "$CLAUDE_JOB_DIR/tmp/inc23-bun.log" | head; cd /Users/ritikpathania/Developer/PyCharm/brain
```

Expected: `ptySmokeInc2.py` 12/12 PASS; bun suite shows ONLY the five documented failure identities (proves shell untouched — no shell file changed in this increment).

- [ ] **Step 4: Final gates + commit**

Re-run Step 7's vendor-scan/zero-diff/WIP commands (now including the smoke script in the added-line scan scope):

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git diff scripts/ptySmokeInc21.py | head -40   # confirm only our three hunks
git add scripts/ptySmokeInc21.py
git commit -m "test(smoke): assert /memory lists seeded concepts on open

ptySmokeInc21 previously skipped the initial empty-query fetch because
blank-query behavior was unspecified. With the daemon now listing stored
concepts for blank queries, B0 asserts the seeded node renders before any
keystroke; docstring updated accordingly.

Co-Authored-By: Claude <noreply@anthropic.com>"
git log --oneline main..HEAD
```

Expected: 2 commits ahead of main (Task 1 + Task 2).

- [ ] **Step 5: Report and stop at the finishing gate**

Announce finishing-a-development-branch; present its standard menu (merge locally / push+PR / keep). Merge uses the working-tree-safe ff recipe (`git fetch . feature/…:main` then checkout) — never a working-tree-touching merge. Pushes require explicit user approval.

---

## Self-Review Record

- **Spec coverage:** spec §5.1 wire tests → Task 1 Steps 2-3; §4 design → Task 1 Step 4; §5.2 smoke flip → Task 2 Steps 1-2; §6 gates → Task 1 Steps 6-7, Task 2 Steps 3-4; §4.2 assembly untouched → stated in Task 1 Step 4. No gaps.
- **Placeholder scan:** none — every code step carries literal text; anchors carry fallback instructions tied to semantic contracts, not "fill in later".
- **Type consistency (verified against live code this session):** `Node.node_type: NodeType` where `pub type NodeType = NodeKind` (entities.rs:86,308) — so `matches!(n.node_type, brain_domain::NodeKind::Concept)` is valid; `updated_at: u64` so `(updated_at * 1000) as i64` casts correctly; `list_all` is a trait method called through the imported-path form matching the typed arm's `find_by_id` usage; helper signatures match `memory_relations.rs` exactly; DTO field set/order matches `protocol.rs::MemoryItemDto`.
- **Determinism check:** newest-first assertion survives same-second stores (label tie-break puts "Newer…" before "Older…"); typed probe uses a single-token label so the fallback projector's whitespace tokenizer can't miss it.
