# Increment 36 — Context Provenance & Retrieval Observability Design Specification

## Overview

Increment 36 introduces context provenance tracking and live retrieval observability into the Brain CLI Shell (`brain-shell`). Prior to Increment 36, context retrieval telemetry produced by the memory orchestrator was emitted by the Rust daemon in `stream_end` frame metadata, but dropped at the transport/adapter seam before reaching presentation view models.

Increment 36 establishes strict provenance preservation across all architecture layers:
`daemon stream (stream_end metadata.telemetry) → BrainStreamChunk → chunkToTurnEvents → BrainTurnEvent (turn_complete) → BrainTurnViewModel (telemetry) → TranscriptRow / latestTelemetry → TurnSummaryCard + /context overlay`.

## Architectural Invariants

1. **Observational Truthfulness**:
   - `/context` reports the exact provenance of the generation turn that produced the response. It does NOT run a fresh retrieval or recalculate context.
2. **Explicit Absence Semantics**:
   - Populated Telemetry: Reports Epoch ID, Candidates evaluated, Memories assembled, Tokens injected, Latency, and Finish reason.
   - Zero-Memory Retrieval: Displays clear notice that retrieval evaluated candidates but 0 memories met the relevance/budget threshold for prompt assembly.
   - Missing/Unavailable Telemetry: Displays explicit unavailable message rather than manufacturing zeroes.
3. **Boundary Invariance**:
   - `ContextTelemetry` survives the entire pipeline without modification or fabrication.
4. **Active Session Seam**:
   - Changing or deleting active sessions resets `latestTelemetry` immediately to avoid cross-turn contamination.

## Component Contracts

### 1. Shell Client Interface (`BrainBackendClient.ts`)

```typescript
export interface ContextTelemetry {
  generationId?: string;
  sessionId?: string;
  retrievalEpochId?: string;
  candidatesRetrieved: number;
  memoriesAssembled: number;
  contextTokensUsed: number;
  assemblyLatencyMs?: number;
  totalDurationMs?: number;
  finishReason?: string;
}
```

### 2. Turn Events & View Models (`BrainTurnEvents.ts`, `BrainViewModels.ts`)

- `BrainTurnEvent.turn_complete` carries `telemetry?: ContextTelemetry`.
- `BrainTurnViewModel` carries `telemetry?: ContextTelemetry`.
- `BrainTurnTransformer` reduces `telemetry` from `turn_complete` without mutation.

### 3. Transcript Projection (`toRows.ts`, `MessageRow.tsx`)

- `toRows.ts` attaches `telemetry` to assistant rows and emits a `kind: 'context'` row when `memoriesAssembled > 0`.
- `MessageRow.tsx` renders `ContextRowView`:
  `✦ Context: <N> memory/memories (<tokens> tokens) · <epoch> (<latency>ms)`

### 4. Interactive Overlay (`ContextOverlayView.tsx`, `/context`)

- Registered as `/context` in `builtin.ts`.
- Opens `ModalFrame` with structured sections:
  - Retrieval Epoch & Session
  - Memory Augmentation (candidates, assembled memories, injected tokens)
  - Latency & Execution (assembly latency, turn duration, finish reason)
- Dismissed via `Enter` or `Esc`.
