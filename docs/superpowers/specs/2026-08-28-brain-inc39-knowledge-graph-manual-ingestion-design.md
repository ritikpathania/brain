# Increment 39 — Knowledge Graph Manual Ingestion Surface Design Specification

## Overview

Increment 39 exposes the manual knowledge graph node ingestion capability from the backend storage engine through to the Shell Controller and `/memory add` Command Surface.

Prior to Increment 39:
- SQLite and domain repositories support storing concept nodes (`id`, `label`, `kind: Concept`, `properties: { content, scope, relations }`).
- Daemon exposes `v1/memory/store` in `daemon/src/transport/uds/handlers.rs`.
- `UdsBrainBackendClient.ts` implements `storeMemory(input: MemoryStoreInput): Promise<MemoryStoreOutput>`.
- `DefaultContextOrchestrator` retrieves stored nodes in `/memory search` (Inc 27) and during context compilation for generation prompts (Inc 30).
- `SessionController` has `deleteMemoryNode` (Inc 31) and `searchMemories` (Inc 27), but lacks `storeMemoryNode`.
- `/memory` command supports `delete <id>` and opening `MemoryOverlayView`, but has no `add` subcommand.

Increment 39 closes this final seam in the memory lifecycle (`create → search → retrieve → delete`), allowing users to ingest new concepts directly from the shell.

---

## Architectural Invariants

1. **Authoritative Backend Storage**:
   - The shell does NOT persist or maintain its own memory database.
   - All memory node creations route through `client.storeMemory({ label, content, scope, sessionId })`.
   - Node ID generation remains authoritative in the backend.

2. **Immediate Visibility & Context Participation**:
   - Ingested concept nodes must immediately be searchable via `/memory` overlay.
   - Ingested concept nodes must participate in automated prompt context augmentation when matching query keywords according to standard BM25 ranking rules.

3. **Atomic Failure Safety**:
   - A failed `storeMemory` RPC emits a clear system notice and does not corrupt or mutate local controller state.

4. **Preserve Existing Memory Surface**:
   - `/memory` (no args) continues to open `MemoryOverlayView`.
   - `/memory delete <node_id>` continues to delete nodes.
   - Zero modifications to backend storage schema or daemon wire protocols.

---

## Component Boundaries & Changes

### 1. `packages/brain-shell/src/state/sessionController.ts`
- Add `storeMemoryNode(label: string, content: string, scope?: string): Promise<MemoryStoreOutput | undefined>`:
  - Validates `label` and `content`.
  - Calls `client.storeMemory({ label, content, scope: scope ?? 'workspace', sessionId: this.sessionId })`.
  - Emits confirmation notice: `Stored memory concept “<label>” (<node_id>)`.
  - On error: emits notice `Could not store memory node: <error>` and returns `undefined`.

### 2. `packages/brain-shell/src/commands/builtin.ts`
- Expand `/memory` command definition:
  - `argumentHint: '<add <label> [--content <text>] | delete <id>>'`
  - Subcommands:
    - `add`: parses label and content, dispatches `{ type: 'action', action: 'memory-add', args: [...] }`.
    - `delete`: dispatches `{ type: 'action', action: 'memory-delete', args: [targetId] }`.
    - (default): dispatches `{ type: 'overlay', overlay: 'memory' }`.

### 3. `packages/brain-shell/src/ui/shell/AppShell.tsx`
- In `runCommand`:
  - Route `action === 'memory-add'`: parses args and calls `void controller.storeMemoryNode(label, content)`.

---

## Verification Matrix

1. **Unit Tests**:
   - `memoryIngestionLogic.test.ts`:
     - Test `storeMemoryNode` validation (empty label, empty content).
     - Test successful storage and notice emission.
     - Test error handling and notice emission.
     - Test command argument parsing (`/memory add <label> --content <text>`, `/memory add <label> <content>`, `/memory add <label>`).
2. **Tracked Bun Suite**:
   - Run full 373+ test suite (100% green).
3. **Canonical Live PTY Smoke Test (`ptySmokeInc39.py`)**:
   - Sequence:
     1. Start live daemon and launch `brain-shell`.
     2. Ingest concept via `/memory add Quantum Teleportation --content Entangled photon pairs enable quantum state transfer across optical channels`.
     3. Verify confirmation notice with created node ID.
     4. Open `/memory` overlay -> verify `Quantum Teleportation` is listed.
     5. Submit prompt: `How does quantum teleportation work?` -> verify context telemetry reflects retrieved memory and response streams.
     6. Delete node via `/memory delete <node_id>` -> verify deletion confirmation.
     7. Open `/memory` overlay -> verify node is purged.
4. **All 15 Prior PTY Regressions**:
   - Inc 2, 21, 24, 26, 27, 28, 29, 30, 31, 32, 34, 35, 36, 37, 38.
5. **Clean Worktree Check**:
   - Detached worktree `cargo check --workspace` passing.
6. **WIP Baseline Preservation**:
   - 3,940 paths byte-identical.
