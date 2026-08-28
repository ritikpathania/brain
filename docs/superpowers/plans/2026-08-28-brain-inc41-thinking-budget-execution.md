# Increment 41 — Backend Thinking Budget Plumbing + `/thinking` Shell Control Implementation Plan

## Goal Description
Connect LLM reasoning thinking budget configuration from the `/thinking` command surface down to provider execution in the Rust daemon.

## Proposed Changes

### 1. `crates/brain-services/src/model/mock.rs` & `daemon/src/transport/uds/handlers.rs`
- Add `last_request_thinking_budget` recording to `DeterministicMockProvider`.
- Parse `thinkingConfig` (`mode` and `budgetTokens`) in `v1/generation/stream` in `handlers.rs`.
- Forward `thinking_budget: Option<usize>` directly into `brain_core::model::GenerationRequest`.
- Include `thinking_budget` in `stream_start` metadata packet.

### 2. `packages/brain-shell/src/state/sessionController.ts`
- Implement `setThinkingBudget(budget: number | null): boolean`.
- Implement `reportThinkingStatus(): void`.
- Update `ShellSnapshot` with `thinkingBudget?: number | null`.
- Pass `thinkingConfig` in `BrainGenerationRequest` during `submit()`.

### 3. `packages/brain-shell/src/commands/builtin.ts` & `AppShell.tsx`
- Register `/thinking` command (`aliases: ['think']`, `argumentHint: '[tokens | off]'`).
- Route actions `thinking-set` and `thinking-status` in `AppShell.tsx`.

## Verification Plan

### Automated Tests
- Unit Tests: `bun test src/test/adapter/thinkingConfigLogic.test.ts` (10/10 passing).
- Full Tracked Suite: `bun test` (407/407 passing).
- Canonical Live PTY Smoke Suite: Inc 2 through 41 (18/18 passing).
- Clean Worktree: `cargo check --workspace` in detached worktree (passing).
- WIP Preservation: 3,939 paths byte-identical.
