# Increment 38 — Runtime Model Discovery & Switching Surface Implementation Plan

## Goal Description
Connect the runtime model discovery and switching capabilities from `ModelGateway` through to the TypeScript SDK, SessionController, and TUI presentation (`StatusBarView` and `ModelPickerView`).

## Proposed Changes

### 1. `packages/brain-shell/src/state/sessionController.ts`
- Add `activeModelDescriptor?: BrainModelDescriptor` to `SessionController`.
- Add `activeModelName` and `activeModel` to `SessionSnapshot`.
- Implement `listAvailableModels(): Promise<BrainModelDescriptor[]>`.
- Implement `switchModel(query?: string): Promise<BrainModelDescriptor | undefined>`.
- Forward `model: this.activeModelDescriptor?.id` in `BrainGenerationRequest`.

### 2. `packages/brain-shell/src/commands/builtin.ts` & `AppShell.tsx`
- Register `/model [name_or_alias]` and alias `/models`.
- Create `modelPickerLogic.ts` and `ModelPickerView.tsx`.
- Wire `modelOpen` overlay and `model-switch` action in `AppShell.tsx`.
- Replace static `useMainLoopModel()` in `StatusBarView` with `snapshot.activeModelName`.

## Verification Plan

### Automated Tests
- Unit Tests: `bun test src/test/adapter/modelSwitchingLogic.test.ts src/test/ui/overlays/modelPickerLogic.test.ts src/test/ui/overlays/modelPickerView.test.tsx` (15/15 passing).
- Full Tracked Suite: `bun test` (373/373 passing).
- Canonical Live PTY Smoke Suite: Inc 2, 21, 24, 26, 27, 28, 29, 30, 31, 32, 34, 35, 36, 37, 38 (15/15 passing).
- Clean Worktree: `cargo check --workspace` in detached worktree (passing).
- WIP Preservation: 3,940 paths byte-identical.
