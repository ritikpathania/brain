# Increment 36 — Context Provenance & Retrieval Observability Implementation Plan

## Goal Description
Connect the context telemetry pipeline from the Rust daemon generation stream through to the shell's view models, transcript rows, and interactive `/context` overlay.

## Proposed Changes

### 1. `packages/brain-shell` Transport & Adapter Pipeline
- Define `ContextTelemetry` in `BrainBackendClient.ts`.
- Map `metadata.telemetry` into `turn_complete` in `chunkToTurnEvents.ts`.
- Reduce `telemetry` in `BrainTurnTransformer.ts`.
- Extend `TranscriptRow` with `ContextTelemetryData` and add `kind: 'context'` in `messages.ts`.
- Project context rows in `toRows.ts` and render `ContextRowView` in `MessageRow.tsx`.
- Track `latestContextTelemetry` on `SessionController`.
- Implement `ContextOverlayView.tsx` with `ModalFrame`.
- Register `/context` in `builtin.ts` and wire `AppShell.tsx`.
- Yield finished chunks properly in `UdsBrainBackendClient.ts`.

### 2. `daemon`
- Support extracting `last_user_prompt` from block arrays as well as strings in `handlers.rs`.

## Verification Plan

### Automated Tests
- Unit & Wire Tests: `bun test src/test/adapter/contextProvenanceWire.test.ts src/test/ui/overlays/contextOverlayView.test.tsx` (8/8 passing).
- Full Tracked Suite: `bun test` (342/342 passing).
- Canonical PTY Smoke Suite: Inc 2, 21, 24, 26, 27, 28, 29, 30, 31, 32, 34, 35, 36 (13/13 passing).
- Clean Worktree: `cargo check --workspace` in detached worktree (passing).
