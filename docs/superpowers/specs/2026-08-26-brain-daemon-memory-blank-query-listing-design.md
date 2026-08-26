# Brain Daemon Blank-Query Memory Listing — Design Spec

**Date:** 2026-08-26
**Status:** Approved audit recommendation → spec for review
**Base:** main @ `c0cd20be`
**Evidence:** Post-Inc 22 gap audit finding A1 (delivered 2026-08-26)

## 1. Problem

Every fresh `/memory` open misinforms the user. Chain, all verified on
current `main`:

1. The shell opens the overlay with an empty query and immediately fetches:
   `AppShell.tsx:293` resets the query to `''`; the debounced effect at
   `AppShell.tsx:207` calls `searchMemories('', 20)`.
2. The daemon short-circuits blank queries to "zero hits" at two independent
   guards: the FTS primary path (`crates/brain-storage/src/search_projection.rs:233-235`)
   and the fallback projector (`crates/brain-services/src/memory_list_projection.rs:87-92`).
3. The overlay therefore renders its ready-with-zero-rows state and paints
   **"No concepts recorded in the Brain knowledge graph yet."**
   (`packages/brain-shell/src/ui/overlays/MemoryOverlayView.tsx:34`) — even
   when the graph holds stored concepts.

The empty state is only honest when the graph truly has no concept nodes;
today it shows for every open until the user types.

## 2. Goal

A blank (or whitespace-only) query to `v1/memory/search` lists the stored
concept nodes — newest first, honoring `limit` — through the same enrichment
path Inc 22 introduced, so `/memory` opens showing the graph's actual
contents. Typed queries behave byte-for-byte as today.

## 3. Non-goals

- No change to the wire contract: same action, same `MemoryItemDto`, same
  `SearchMemoryResponseBody` shape, same client, same overlay component.
- No change to typed-query behavior, ranking, or the two blank-guards in
  `brain-storage`/`brain-services` (other callers depend on their semantics).
- No relevance scoring (audit A3 — separate domain-contract track via
  `SearchMetadata` widening).
- No pagination/cursor surface beyond the existing `limit` (workspace-scale
  graphs make `list_all` + truncate acceptable today; noted in risks).
- No Edge materialization, no consolidate surfacing (ledger items stay separate).

## 4. Design

### 4.1 Handler-level branch (internal compatibility fix)

In `daemon/src/transport/uds/handlers.rs`, inside the `memory/search` /
`v1/memory/search` block (covers both aliases): the block today runs
`:1306`–`:1390` — parse payload, bind `query_text`/`limit`
(`unwrap_or(10)` at `:1318`, unchanged)/`matches`, then retrieve and map.
The branch inserts immediately after the `let storage = …` binding
(`:1321`); the existing retrieval + mapping flow becomes the `else` arm,
verbatim. Response assembly below the block serves both arms unchanged.

```rust
if query_text.trim().is_empty() {
    // Blank queries mean "show me what's there": the retrieval pipeline
    // short-circuits blanks, so list stored concepts directly (newest
    // first) instead of returning a misleading empty page.
    let mut nodes =
        brain_core::repositories::NodeRepository::list_all(storage.as_ref())
            .unwrap_or_default();
    nodes.retain(|n| matches!(n.node_type, brain_domain::NodeKind::Concept));
    nodes.sort_by(|a, b| {
        b.updated_at.cmp(&a.updated_at).then_with(|| a.label.cmp(&b.label))
    });
    for node in nodes.into_iter().take(limit) {
        // Hoist helper results BEFORE moving node fields into the DTO —
        // same borrow-order discipline as the typed arm below.
        let props = node.properties;
        let excerpt = super::memory_relations::preferred_excerpt(Some(&props), &node.label);
        let scope = super::memory_relations::preferred_scope(Some(&props), "workspace");
        let relations = super::memory_relations::extract_relations(Some(&props), None);
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
    // …existing search_query construction, app.search, fallback projection,
    // and summary→DTO mapping, byte-identical…
}
```

Properties are already in hand per node — no per-hit lookups on this path.
Deterministic order: `updated_at` descending, tie-break label ascending
(total order, so equal-timestamp stores still sort deterministically).
`score` stays the display default (100) consistent with the typed path.
`timestamp` derives from the node's own `updated_at` (seconds → millis),
which is natural here; the typed path keeps its request-time timestamp
(audit B8 stays out of scope there).

Exact field names pinned against current code: `node.node_type`
(store arm constructs `NodeKind::Concept`), `node.updated_at`
(set via `.with_updated_at(now_secs)` in the store arm),
`NodeRepository::list_all(storage.as_ref())` (pattern at
`brain_runtime.rs:987-989`). `matches!` avoids requiring `PartialEq` on
`NodeKind`. If `updated_at`'s concrete numeric type differs at execution
time, adjust the multiply/cast once — sorting semantics unchanged.

### 4.2 Response assembly

`serialized_context`, provenance, and token-count assembly operate on
`matches` after either branch — unchanged code serves both paths. An empty
graph still yields `[]`, making the overlay's existing empty-copy honest.

### 4.3 Unchanged by design

Shell (`AppShell.tsx`, controller, client), overlay view, DTOs, storage and
services crates: zero diffs. The blank-guards remain (they protect FTS5
syntax and other pipeline consumers).

## 5. Testing strategy

1. **Extend the tracked integration suite**
   `daemon/tests/uds_memory_relations_tests.rs` with:

   ```rust
   #[tokio::test]
   async fn blank_query_lists_stored_concepts_newest_first() {
       let d = start_daemon_at(get_temp_dir()).await;

       rpc(&d.socket_path, 1, "memory/store", serde_json::json!({
           "label": "Older Plain Node",
           "content": "older body",
       })).await;

       rpc(&d.socket_path, 2, "memory/store", serde_json::json!({
           "label": "Newer Related Node",
           "content": "newer body",
           "scope": "compiler",
           "relations": [{"relation": "supports", "target_id": "beta-1"}],
       })).await;

       let found = rpc(&d.socket_path, 3, "memory/search",
                       serde_json::json!({"query": "", "limit": 10})).await;
       let m = found["memories"].as_array().expect("memories array");
       assert_eq!(m.len(), 2, "both stored concepts listed: {found}");
       assert_eq!(m[0]["label"], "Newer Related Node", "newest first");
       assert_eq!(m[0]["excerpt"], "newer body");
       assert_eq!(m[0]["scope"], "compiler");
       assert_eq!(m[0]["relations"][0]["relation"], "supports");
       assert_eq!(m[1]["label"], "Older Plain Node");
       assert_eq!(m[1]["relations"].as_array().unwrap().len(), 0);

       let ws = rpc(&d.socket_path, 4, "memory/search",
                    serde_json::json!({"query": "   ", "limit": 10})).await;
       assert_eq!(ws["memories"].as_array().unwrap().len(), 2,
                  "whitespace-only query behaves as blank");
   }

   #[tokio::test]
   async fn blank_query_honors_limit_and_typed_queries_unchanged() {
       let d = start_daemon_at(get_temp_dir()).await;
       for i in 0..3 {
           rpc(&d.socket_path, i, "memory/store",
               serde_json::json!({"label": format!("Node {i}"), "content": "c"})).await;
       }
       let limited = rpc(&d.socket_path, 10, "memory/search",
                         serde_json::json!({"query": "", "limit": 2})).await;
       assert_eq!(limited["memories"].as_array().unwrap().len(), 2);
       let typed = rpc(&d.socket_path, 11, "memory/search",
                       serde_json::json!({"query": "Node 1", "limit": 10})).await;
       let tm = typed["memories"].as_array().unwrap();
       assert!(tm.iter().any(|x| x["label"] == "Node 1"));
   }
   ```

   (Harness helpers in the sketch are verbatim from the tracked suite:
   `get_temp_dir`, `start_daemon_at`, `rpc(&d.socket_path, id: u64,
   action, body)` returning the parsed body. Note the asserted order holds
   even if both stores land in the same second: the label tie-break sorts
   "Newer Related Node" before "Older Plain Node".)
   Existing two tests must stay green untouched — they pin typed behavior.

2. **PTY smoke**: `scripts/ptySmokeInc21.py` flow B gains a pre-typing
   assertion proving the user-facing fix — after `B1 memory modal opens`,
   add `check("B0 initial listing shows the seeded concept", wait_for(LABEL))`
   before any keystrokes; remove the docstring note claiming the initial
   fetch is "deliberately not asserted". All later steps unchanged.

3. **bun side**: no changes; full suite must hold its documented five
   failure identities (proves shell untouched).

## 6. Verification gates & repo constraints

Same battery as Inc 22:

- `RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks"`
  on every cargo call; `-p brain-daemon`; full suite `--no-fail-fast` with
  UNFILTERED log capture. Expected failure identity: ONLY the known untracked
  security-audit mismatch.
- Vendor scan on added lines across `daemon/src/` (and
  `packages/brain-shell/src/` though untouched) → 0.
- Zero-diff gates: `crates/**`, all of `packages/brain-shell/src/**`,
  `daemon/src/server/protocol.rs`.
- PTY regression: `ptySmokeInc21.py` 10 checks PASS (B0 added);
  `ptySmokeInc2.py` 12/12 unaffected.
- Explicit-path commits + trailer; pre-stage `git diff HEAD -- <file>` on
  shared files (handlers.rs, smoke script) with hunk-filter recipe if foreign
  WIP appears; never stash; ~3.7k WIP paths preserved byte-for-byte.
- Pushes need explicit approval each time. Work IN PLACE, no worktrees.

## 7. Ledger impact

- Empty-query memory listing: **designed here → closes on merge.**
- Unchanged/open: edge materialization & dangling targets; real scores
  (domain track); standalone-build debt (81-error re-quantification, crates
  track); security-audit mismatch (product decision); session-summary
  enrichment split shell-half/contract-half; fixture hygiene
  (snapshot-to-tmp/deterministic); synthetic typed-path metadata (B8).

## 8. Risks

| Risk | Mitigation |
|---|---|
| `list_all` loads every node per blank query | Workspace-scale graphs; single-digit-ms SQLite scan; `take(limit)` truncates output. Pagination deferred (non-goal) |
| Non-concept node kinds leak into listing | `retain(matches!(… Concept))` filters; store only ever creates Concepts |
| Ordering ambiguity across SQLite versions | Explicit total-order sort (recency desc, label asc) — deterministic |
| `updated_at` type mismatch vs spec assumption | Pinned at execution; single cast-site adjustment |
| Behavior change surprises typed-path consumers | Branch strictly gated on `trim().is_empty()`; typed path byte-identical, pinned by existing tests |
