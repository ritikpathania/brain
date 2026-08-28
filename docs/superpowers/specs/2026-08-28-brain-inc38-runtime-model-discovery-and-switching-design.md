# Increment 38 — Runtime Model Discovery & Switching Design Specification

## Overview

Increment 38 connects the Brain runtime model discovery and switching capabilities from the backend `ModelGateway` through to the Shell Controller, Command Surface, and UI Presentation (`StatusBarView` and `ModelPickerView`).

Prior to Increment 38:
- Backend `ModelGateway` in `crates/brain-core` provides `list_models()` and `resolve_model(query)`.
- UDS daemon handlers `v1/model/list` and `v1/model/resolve` return registered `ModelDescriptorDto` records (`id`, `name`, `provider`, `context_window`, `max_output_tokens`, `supports_thinking`, `supports_tools`, `is_default`).
- `UdsBrainBackendClient.ts` implements `listModels()` and `resolveModel(query)`.
- Generation endpoint `v1/generation/stream` accepts optional `model` in payload and routes to requested model.
- **However**, `SessionController` does not track `activeModel` or forward `model` in `BrainGenerationRequest`.
- `StatusBarView.tsx` reads from a static mock hook `useMainLoopModel()`, hardcoded to `'brain-default'`.
- There is no `/model` slash command or interactive model picker overlay in the shell.

Increment 38 establishes runtime model discovery, interactive model selection, direct alias resolution, and reactive status bar display without inventing a duplicate model registry in the shell.

---

## Architectural Invariants

1. **Authoritative Model Resolution (`ModelGateway` is Source of Truth)**:
   - The shell does NOT persist or maintain its own model catalog.
   - All model discovery routes to `client.listModels()`.
   - All model alias / query resolution routes to `client.resolveModel(query)`.

2. **Model Identity Separation**:
   - **User Query / Alias**: What the user types (e.g. `/model sonnet`, `/model gpt4`, `/model default`).
   - **Resolved Model Identity (`id`)**: The canonical ID string sent in `BrainGenerationRequest.model` (e.g. `claude-3-5-sonnet-20241022`).
   - **Display Identity (`name` + `provider`)**: The human-readable name formatted for presentation in `StatusBarView` and `ModelPickerView` (e.g. `claude-3-5-sonnet (anthropic)`).

3. **Atomic Mutation & Failure Safety**:
   - When resolving `/model <query>`:
     - Success -> `SessionController.activeModel` is updated to the resolved descriptor, `StatusBarView` updates reactively, and a confirmation notice is emitted (`Switched active model to “<name>” (<id>)`).
     - Error / Unknown -> `activeModel` remains untouched, and an error notice is emitted (`Could not resolve model: <error>`).

4. **Generation Request Injection**:
   - `SessionController.submit(text)` includes `model: this.activeModel?.id` in `BrainGenerationRequest`.
   - When `activeModel` is undefined or default, the daemon routes to its configured default provider.

---

## Component Boundaries & Changes

### 1. `BrainBackendClient.ts` / `UdsBrainBackendClient.ts`
- Ensure `BrainModelDescriptor` type is complete and exported.
- Verified: `listModels()` and `resolveModel()` are already implemented in `UdsBrainBackendClient.ts`.

### 2. `SessionController.ts`
Add model tracking and switching methods:
- Property: `private activeModelDescriptor?: BrainModelDescriptor`
- Getter: `get activeModel(): BrainModelDescriptor | undefined`
- Getter: `get activeModelName(): string` (returns `activeModelDescriptor?.name ?? 'brain-default'`)
- Method: `listAvailableModels(): Promise<BrainModelDescriptor[]>`
- Method: `switchModel(query?: string): Promise<BrainModelDescriptor | undefined>`
- Update `submit(text)`: forward `model: this.activeModelDescriptor?.id` in `BrainGenerationRequest`.

### 3. Model Picker Overlay (`modelPickerLogic.ts` & `ModelPickerView.tsx`)
- Pure logic `modelPickerLogic.ts`:
  - `toModelItemVM(descriptor, currentModelId)`
  - `modelListDecision(action, selected, count)`
- Component `ModelPickerView.tsx`:
  - Uses `ModalFrame` (title: "Select Active Model", subtitle: "Choose default generation model from registered providers").
  - Renders list of available models with:
    - Active marker (`●`)
    - Default badge (`[default]`)
    - Capability flags (`[thinking]`, `[tools]`)
    - Context window and max output tokens
    - Provider label
  - Navigation: `↑↓ navigate · enter switch model · esc close`.

### 4. Command Surface & Keybinding Routing (`builtin.ts` & `AppShell.tsx`)
- Register `/model` in `builtin.ts`:
  - `argumentHint: '[name_or_alias]'`
  - `run: (ctx) => ctx.args.length > 0 ? { type: 'action', action: 'model-switch', args: ctx.args } : { type: 'overlay', overlay: 'model' }`
  - Alias `/models` -> `{ type: 'overlay', overlay: 'model' }`.
- Update `AppShell.tsx`:
  - State: `modelOpen`, `modelList`, `modelSelected`, `modelLoading`.
  - Replace static `useMainLoopModel()` in `StatusBarView` with reactive `snapshot.activeModelName` (or `controller.activeModelName`).
  - Wire `model-switch` action -> `void controller.switchModel(args.join(' '))`.
  - Wire `modelOpen` overlay with `useBoundInput` -> committing calls `void controller.switchModel(chosen.id)`.

---

## Verification Matrix

1. **Unit Tests**:
   - `modelSwitchingLogic.test.ts`: Test `switchModel` resolution, active model getter, error fallback, and request payload forwarding.
   - `modelPickerLogic.test.ts`: Test VM normalization, selection clamping, and keyboard decision reducer.
   - `modelPickerView.test.tsx`: Test rendering of active marker, badges, context limits, and loading/empty states.
2. **Tracked Bun Suite**:
   - Run all 358+ tests to ensure 100% green.
3. **Canonical Live PTY Smoke Test (`ptySmokeInc38.py`)**:
   - Sequence:
     1. Boot shell -> verify status bar shows default model.
     2. Open `/model` overlay -> verify registered models listed with capability badges.
     3. Navigate and select a model with `Enter` -> verify active model switches and notice is emitted.
     4. Verify status bar reactively displays the new model name.
     5. Use direct `/model <alias>` switch -> verify instant switch and confirmation.
     6. Submit user prompt -> verify generation completes successfully with the selected model.
4. **All Prior PTY Regressions**:
   - Inc 2, 21, 24, 26, 27, 28, 29, 30, 31, 32, 34, 35, 36, 37 (all 14 suites green).
5. **Clean Worktree Check**:
   - Detached worktree `cargo check --workspace` passing with 0 errors.
6. **WIP Preservation**:
   - 3,939 paths byte-identical.
