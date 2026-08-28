# Increment 43 — Knowledge Graph Relational Authoring (`/memory link` & `/memory unlink`) Plan

## Goal Description
Connect explicit knowledge graph edge creation and deletion from the `/memory link` and `/memory unlink` command surface down to `EdgeRepository` persistence in SQLite.

## Proposed Changes

### 1. `daemon/src/server/protocol.rs` & `daemon/src/transport/uds/handlers.rs`
- Add `LinkMemoryPayload`, `LinkMemoryResponseBody`, `UnlinkMemoryPayload`, `UnlinkMemoryResponseBody`.
- Implement `v1/memory/link` and `v1/memory/unlink` handlers.

### 2. `packages/brain-shell/src/client/BrainBackendClient.ts`
- Add `linkMemory` and `unlinkMemory` SDK methods.

### 3. `packages/brain-shell/src/state/sessionController.ts`
- Implement `linkMemories` and `unlinkMemories` with deterministic node resolution.

### 4. `packages/brain-shell/src/commands/builtin.ts` & `AppShell.tsx`
- Add `/memory link`, `/memory unlink`, `/link`, `/unlink` commands.

## Verification Plan
- Unit tests: `bun test src/test/adapter/memoryLinkLogic.test.ts`
- Live PTY test: `scripts/ptySmokeInc43.py`
- Canonical matrix: Incs 2 through 43 (20 suites).
- Rust domain/storage tests & clean detached worktree check.
- 3,938 WIP paths preserved.
