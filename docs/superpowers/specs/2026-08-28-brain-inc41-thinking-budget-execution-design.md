# Increment 41 — Backend Thinking Budget Plumbing + `/thinking` Shell Control Design Specification

## Overview

Increment 41 connects LLM reasoning / thinking token budget configuration end-to-end:
1. **Backend Engine / Daemon**: Parses `thinkingConfig` in `v1/generation/stream` and passes `thinking_budget: Option<usize>` directly into `brain_core::model::GenerationRequest` and downstream model provider execution.
2. **Provider Observability**: Records `last_request_thinking_budget` in `DeterministicMockProvider` for direct structural assertion.
3. **SessionController State**: Manages `thinkingBudget: number | null`, enforces positive integer validation, updates reactive snapshots, and sends `thinkingConfig` with generation requests.
4. **Shell Command Surface**: Provides `/thinking [budget | off]` (and alias `/think`) to configure, disable, or inspect the active reasoning token budget.

```text
/thinking [budget | off]
        │
        ▼
SessionController.setThinkingBudget(...) / reportThinkingStatus()
        │
        ▼
BrainGenerationRequest.thinkingConfig { mode, budgetTokens }
        │
        ▼
v1/generation/stream [handlers.rs]
        │
        ▼
brain_core::model::GenerationRequest { thinking_budget: Some(tokens) }
        │
        ▼
ModelProvider.stream_generation(...)
```

---

## Architectural Invariants & Guarantees

1. **Truthful Provider Execution**:
   - The shell never claims a thinking budget is active unless the daemon handler parses and forwards `thinking_budget` directly into the provider's `GenerationRequest`.
2. **Atomic Configuration**:
   - Setting an invalid budget (non-integer, negative, NaN, empty) emits an error notice and leaves the previous configuration untouched.
3. **Effective State Reporting**:
   - Bare `/thinking` command reports the effective active budget (`Current reasoning thinking budget: X tokens` or `Reasoning thinking budget is disabled (default)`).
4. **Zero Domain Schema Changes**:
   - Thinking budget is an interactive turn/session generation parameter, cleanly utilizing existing `BrainThinkingConfig` and `brain_core::model::GenerationRequest.thinking_budget`.

---

## Component Boundaries & Changes

### 1. `daemon/src/transport/uds/handlers.rs`
- Define `ThinkingConfigPayload`:
  ```rust
  #[derive(serde::Deserialize)]
  struct ThinkingConfigPayload {
      #[serde(default)]
      mode: Option<String>,
      #[serde(rename = "budgetTokens", alias = "budget_tokens", default)]
      budget_tokens: Option<usize>,
  }
  ```
- In `v1/generation/stream`:
  - Deserialize `thinking_config: Option<ThinkingConfigPayload>`.
  - Compute `thinking_budget: Option<usize>`.
  - Pass `thinking_budget` into `brain_core::model::GenerationRequest`.
  - Include `thinking_budget` in `start_packet` metadata.

### 2. `crates/brain-services/src/model/mock.rs`
- Add `last_request_thinking_budget: Arc<Mutex<Option<usize>>>` to `DeterministicMockProvider`.
- Record `request.thinking_budget` in `stream_generation`.
- Expose `pub fn last_request_thinking_budget(&self) -> Option<usize>`.

### 3. `packages/brain-shell/src/state/sessionController.ts`
- Add `private thinkingBudget: number | null = null;`.
- Add `thinkingBudget?: number | null;` to `ShellSnapshot`.
- Implement `setThinkingBudget(budget: number | null): boolean`.
- Implement `reportThinkingStatus(): void`.
- Include `thinkingConfig` in `this.client.streamText(...)` request payload.

### 4. `packages/brain-shell/src/commands/builtin.ts` & `AppShell.tsx`
- Register `/thinking` command (`aliases: ['think']`, `argumentHint: '[tokens | off]'`).
- Route actions:
  - `thinking-set` -> `controller.setThinkingBudget(arg === 'off' ? null : Number(arg))`.
  - `thinking-status` -> `controller.reportThinkingStatus()`.

---

## Verification Matrix

1. **Rust Unit Tests (`crates/brain-services/tests/model_gateway_tests.rs`)**:
   - Assert `thinking_budget: Some(4096)` in `GenerationRequest` is captured by provider.
2. **TypeScript Unit Tests (`packages/brain-shell/src/test/adapter/thinkingConfigLogic.test.ts`)**:
   - `setThinkingBudget` sets positive integer budget and emits confirmation notice.
   - `setThinkingBudget(null)` disables budget and emits notice.
   - Validation: non-integer / negative / zero values emit usage notice and do not mutate state.
   - `submit()` sends formatted `thinkingConfig` in generation request payload.
   - `/thinking` and `/think` command parsing and routing.
3. **Canonical Live PTY Smoke Test (`scripts/ptySmokeInc41.py`)**:
   - Launch shell against live daemon.
   - Run `/thinking` -> verify disabled default notice.
   - Run `/thinking 4096` -> verify confirmation notice.
   - Run `/think 8192` -> verify updated confirmation notice.
   - Submit prompt `Explain reasoning` -> verify streaming response completes.
   - Run `/thinking off` -> verify disabled notice.
4. **Full Regression Matrix**:
   - All 18 canonical PTY smoke tests green.
   - 397+ Bun tests green.
   - Rust domain/storage tests green.
   - Clean detached worktree `cargo check --workspace`.
   - 3,939 WIP paths preserved byte-for-byte.
