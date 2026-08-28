# Increment 44 — Dynamic Session System Prompt / Instructions (`/system` & `/instructions`) Design Specification

## Overview

Increment 44 closes the missing session-level system prompt / instructions steering seam:
1. **State Layer (`SessionController.ts`)**:
   - `private systemPrompt: string | null = null;`
   - `get currentSystemPrompt(): string | null`
   - `setSystemPrompt(prompt: string | null): void`:
     - If `prompt === null` or normalized to `'clear'` / `'off'` / `'reset'` -> resets `systemPrompt` to `null` and emits `Cleared custom system instructions (using default engine prompt)`.
     - If non-empty -> stores trimmed string and emits `Set system instructions: “<preview>”`.
     - Rejects whitespace-only prompts.
   - `reportSystemPromptStatus(): void`:
     - Emits current custom instructions or `Default engine prompt active (no custom instructions set)`.
   - In `submit()`:
     - Injects `systemPrompt: this.systemPrompt ?? undefined` into `BrainGenerationRequest`.
2. **Command Surface (`builtin.ts` & `AppShell.tsx`)**:
   - `/system [instructions | clear | off | reset]`
   - Top-level alias `/instructions [text | clear | off | reset]`
   - If called without arguments -> reports current prompt status via `reportSystemPromptStatus()`.
   - If called with `clear`, `off`, `reset`, `none`, `disable` -> clears instructions.
   - Otherwise sets custom prompt instructions.
3. **Backend Execution**:
   - `v1/generation/stream` in `handlers.rs` already accepts `system_prompt`, merges it with the compiled memory context, and forwards it to the provider execution.

```text
/system You are a Rust systems programmer
            │
            ▼
SessionController.setSystemPrompt()
   ├── State: systemPrompt = "You are a Rust systems programmer"
   └── Notice: Set system instructions: “You are a Rust systems programmer”
            │
            ▼
SessionController.submit()
   └── Injects systemPrompt into BrainGenerationRequest
            │
            ▼
v1/generation/stream [daemon/src/transport/uds/handlers.rs]
   ├── Merges system_prompt + context_snapshot.serialized_context
   └── Forwards combined_system_prompt into provider execution
```

---

## Architectural Invariants & Guarantees

1. **Ephemeral by Design**:
   - Session instructions are in-memory in `SessionController` for the session lifetime without requiring database migrations.
2. **Atomicity & Clean Clearing**:
   - Clearing (`null`, `'off'`, `'clear'`, `'reset'`) cleanly removes the custom prompt without leaving empty strings.
3. **Observation vs Mutation**:
   - Running `/system` without arguments reports current state without mutating anything.
4. **Failure Neutrality**:
   - Failures during generation or transport outage leave the configured system prompt untouched.

---

## Verification Matrix

1. **TypeScript Unit Tests (`systemPromptLogic.test.ts`)**:
   - Setting custom instructions and status reporting.
   - Clearing instructions via `clear`, `off`, `reset`.
   - Whitespace-only rejection.
   - Inclusion in `BrainGenerationRequest` inside `submit()`.
   - Omission from `BrainGenerationRequest` when unset or cleared.
   - Command parsing for `/system` and `/instructions`.
2. **Canonical Live PTY Smoke Test (`scripts/ptySmokeInc44.py`)**:
   - Check initial status with `/system`.
   - Set custom prompt `/system You are an expert AI assistant focused on brevity`.
   - Verify status with `/instructions`.
   - Submit turn under custom instructions -> verify prompt was sent and response generated.
   - Clear prompt with `/system off`.
   - Verify cleared status.
3. **Full 21-Suite Regression Matrix**:
   - All 21 canonical PTY suites green.
   - Full 427+ Bun test suite green.
   - 74 Rust domain/storage tests green.
   - Clean detached worktree `cargo check --workspace`.
   - 3,938 WIP paths preserved byte-for-byte.
