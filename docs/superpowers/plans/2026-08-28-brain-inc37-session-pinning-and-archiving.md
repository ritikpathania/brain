# Increment 37 — Session Pinning & Archiving Lifecycle Implementation Plan

## Goal Description
Expose session pinning, unpinning, archiving, and restoring capabilities across the Shell Controller, SDK, and TUI Command Surface, connecting existing storage schema, domain events, and daemon RPCs (`v1/session/pin`, `v1/session/archive`, `v1/session/restore`).

## Proposed Changes

### 1. `packages/brain-shell/src/state/sessionController.ts`
- Implement `pinSession(target?: string, pinned = true): Promise<boolean>`
- Implement `archiveSession(target?: string): Promise<boolean>`
- Implement `restoreSession(target: string): Promise<boolean>`

### 2. `packages/brain-shell/src/commands/builtin.ts` & `AppShell.tsx`
- Expand `/session` with `delete`, `pin`, `unpin`, `archive`, `restore` subcommands.
- Register top-level aliases: `/pin`, `/unpin`, `/archive`, `/restore`.
- Support `/resume all` and `/resume --all`.
- Route actions in `AppShell.tsx`.

### 3. `packages/brain-shell/src/ui/overlays/resumePickerLogic.ts` & `ResumePicker.tsx`
- Add `includeArchived` option to `resumeChoices` and `resolveDirectResumeTarget`.
- Attach `[archived]` badge for archived sessions in `ResumePickerView`.

## Verification Plan

### Automated Tests
- Unit Tests: `bun test src/test/adapter/sessionLifecycleMutations.test.ts src/test/ui/overlays/resumePickerLogic.test.ts src/test/ui/overlays/resumePickerView.test.tsx` (36/36 passing).
- Full Tracked Suite: `bun test` (358/358 passing).
- Canonical PTY Smoke Suite: Inc 2, 21, 24, 26, 27, 28, 29, 30, 31, 32, 34, 35, 36, 37 (14/14 passing).
- Clean Worktree: `cargo check --workspace` in detached worktree (passing).
- WIP Preservation: 3,939 paths byte-identical.
