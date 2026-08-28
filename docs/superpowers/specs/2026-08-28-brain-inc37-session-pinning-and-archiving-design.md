# Increment 37 — Session Pinning & Archiving Lifecycle Design Specification

## Overview

Increment 37 completes the session lifecycle by exposing session pinning, unpinning, archiving, and restoring capabilities across the Shell Controller, SDK, and TUI Command Surface.

Prior to Increment 37:
- SQLite storage schema (`sessions.pinned`, `sessions.archived`), Domain aggregates (`Session::pin`, `Session::archive`), and UDS Daemon handlers (`v1/session/pin`, `v1/session/archive`, `v1/session/restore`) already exist.
- `UdsBrainBackendClient.ts` already implements `pinSession`, `archiveSession`, `restoreSession`.
- `ResumePickerView.tsx` already displays `★` for pinned sessions and filters out archived sessions.
- **However**, `SessionController.ts` lacks pinning and archiving methods, and the shell command surface only accepts `/session delete`. Once archived, there is no command to restore a session or toggle archived visibility.

Increment 37 exposes the complete pinning/archiving lifecycle through deterministic target resolution, active-session safety, and discovery.

---

## Architectural Invariants

1. **Deterministic Target Resolution**:
   - **Omitted Target (`/session pin`, `/session unpin`, `/session archive`)**: Targets the **active session** if one exists. If no session is active, reports `No active session to <action>. Specify a session ID or prefix.`
   - **Explicit Target (`<id_or_prefix>`)**:
     1. Exact session ID match -> execute mutation immediately.
     2. Unique prefix match (>= 4 chars or unique across catalog) -> execute mutation.
     3. Ambiguous prefix -> report matching candidates (`Ambiguous session prefix “...” matches: id1, id2`).
     4. No match -> report notice (`No session found matching “...”.`).
   - **Restore Target (`/session restore <id_or_prefix>`)**: Requires an explicit target ID/prefix (since an archived session cannot be the active session).

2. **Active-Session Safety & State Reset**:
   - When the **active session** is archived via `/session archive [id_or_prefix]`:
     - The active session is archived in storage via `v1/session/archive`.
     - `SessionController.sessionId` is reset to `undefined` (just like session deletion).
     - `latestTelemetry` is reset to `undefined`.
     - A system notice is emitted: `Archived active session “<title>” (<id_prefix>)`.
   - When an **inactive session** is archived:
     - Active session remains intact; system notice emitted: `Archived session “<title>” (<id_prefix>)`.
   - When a session is **pinned / unpinned**:
     - Pinned status updated in storage via `v1/session/pin`.
     - System notice emitted: `Pinned session “<title>” (<id_prefix>)` or `Unpinned session “<title>” (<id_prefix>)`.
   - When a session is **restored**:
     - Restored in storage via `v1/session/restore`.
     - System notice emitted: `Restored session “<title>” (<id_prefix>)`.

3. **Archived Session Discovery Mechanism**:
   - Standard `/resume` filters out archived sessions by default to keep the active picker clean.
   - `/resume all` or `/resume --all` opens the `ResumePickerView` including archived sessions, tagged with `[archived]`.
   - Cross-session transcript search (`/search <query>`) continues to search across all sessions (active and archived), and matches can be restored via `/session restore <id_or_prefix>`.

---

## Component Boundaries & Changes

### 1. `SessionController.ts`
Add methods:
- `pinSession(target?: string, pinned = true): Promise<boolean>`
- `archiveSession(target?: string): Promise<boolean>`
- `restoreSession(target: string): Promise<boolean>`
- Update `listSessions(includeArchived = false)` to support fetching all sessions.

### 2. `commands/builtin.ts` & `AppShell.tsx`
Register subcommands under `/session`:
- `/session pin [id_or_prefix]`
- `/session unpin [id_or_prefix]`
- `/session archive [id_or_prefix]`
- `/session restore <id_or_prefix>`
- Top-level aliases: `/pin`, `/unpin`, `/archive`, `/restore`.
- Extend `/resume [all]` to support opening picker with archived items.

### 3. `resumePickerLogic.ts` & `ResumePicker.tsx`
- Support `includeArchived` option in `resumeChoices`.
- When an archived session is rendered in `ResumePickerView`, display `[archived]` tag alongside title.

---

## Verification Matrix

1. **Unit Tests**:
   - `sessionLifecycleMutations.test.ts`: Test `pinSession`, `archiveSession`, `restoreSession`, active-session reset, and prefix resolution.
   - `resumePickerLogic.test.ts`: Test `resumeChoices` with `includeArchived` flag.
2. **Tracked Bun Suite**:
   - Run all 342+ tests to ensure 100% green.
3. **Canonical Live PTY Smoke Test (`ptySmokeInc37.py`)**:
   - Sequence:
     1. Create active session.
     2. `/session pin` -> verify confirmation.
     3. Check `/resume` -> verify `★` badge on pinned item.
     4. `/session unpin` -> verify unpinned confirmation.
     5. `/session archive` -> verify active session reset.
     6. Check `/resume` -> verify archived session hidden.
     7. `/resume all` -> verify archived session visible with `[archived]`.
     8. `/session restore <prefix>` -> verify restored notice.
     9. Check `/resume` -> verify session restored and visible in standard list.
4. **All Prior PTY Regressions**:
   - Inc 2, 21, 24, 26, 27, 28, 29, 30, 31, 32, 34, 35, 36 (all green).
5. **Clean Worktree Check**:
   - Detached worktree `cargo check --workspace` passing.
6. **WIP Preservation**:
   - 3,939 paths byte-identical.
