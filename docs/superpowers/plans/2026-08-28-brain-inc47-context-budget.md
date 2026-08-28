# Increment 47 — Retrieval Context Budget Steerability (`/context budget`) Implementation Plan

## Proposed Changes

### 1. Session Controller (`packages/brain-shell/src/state/sessionController.ts`)
- Add `contextMaxTokens: number | null = null`.
- Add `currentContextMaxTokens` getter.
- Implement `setContextBudget(budget: number | null): boolean` with strict validation.
- Implement `reportContextBudgetStatus(): void`.
- Update `formatSessionInfoSummary` to show configured vs engine default context budget.
- In `submit()`, pass `max_tokens: this.contextMaxTokens ?? 2048` to `this.client.retrieveContext`.

### 2. Builtin Commands & Dispatch (`builtin.ts` & `AppShell.tsx`)
- In `packages/brain-shell/src/commands/builtin.ts`:
  - Enhance `/context` command with `budget` subcommand and `/context budget [tokens | default]`.
- In `packages/brain-shell/src/ui/shell/AppShell.tsx`:
  - Handle `context-budget-set` and `context-budget-status` actions.

### 3. Tests & Verification
- Unit test: `packages/brain-shell/src/test/adapter/contextBudgetLogic.test.ts`
- Live PTY test: `scripts/ptySmokeInc47.py`
- Canonical matrix: Incs 2 through 47 (24 suites).
- Rust tests & clean worktree check.
- WIP preservation.
