# Increment 45 — Authoritative Daemon Diagnostics Telemetry (`v1/system/status` & `/doctor`) Implementation Plan

## Proposed Changes

### 1. Daemon Protocol & Handlers (`protocol.rs` & `handlers.rs`)
- In `daemon/src/server/protocol.rs`:
  - Define `SystemStatusResponseBody`.
- In `daemon/src/transport/uds/handlers.rs`:
  - Implement `v1/system/status` and `system/status` UDS actions.
  - Query storage for total sessions, nodes, edges.
  - Query model gateway for active models.
  - Calculate uptime from process start.

### 2. TypeScript SDK Client (`BrainBackendClient.ts` & `UdsBrainBackendClient.ts`)
- In `packages/brain-shell/src/client/BrainBackendClient.ts`:
  - Add `SystemStatusOutput` interface and `getSystemStatus()` to `BrainBackend`.
- In `packages/brain-shell/src/client/UdsBrainBackendClient.ts`:
  - Implement `getSystemStatus()`.

### 3. Doctor Probe & UI (`doctorProbe.ts`, `DoctorOverlayView.tsx`, `AppShell.tsx`)
- In `packages/brain-shell/src/adapter/doctorProbe.ts`:
  - Enrich probe logic using authoritative `client.getSystemStatus()`.
- In `packages/brain-shell/src/ui/overlays/DoctorOverlayView.tsx`:
  - Render authoritative daemon metrics.
- In `packages/brain-shell/src/ui/shell/AppShell.tsx`:
  - Pass backend client to `DoctorProbe`.

### 4. Tests & Verification
- Unit test: `packages/brain-shell/src/test/adapter/daemonStatusLogic.test.ts`
- Live PTY test: `scripts/ptySmokeInc45.py`
- Canonical matrix: Incs 2 through 45 (22 suites).
- Rust tests & clean worktree check.
- WIP preservation.
