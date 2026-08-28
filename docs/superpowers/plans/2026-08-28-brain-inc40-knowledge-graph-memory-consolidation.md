# Increment 40 — Knowledge Graph Memory Consolidation Surface Implementation Plan

## Goal Description
Connect the existing backend `v1/memory/consolidate` RPC and TypeScript SDK `triggerConsolidation` method to the SessionController and `/memory consolidate` Command Surface.

## Proposed Changes

### 1. `packages/brain-shell/src/state/sessionController.ts`
- Implement `consolidateMemories(options?): Promise<ConsolidationResult | undefined>`.
- Emits confirmation notice:
  - When `actionsApplied > 0`: `Consolidated memory graph: <N> actions applied (<P> promoted, <M> merged, <A> archived, <Pr> pruned) in <D>ms`
  - When `actionsApplied === 0`: `Memory graph is fully consolidated: 0 actions needed in <D>ms`
  - When `errors.length > 0`: `Memory consolidation completed with errors: <errors>`
- On RPC failure: catches exception, leaves state intact, emits error notice: `Could not consolidate memory graph: <error>`.

### 2. `packages/brain-shell/src/commands/builtin.ts` & `AppShell.tsx`
- Update `/memory` command:
  - `argumentHint: '<add <label> [--content <text>] | delete <id> | consolidate>'`
  - Routes `consolidate` subcommand -> action `memory-consolidate`.
- Register `/consolidate` top-level command -> action `memory-consolidate`.
- Route `action === 'memory-consolidate'` in `AppShell.tsx` to `controller.consolidateMemories()`.

## Verification Plan

### Automated Tests
- Unit Tests: `bun test src/test/adapter/memoryConsolidationLogic.test.ts` (6/6 passing).
- Full Tracked Suite: `bun test` (397/397 passing).
- Canonical Live PTY Smoke Suite: Inc 2 through 40 (17/17 passing).
- Clean Worktree: `cargo check --workspace` in detached worktree (passing).
- WIP Preservation: 3,940 paths byte-identical.
