# Increment 39 — Knowledge Graph Manual Ingestion Surface Implementation Plan

## Goal Description
Connect the existing backend `v1/memory/store` RPC and TypeScript SDK `storeMemory` method to the SessionController and `/memory add` Command Surface.

## Proposed Changes

### 1. `packages/brain-shell/src/state/sessionController.ts`
- Implement `storeMemoryNode(label: string, content: string, scope = 'workspace'): Promise<MemoryStoreOutput | undefined>`.
- Emits confirmation notice: `Stored memory concept “<label>” (<nodeId>)`.
- On error: catches exception, leaves state intact, emits error notice: `Could not store memory node: <error>`.

### 2. `packages/brain-shell/src/commands/memoryArgs.ts` & `builtin.ts` & `AppShell.tsx`
- Implement deterministic argument tokenization and parsing `parseMemoryAddArgs(args: string[])`.
- Update `/memory` command definition: `argumentHint: '<add <label> [--content <text>] | delete <id>>'`.
- Route `action === 'memory-add'` in `AppShell.tsx` to `controller.storeMemoryNode(label, content)`.

## Verification Plan

### Automated Tests
- Unit Tests: `bun test src/test/adapter/memoryIngestionLogic.test.ts` (18/18 passing).
- Full Tracked Suite: `bun test` (391/391 passing).
- Canonical Live PTY Smoke Suite: Inc 2, 21, 24, 26, 27, 28, 29, 30, 31, 32, 34, 35, 36, 37, 38, 39 (16/16 passing).
- Clean Worktree: `cargo check --workspace` in detached worktree (passing).
- WIP Preservation: 3,940 paths byte-identical.
