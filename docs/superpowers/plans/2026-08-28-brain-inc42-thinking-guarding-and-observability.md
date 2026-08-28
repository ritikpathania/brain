# Increment 42 — Model Reasoning Guarding & Thinking Status Observability Plan

## Goal Description
Enforce authoritative model reasoning capability guarding in the daemon and expose live thinking status observability in the shell status bar.

## Proposed Changes

### 1. `daemon/src/transport/uds/handlers.rs`
- Check `resolved_model_desc.supports_thinking`.
- Compute `effective_thinking_budget = if resolved_model_desc.supports_thinking { thinking_budget.map(|b| b.min(resolved_model_desc.max_output_tokens)) } else { None }`.
- Pass `effective_thinking_budget` into `brain_core::model::GenerationRequest`.
- Include `thinking_supported` and `effective_thinking_budget` in `stream_start` metadata.

### 2. `packages/brain-shell/src/ui/shell/StatusBar.tsx` & `AppShell.tsx`
- Add `thinkingBudget` and `supportsThinking` props to `StatusBar`.
- Render `think:4k` / `think:off` / `think:n/a`.
- Connect snapshot state to `StatusBar` in `AppShell.tsx`.

## Verification Plan
- Unit tests: `bun test src/test/ui/shell/statusBarThinkingLogic.test.ts`
- Live PTY test: `scripts/ptySmokeInc42.py`
- Canonical matrix: Incs 2 through 42 (19 suites).
- Rust tests & clean worktree `cargo check --workspace`.
- 3,938 WIP paths preserved byte-for-byte.
