# Increment 46 — Active Session Composite Info Inspector (`/session info`) Implementation Plan

## Proposed Changes

### 1. Session Controller (`packages/brain-shell/src/state/sessionController.ts`)
- Implement `reportSessionInfo(): void` assembling composite session runtime metadata into a structured notice.
- Include helper `formatSessionInfoSummary()` for clean testability.

### 2. Builtin Commands & Dispatch (`builtin.ts` & `AppShell.tsx`)
- In `packages/brain-shell/src/commands/builtin.ts`:
  - Register `/session info`, `/session stats`, `/session status`.
- In `packages/brain-shell/src/ui/shell/AppShell.tsx`:
  - Route `session-info` action to `controller.reportSessionInfo()`.

### 3. Tests & Verification
- Unit test: `packages/brain-shell/src/test/adapter/sessionInfoLogic.test.ts`
- Live PTY test: `scripts/ptySmokeInc46.py`
- Canonical matrix: Incs 2 through 46 (23 suites).
- Rust tests & clean worktree check.
- WIP preservation.
