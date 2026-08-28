# Increment 40 — Knowledge Graph Memory Consolidation Surface Design Specification

## Overview

Increment 40 exposes the knowledge graph memory consolidation and hygiene capability (`v1/memory/consolidate`) from SQLite storage through to the SessionController and `/memory consolidate` Command Surface.

Prior to Increment 40:
- `crates/brain-storage` implements ACID-transactional `consolidate_memories(policy)` with semantic promotion, duplicate node merging, edge archival, and edge pruning.
- Daemon exposes `v1/memory/consolidate` with single-flight concurrency serialization and returns structured metrics (`actions_applied`, `promoted`, `merged`, `archived`, `pruned`, `duration_ms`, `errors`).
- `UdsBrainBackendClient.ts` implements `triggerConsolidation(options?): Promise<ConsolidationResult>`.
- `SessionController` has `storeMemoryNode` (Inc 39), `deleteMemoryNode` (Inc 31), and `searchMemories` (Inc 27), but lacks `consolidateMemories`.
- `/memory` command supports `add` and `delete`, but has no `consolidate` subcommand.

Increment 40 completes the knowledge graph lifecycle (`create → search → retrieve / context-augment → consolidate → delete`).

---

## Architectural Invariants

1. **Authoritative Backend Execution**:
   - The shell does NOT perform local graph consolidation or heuristic merging.
   - All consolidation requests route through `client.triggerConsolidation()`.
   - Action counts and execution metrics come directly from `ConsolidationResult`.

2. **Truthful Presentation**:
   - The notice text is derived strictly from the authoritative `ConsolidationResult`:
     - If `actionsApplied > 0`: `Consolidated memory graph: <N> actions applied (<P> promoted, <M> merged, <A> archived, <Pr> pruned) in <D>ms`
     - If `actionsApplied === 0`: `Memory graph is fully consolidated: 0 actions needed in <D>ms`
   - Errors returned in `result.errors` or caught via RPC rejection are surfaced clearly in error notices.

3. **Atomic Failure Safety**:
   - A failed `triggerConsolidation` RPC emits an error notice and leaves local controller state untouched.

4. **Non-Interactive Deterministic Flow**:
   - Command `/memory consolidate` (and alias `/consolidate`) executes non-interactively without prompting for confirmation.

5. **Zero Backend / Protocol Changes**:
   - Reuses existing `v1/memory/consolidate` endpoint and wire contracts.

---

## Component Boundaries & Changes

### 1. `packages/brain-shell/src/state/sessionController.ts`
- Add `consolidateMemories(options?: { promotionWeightThreshold?: number; pruningWeightThreshold?: number; stalenessAgeThresholdSecs?: number }): Promise<ConsolidationResult | undefined>`:
  - Calls `await this.client.triggerConsolidation(options)`.
  - Formats summary notice using `actionsApplied`, `promoted`, `merged`, `archived`, `pruned`, and `durationMs`.
  - On error: emits `Could not consolidate memory graph: <error>` and returns `undefined`.

### 2. `packages/brain-shell/src/commands/builtin.ts`
- Update `/memory` command definition:
  - `argumentHint: '<add <label> [--content <text>] | delete <id> | consolidate>'`
  - Routes `consolidate` subcommand -> returns `{ type: 'action', action: 'memory-consolidate' }`.
- Register `/consolidate` top-level alias:
  - `description: 'Consolidate, merge duplicates, and prune stale edges in the knowledge graph'`
  - `run: () => ({ type: 'action', action: 'memory-consolidate' })`.

### 3. `packages/brain-shell/src/ui/shell/AppShell.tsx`
- In `runCommand`:
  - Route `action === 'memory-consolidate'`: calls `void controller.consolidateMemories()`.

---

## Verification Matrix

1. **Unit Tests (`memoryConsolidationLogic.test.ts`)**:
   - `SessionController.consolidateMemories`:
     - Successful execution with 0 actions -> emits `Memory graph is fully consolidated: 0 actions needed in <D>ms`.
     - Successful execution with >0 actions -> emits `Consolidated memory graph: <N> actions applied (<P> promoted, <M> merged, <A> archived, <Pr> pruned) in <D>ms`.
     - RPC failure -> emits `Could not consolidate memory graph: <error>`.
   - Command routing:
     - `/memory consolidate` returns action `memory-consolidate`.
     - `/consolidate` returns action `memory-consolidate`.
2. **Tracked Bun Test Suite**:
   - 391+ tests green.
3. **Canonical Live PTY Smoke Test (`ptySmokeInc40.py`)**:
   - Sequence:
     1. Start live daemon and launch shell.
     2. Ingest concept node via `/memory add`.
     3. Run `/memory consolidate`.
     4. Verify structured consolidation notice with actions count and duration.
     5. Run `/consolidate` alias -> verify second consolidation notice.
     6. Verify search and generation with context remain fully operational.
4. **All 16 Prior PTY Regressions**:
   - Incs 2, 21, 24, 26, 27, 28, 29, 30, 31, 32, 34, 35, 36, 37, 38, 39.
5. **Clean Worktree Check**:
   - Detached worktree `cargo check --workspace` passing.
6. **WIP Baseline Preservation**:
   - 3,940 paths byte-identical.
