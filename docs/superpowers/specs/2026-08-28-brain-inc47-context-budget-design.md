# Increment 47 — Retrieval Context Budget Steerability (`/context budget`) Design Specification

## Overview

Increment 47 exposes explicit user steering over the retrieval context token budget (`max_tokens`) within `SessionController` and routes it into `retrieveContext()` during generation turns.

```text
/context budget 4096 (or /context budget default)
        │
        ▼
SessionController.setContextBudget(tokens)
        │
        ▼
SessionController.submit()
        │
        ▼
retrieveContext({
    session_id: ...,
    query: ...,
    max_tokens: 4096 // or 2048 default
})
        │
        ▼
v1/context/retrieve (daemon compiles context within requested token budget)
```

---

## 1. Controller State & Budget Steering

### `SessionController`
- `private contextMaxTokens: number | null = null;`
- `get currentContextMaxTokens(): number | null`
- `setContextBudget(budget: number | null): boolean`
  - Validates positive non-zero finite integer.
  - If `null` (or `'default'`), resets to `null` (engine default).
  - Rejects `<= 0`, `NaN`, floats, or non-numeric strings without mutating existing state.
- `reportContextBudgetStatus(): void`
  - If `contextMaxTokens !== null`: `Current context retrieval budget: ${this.contextMaxTokens} tokens (configured).`
  - If `contextMaxTokens === null`: `Current context retrieval budget: 2048 tokens (engine default).`
- `submit()`
  - Calls `retrieveContext({ ..., max_tokens: this.contextMaxTokens ?? 2048 })`.

---

## 2. Presentation & Inspector Updates

### `formatSessionInfoSummary`
- Renders:
  - If `contextMaxTokens !== null`: `Context Budget: ${options.contextMaxTokens} tokens (configured)`
  - If `contextMaxTokens === null`: `Context Budget: 2048 tokens (engine default)`
- Renders latest telemetry if available.

---

## 3. Command Surface

### `packages/brain-shell/src/commands/builtin.ts`
- Command `/context`:
  - `/context` without arguments opens the Context overlay (`type: 'overlay', overlay: 'context'`).
  - `/context budget <tokens | default>` sets the token budget.
  - `/context budget` without argument reports the current context budget status.
- Dispatched in `AppShell.tsx`:
  - `context-budget-set`
  - `context-budget-status`

---

## 4. Verification Matrix

1. **Unit Tests (`contextBudgetLogic.test.ts`)**:
   - Positive integer accepted and configured.
   - Zero, negative, NaN, floats, and empty strings rejected without mutating state.
   - `default`, `reset`, `none` restore `null`.
   - `reportContextBudgetStatus` outputs correct status.
   - `submit()` propagates configured `max_tokens` into `retrieveContext()`.
   - `/session info` displays configured vs engine default budget.
2. **Canonical Live PTY Smoke Test (`scripts/ptySmokeInc47.py`)**:
   - Boots live daemon & shell.
   - Inspects default `/context budget`.
   - Sets `/context budget 4096`.
   - Ingests memory concepts and executes generation turn.
   - Asserts turn completes and telemetry is recorded.
   - Restores `/context budget default`.
3. **24-Suite Canonical Regression Matrix**.
4. **Clean detached worktree check & 3,938 WIP paths preserved byte-for-byte**.
