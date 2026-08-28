# Increment 45 — Authoritative Daemon Diagnostics Telemetry (`v1/system/status` & `/doctor`) Design Specification

## Overview

Increment 45 replaces client-side approximations in `/doctor` with an authoritative daemon health and telemetry RPC endpoint (`v1/system/status`).

```text
/doctor
   │
   ▼
DoctorOverlayView (AppShell)
   │
   ▼
DoctorProbe.runDiagnostics()
   │
   ▼
BrainBackendClient.getSystemStatus()
   │
   ▼
v1/system/status [daemon/src/transport/uds/handlers.rs]
   ├── Query SQLite connection & table metrics (sessions, nodes, edges)
   ├── Query Model Gateway registered models count
   ├── Calculate daemon uptime from process start
   └── Return SystemStatusResponseBody
           │
           ▼
DoctorOverlayView renders authoritative engine health, uptime, and database telemetry
```

---

## 1. Protocol & Daemon Wire Contract

### `daemon/src/server/protocol.rs`
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatusResponseBody {
    pub status: String, // "healthy" | "degraded" | "unhealthy"
    pub version: String,
    pub uptime_seconds: u64,
    pub sqlite_healthy: bool,
    pub total_sessions: usize,
    pub total_nodes: usize,
    pub total_edges: usize,
    pub active_models_count: usize,
}
```

### `daemon/src/transport/uds/handlers.rs`
- Handles `system/status` and `v1/system/status`.
- Inspects `app.runtime().sqlite_storage()`:
  - `total_sessions`: `SessionRepository::list_all(storage.as_ref())?.len()`
  - `total_nodes`: `NodeRepository::list_all(storage.as_ref())?.len()`
  - `total_edges`: queries connection count or edge repository.
- Inspects `app.model_gateway()`:
  - `active_models_count`: `gateway.list_models().len()`
- Calculates uptime from daemon process start time.
- If SQLite query fails, returns `status: "degraded"` with `sqlite_healthy: false`.
- Response is strictly read-only and side-effect free.

---

## 2. TypeScript SDK Client

### `packages/brain-shell/src/client/BrainBackendClient.ts`
```typescript
export interface SystemStatusOutput {
  status: 'healthy' | 'degraded' | 'unhealthy';
  version: string;
  uptimeSeconds: number;
  sqliteHealthy: boolean;
  totalSessions: number;
  totalNodes: number;
  totalEdges: number;
  activeModelsCount: number;
}
```
Added to `BrainBackend` interface and `MockBrainBackendClient`.

### `packages/brain-shell/src/client/UdsBrainBackendClient.ts`
```typescript
async getSystemStatus(): Promise<SystemStatusOutput>
```
Invokes `v1/system/status` over UDS socket.

---

## 3. Adapter & View Integration

### `packages/brain-shell/src/adapter/doctorProbe.ts`
- `DoctorProbe` accepts an optional `client?: BrainBackendClient`.
- `checkDaemonHealth()` queries `client.getSystemStatus()`.
- Populates authoritative subsystem statuses:
  - **UDS Daemon Socket & Uptime**: status, latency, uptime formatted (e.g. `2h 15m`).
  - **SQLite Database Engine**: session count, concept node count, relationship edge count.
  - **Model Gateway**: active registered model count.
- If client query fails or client is not provided, falls back gracefully to socket connectivity check without crashing.

### `packages/brain-shell/src/ui/overlays/DoctorOverlayView.tsx`
- Displays authoritative metrics (version, uptime, total sessions, memory nodes, edges, models).

---

## Verification Matrix

1. **Unit Tests**:
   - `daemonStatusLogic.test.ts`: Tests `DoctorProbe` with mock backend returning healthy, degraded, and unreachable responses.
2. **Canonical Live PTY Smoke Test (`scripts/ptySmokeInc45.py`)**:
   - Spawns live daemon and TUI client.
   - Ingests memory concepts and links them.
   - Opens `/doctor` overlay.
   - Asserts authoritative metrics appear: `● HEALTHY`, concept node counts, edge counts, version, and uptime.
3. **22-Suite Canonical Regression Matrix**:
   - Incs 2 through 45 all pass.
4. **Rust domain/storage test suite & clean detached worktree check**.
5. **WIP baseline preservation**: 3,938 paths byte-identical.
