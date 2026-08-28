# Increment 44 — Dynamic Session System Prompt / Instructions (`/system` & `/instructions`) Plan

## Goal Description
Connect custom system prompt / instructions steering through `SessionController` and `BrainGenerationRequest` to backend generation.

## Proposed Changes

### 1. `packages/brain-shell/src/state/sessionController.ts`
- Add `systemPrompt` field, `currentSystemPrompt` getter, `setSystemPrompt`, `reportSystemPromptStatus`.
- Inject `systemPrompt` into `BrainGenerationRequest` in `submit()`.

### 2. `packages/brain-shell/src/commands/builtin.ts` & `AppShell.tsx`
- Add `/system` and `/instructions` commands.
- Route `system-set` and `system-status` in `AppShell.tsx`.

## Verification Plan
- Unit tests: `bun test src/test/ui/shell/systemPromptLogic.test.ts`
- Live PTY test: `scripts/ptySmokeInc44.py`
- Canonical matrix: Incs 2 through 44 (21 suites).
- Rust domain/storage tests & clean detached worktree check.
- 3,938 WIP paths preserved.
