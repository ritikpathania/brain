# Brain Daemon Memory Relations Round-Trip Fix — Design Spec

**Date:** 2026-08-26
**Status:** Approved recon → spec for review
**Base:** main @ `b0e80bd2`
**Evidence:** full recon matrix delivered 2026-08-26 (repro harness, layer-isolated proof)

## 1. Problem

`v1/memory/store` persists relation objects, but `v1/memory/search` returns
`relations: []` for every memory. Proven layer-by-layer on a throwaway daemon:

| Layer | Site | Behavior |
|---|---|---|
| Store wire | `daemon/src/server/protocol.rs:199-208` | `relations: Option<Vec<Value>>` received |
| Handler persist | `daemon/src/transport/uds/handlers.rs:1427-1429` | written into node props as native JSON **Array** |
| SQLite | `crates/brain-storage/src/store.rs:1058-1062` | durable in `nodes.properties` blob (repro read it back verbatim) |
| Retrieval mapping | `crates/brain-application/src/application.rs:270,275,296-306` | **ROOT CAUSE** — metadata built only from the two-variant `SearchMetadata` enum (`Session{archived,pinned}` / `Message{session_id,role}`); node properties structurally unreachable; also hardcodes `body = node.label` and discards projector scores/edges |
| Search decode | `daemon/src/transport/uds/handlers.rs:1351-1355` | expects metadata value as **String-encoded JSON**, absent → silent `[]`; latent second mismatch |
| Wire DTO | `daemon/src/server/protocol.rs:186-187` | `relations: Vec<Value>` + `#[serde(default)]` — already array-capable |
| Client | `packages/brain-shell/src/client/UdsBrainBackendClient.ts:829` | faithful passthrough (`m.relations ?? []`) |

Collateral from the same drop: search `excerpt == label` (body hardcoded), and
`score` is always the default 100 (fallback scores discarded at
`application.rs:270`). Relations are additionally never materialized as graph
`Edge` rows (`edges` count = 0 after store) — recorded as out of scope (§7).

## 2. Goal

A stored memory's relations round-trip end-to-end: what `memory/store`
accepts, `memory/search` returns — plus truthful `excerpt` and `scope`.
Internal compatibility fix only; **zero IPC/schema/domain changes**.

## 3. Non-goals

- No change to `MemoryItemDto`, `StoreMemoryPayload`, or any DTO (wire contract already correct).
- No client/UI changes (`UdsBrainBackendClient`, `MemoryOverlayView` already handle populated arrays).
- No Edge materialization / dangling-target semantics (`target_id` values like `"beta-1"` are not parseable `NodeId`s — separate future increment).
- No score passthrough (would require widening the `SearchMetadata` enum — domain change; default 100 stays).
- No empty-query listing semantics (two independent early-return guards at `search_projection.rs:233-235` and `memory_list_projection.rs:87-92` are deliberate; separate product decision).

## 4. Design

### 4.1 New pure module: `daemon/src/transport/uds/memory_relations.rs`

Unit-testable helpers over `serde_json` values; no I/O, no async:

```rust
/// Resolve relations from a node's property map, tolerating both native
/// JSON arrays and legacy string-encoded JSON. Falls back to the summary
/// metadata string when the node carries nothing.
pub fn extract_relations(
    props: Option<&serde_json::Map<String, serde_json::Value>>,
    metadata_fallback: Option<&str>,
) -> Vec<serde_json::Value>

/// Prefer the stored `content` property; fall back to whatever body the
/// pipeline produced (today: the label).
pub fn preferred_excerpt(
    props: Option<&serde_json::Map<String, serde_json::Value>>,
    fallback_body: &str,
) -> String

/// Resolve scope from properties, falling back to `"workspace"`.
pub fn preferred_scope(
    props: Option<&serde_json::Map<String, serde_json::Value>>,
    fallback: &str,
) -> String
```

Decode order in `extract_relations`:
1. `props["relations"]` as `Value::Array` → `from_value::<Vec<Value>>`
2. `props["relations"]` as `Value::String` → `from_str::<Vec<Value>>` (legacy tolerance)
3. `metadata_fallback` string → `from_str::<Vec<Value>>` (any future producer)
4. `vec![]`

Non-array garbage at any step falls through to the next step (never errors).

### 4.2 Wiring: `daemon/src/transport/uds/handlers.rs` memory/search block

Inside the per-summary loop (~1351-1367): attempt a node lookup keyed by
`summary.id`, enrich when — and only when — the id resolves to a stored node.
Sessions/messages (ids like `session:…`) fail UUID parse or miss lookup and
keep today's behavior exactly.

```rust
// Enrich from the stored node when the summary resolves to one: the
// application-layer projection drops node properties (SearchMetadata enum),
// so relations/excerpt/scope are recovered here at the boundary.
let node_props = uuid::Uuid::parse_str(&summary.id)
    .ok()
    .and_then(|u| {
        use brain_core::repositories::NodeRepository;
        let nid = brain_domain::NodeId(u);
        storage.find_by_id(&nid).ok().flatten().map(|n| n.properties)
    });
let relations = memory_relations::extract_relations(
    node_props.as_ref(),
    summary.metadata.get("relations").map(|s| s.as_str()),
);
let excerpt = memory_relations::preferred_excerpt(node_props.as_ref(), &summary.body);
let scope = memory_relations::preferred_scope(node_props.as_ref(), "workspace");
```

(`storage` = `app.runtime().sqlite_storage()`, same accessor the store handler
uses at handlers.rs:1419; UUID-parse→`NodeId(u)`→
`NodeRepository::find_by_id` mirrors `inspect_node` at
`brain_runtime.rs:993-1004`, confirming both APIs exist.)
The existing metadata-string decode lines 1351-1355 fold into step 3 above.

### 4.3 Unchanged by design

- `memory/store` handler: keeps writing native Array props (correct encoding).
- Client, overlay, DTOs, domain crates: zero diffs.

## 5. Testing strategy

1. **Rust unit tests** (in `memory_relations.rs` `#[cfg(test)]`): array form,
   string-encoded form, missing prop with metadata fallback, missing both →
   empty, non-array garbage → empty, excerpt prefers content over label,
   scope fallback.
2. **New tracked integration suite** `daemon/tests/uds_memory_relations_tests.rs`,
   following the tracked sibling harness pattern
   (`uds_session_autotitle_tests.rs`: tmp dir + `CARGO_BIN_EXE_brain-daemon`
   + env-var socket/pid/db paths + `Drop` kill/cleanup):
   - store WITH relations → search by label token → assert
     `memories[0].relations` deep-equals the stored array AND
     `memories[0].excerpt == content` AND count ≥ 1 (replaces the vacuous
     `is_array()` style assertion).
   - store WITHOUT relations → `relations == []`, success.
   - Assertions live ONLY in this new tracked file. The untracked
     user-WIP suites (`integration_uds_session.rs`,
     `uds_memory_retrieval_tests.rs`, `uds_security_audit_tests.rs`) must not
     be touched, staged, or extended.
3. **PTY smoke flip**: `scripts/ptySmokeInc21.py` flow B3 expects the relation
   target label `"Beta Concept"` in the expanded detail pane instead of
   `(No outgoing relations)`; docstring note updated. This is the end-to-end proof.
4. **bun side**: `test/client/memorySearchWire.test.ts` already proves client
   passthrough of populated arrays; must stay green unchanged.

## 6. Verification gates & repo constraints

- Build/test: every cargo call wrapped:
  `RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --no-fail-fast`
  with UNFILTERED log capture (grep-filtered pipelines mask exit codes).
  Known pre-existing failure identity: untracked security-audit test mismatch — not attributable.
- bun shell suite: failure identities must remain exactly the documented five.
- Vendor gate extended to Rust scope: added-lines scan across BOTH
  `packages/brain-shell/src/` and `daemon/src/` must return 0 hits for
  `claude|anthropic|vendor`.
- Zero-diff gates re-run: `crates/` untouched by this fix (handler-level only);
  verify with `git diff <base>..HEAD --stat -- crates/`.
- Commits: explicit-path `git add <paths>` only; trailer
  `Co-Authored-By: Claude <noreply@anthropic.com>`. Before staging any shared
  file (esp. `handlers.rs`), run `git diff HEAD -- <file>` — if foreign WIP
  hunks appear, use the hunk-filter/update-index recipe (repo memory #12);
  never `git add .`, never stash. Working tree ~3.7k WIP paths preserved byte-for-byte.
- Pushes to origin require explicit user approval each time.

## 7. Out-of-scope ledger (recorded, not forgotten)

- Edge materialization for stored relations incl. dangling-target policy.
- Real relevance scores through the retrieval pipeline (needs `SearchMetadata` widening).
- Empty-query listing semantics for `/memory`.
- Pre-existing: standalone-build debt (untracked `brain-core` modules), session-summary enrichment, fixture hygiene.

## 8. Risks

| Risk | Mitigation |
|---|---|
| N+1 point lookups per search hit | bounded by `limit ≤ 20`; single-digit-ms SQLite primary-key selects |
| Nodes produced by other flows lack `content`/`relations` props | every helper falls through to current behavior; sessions/messages skip enrichment entirely |
| `summary.id` formats vary across producers | UUID-parse gate: anything non-node-shaped keeps today's output |
| Shared-file WIP sweep on commit | mandatory pre-stage diff check + hunk filter recipe |
