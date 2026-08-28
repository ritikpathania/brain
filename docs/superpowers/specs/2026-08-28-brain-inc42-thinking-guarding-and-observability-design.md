# Increment 42 — Model Reasoning Guarding & Thinking Status Observability Design Specification

## Overview

Increment 42 establishes authoritative model reasoning capability guarding and reactive UI observability:
1. **Backend Engine / Daemon Authoritative Clamping**:
   - `daemon/src/transport/uds/handlers.rs` checks `resolved_model_desc.supports_thinking`.
   - If `supports_thinking == false`: forces `thinking_budget = None` and reports `"thinking_supported": false, "effective_thinking_budget": null` in `stream_start` metadata.
   - If `supports_thinking == true`: clamps `thinking_budget` to `resolved_model_desc.max_output_tokens` (preventing budget overflows) and reports `"thinking_supported": true, "effective_thinking_budget": thinking_budget`.
2. **SessionController & Warnings**:
   - When setting `/thinking <budget>` or switching to a model with `supportsThinking === false`, provides truthful context warning notices if a budget is requested for a model that cannot honor it.
3. **Status Bar Reactive Presentation**:
   - `StatusBarView` and `AppShell` display the live thinking state:
     - Non-thinking model: `think:n/a`
     - Thinking model with active budget (e.g. 4096): `think:4k` (or `think:8k`)
     - Thinking model with disabled budget: `think:off`

```text
configured budget (e.g. 8192)
            │
            ▼
v1/generation/stream
            │
            ▼
Authoritative Model Resolution
  ├── supports_thinking == false ──► effective_budget = None (thinking_supported: false)
  └── supports_thinking == true  ──► effective_budget = min(budget, max_output_tokens)
            │
            ▼
GenerationRequest { thinking_budget: effective_budget }
            │
            ▼
stream_start metadata { thinking_supported, effective_thinking_budget, thinking_budget }
            │
            ▼
StatusBarView: think:4k | think:off | think:n/a
```

---

## Architectural Invariants & Guarantees

1. **Authoritative Execution Truthfulness**:
   - The effective execution budget is decided exclusively at the daemon boundary by resolving `supports_thinking` and `max_output_tokens`.
2. **No Silent Misleading Claims**:
   - If a model cannot reason, no thinking budget is dispatched to the provider.
3. **Non-Mutating Failure Safety**:
   - Invalid model selections or clamping operations do not corrupt user preferences.
4. **Zero Domain Schema Changes**:
   - Pure runtime capability guarding and UI presentation.

---

## Verification Matrix

1. **Daemon & Rust Tests (`crates/brain-services/tests/model_gateway_tests.rs`)**:
   - Non-thinking model (`qwen2.5-coder:32b`) strips thinking budget to `None`.
   - Thinking-capable model (`brain-default`) preserves requested budget.
   - Oversized budget is clamped to `max_output_tokens`.
2. **TypeScript Unit Tests (`packages/brain-shell/src/test/ui/shell/statusBarThinkingLogic.test.ts`)**:
   - Status bar token helper formats `think:4k`, `think:off`, `think:n/a`.
   - Reactive snapshot reflects active model capabilities and budget.
3. **Canonical Live PTY Smoke Test (`scripts/ptySmokeInc42.py`)**:
   - Launch shell against live daemon.
   - Verify default status bar shows `think:off`.
   - Set budget `/thinking 4096` -> verify status bar shows `think:4k`.
   - Switch to non-thinking model `/model qwen2.5-coder:32b` -> verify status bar shows `think:n/a`.
   - Submit generation -> verify stream succeeds with `effective_thinking_budget: null`.
   - Switch back to `brain-default` -> verify status bar restores `think:4k`.
4. **Full 19-Suite Regression Matrix**:
   - All 19 PTY suites green.
   - Full 407+ Bun test suite green.
   - 74 Rust domain/storage tests green.
   - Clean worktree `cargo check --workspace`.
   - 3,938 WIP paths preserved.
