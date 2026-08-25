# Brain Shell Inc 15 — Daemon Reconnection with Backoff & Queued Input

**Date:** 2026-08-25 · **Base:** `main` @ `7047fbad` · **Audit item:** B1 (reconnect/backoff + queued-input replay)
**Status:** Approved design — sections 1–2 walked through in chat and confirmed by the user.

## 0. Problem

The shell's UDS client documents a strict "zero-reconnect invariant": any daemon
outage permanently strands the session. A daemon restart mid-conversation kills the
stream, the user's next prompt fails, and input typed during the outage is silently
dropped. Claude Code-grade parity requires: detect the loss, say so visibly, keep
trying until the daemon returns, and hold (not drop) prompts typed while offline.

Proven boundary facts that shape this design:

- An **in-flight generation cannot be resumed**: it lives only in daemon memory.
  Transparent mid-turn resume would require daemon contract changes — out of scope.
- **Sessions persist across daemon restarts on disk** (`cargo` e2e suite
  `test_product_e2e_persistence_and_daemon_restart`), so post-restart turns continue
  with the existing `sessionId`.
- Every client RPC already opens a fresh socket and fails with typed errors; stream
  failures already arrive as error chunks. Detection raw material exists.

## 1. Decisions

| # | Decision | Choice |
|---|----------|--------|
| D1 | Input typed while disconnected | **Auto-replay FIFO queue** with visible "queued" rows; fires automatically on restore |
| D2 | Retry policy | **Indefinite** exponential backoff (500 ms × 2^n, cap 5 s, ±20% jitter) while the TUI is open |
| D3 | Mid-turn socket drop | **Fail that turn cleanly** (partial transcript kept, tools cancelled), then reconnect for subsequent actions |
| D4 | Placement | **Controller-level `ConnectionMonitor`** (Approach A) — client interface, wire format, and daemon untouched |

Superseded: the vendor-era "deterministic disconnect / zero-reconnect" invariant in
the `UdsBrainBackendClient` header comment is deliberately retired by this increment;
the comment is updated to describe the new lifecycle.

## 2. Architecture

New connection-lifecycle subsystem inside `packages/brain-shell`, three pieces:

```
UdsBrainBackendClient ──(typed failures / error chunks, unchanged)──▶ SessionController
                                                                          │ classifies loss
                                                          ConnectionMonitor│ starts backoff loop
                                                          probeDaemonSocket│ bare connect probe
SessionController ◀──onRestored── ConnectionMonitor ◀──probe ok────────────┘
      │ drains FIFO through normal submit()
      ▼
ShellSnapshot.connection ──▶ StatusBar segment + banner copy
```

- **Detection** reuses existing signals only: stream chunks matching an extended
  connection-error predicate, plus the controller's catch path for thrown RPC errors.
  No polling while healthy; the monitor runs *only* after a classified loss.
- **Probe** is transport-level (`net.createConnection`), not protocol-level — zero
  daemon involvement, no invented health action.

### Layer impact

| Layer | Change |
|-------|--------|
| `client/UdsBrainBackendClient.ts` | Header comment updated; resolved socket path exposed as readonly field. Nothing else. |
| `client/probeDaemonSocket.ts` | NEW — connect probe helper |
| `state/connectionMonitor.ts` | NEW — state machine + `nextDelayMs` |
| `state/sessionController.ts` | Snapshot field, classification predicate, queue, drain, dispose |
| `ui/shell/{StatusBar,AppShell}.tsx` | Optional `connection` prop → segment; banner source switch |
| daemon / crates | **None** |

## 3. Components & Data Flow

### 3.1 `state/connectionMonitor.ts`

```ts
export type ConnectionState =
  | { status: 'connected' }
  | { status: 'reconnecting'; attempt: number };

export function nextDelayMs(attempt: number, rng: () => number = Math.random): number;
// attempt is 1-based: the delay before probe N uses nextDelayMs(N).

export class ConnectionMonitor {
  constructor(opts: {
    probe: () => Promise<boolean>;
    delay: (ms: number) => Promise<void>;   // injectable clock
    onChange: (s: ConnectionState) => void;
    onRestored: () => void;
  });
  start(): void;   // idempotent while already running
  stop(): void;    // generation-counter cancellation; no leaked timers
}
```

Backoff schedule: base 500 ms, ×2 per failed attempt, capped at 5 000 ms, each value
jittered ±20% via injectable RNG. `attempt` counts probes and starts at **1** when the
monitor arms (the status bar therefore never reads 0); it increments per failed probe,
and `onChange` fires on every transition so the segment can show the current count.

### 3.2 `client/probeDaemonSocket.ts`

```ts
export function probeDaemonSocket(socketPath: string, timeoutMs = 1500): Promise<boolean>;
```

Resolves `true` if the socket connects within the timeout; destroys the socket either
way. Never throws.

### 3.3 Controller integration (`sessionController.ts`)

- `ShellSnapshot` gains `connection: ConnectionState` (default `{status:'connected'}`).
- `isConnectionLoss(text: string): boolean` extends the existing `CONNECTION_RE`
  idiom to also match the client's RPC failure vocabulary ("socket not found",
  "socket error", "connection closed unexpectedly", "RPC timeout"). Returns **false**
  for anything matching `/abort/i` — esc must never arm the monitor.
- `submit(text)` while `connection.status !== 'connected'`: appends text to a private
  FIFO, appends the system row `queued — will send on reconnect`, returns without
  touching the client.
- On classified loss (mid-stream chunk or caught RPC throw): the turn settles exactly
  as today (`finishTurn('error')`, partial frozen rows kept, unsettled tools
  cancelled), the terminal row reads `Connection lost — reconnecting…`,
  `connection` flips to `reconnecting`, and `monitor.start()` arms the loop.
- `onRestored`: sets `connected`, then drains the FIFO sequentially through the
  normal public `submit()`; before each replay it re-checks connectivity — a second
  outage mid-drain leaves remaining items queued.
- New `dispose()` stops the monitor; AppShell calls it from a React cleanup effect.

### 3.4 What renders

- Status bar: silent when connected; otherwise `· reconnecting (attempt N)` in the
  warning color, replacing nothing (appended segment).
- Banner line above composer: during disconnection shows `Connection lost —
  reconnecting…`; when connected, previous behavior (raw last error) is preserved.
- Queued prompts are visible transcript rows via `notice()`.

## 4. Error Handling

| Hazard | Handling |
|--------|----------|
| Esc/abort during outage or stream | `isConnectionLoss` returns false for aborts; no reconnect machinery armed |
| Monitor leak after unmount | `dispose()` → `stop()` with generation counter invalidating in-flight probes/timers |
| Probe hangs | 1.5 s probe timeout; probe never rejects |
| Second outage mid-drain | Drain halts; remainder stays queued; monitor rearms |
| Drop while permission dialog parked | Dialog is local UX; best-effort resolve already swallows failure; tool settles cancelled via existing error path |
| Stale sessionId after restart | Sessions persist daemon-side (e2e-proven); if load ever fails, the existing resume-style notice surfaces it |

## 5. Testing Strategy

TDD, one layer at a time:

1. **`nextDelayMs` unit** — growth curve, 5 s cap never exceeded, jitter bounds under
   a fixed RNG.
2. **`ConnectionMonitor` unit** (fake probe/instant clock) — attempt sequence and
   `onChange` transitions, exactly one `onRestored` per recovery, `stop()` cancels.
3. **`sessionControllerReconnect.test.ts`** (fake client harness):
   - submit while offline → queued row, no client call; restore → ordered replay of
     both queued prompts with real streamed answers;
   - mid-turn connection-loss chunk → partial transcript frozen, error row copy,
     snapshot `connection.status === 'reconnecting'`;
   - abort-classified error does NOT flip connection state.
4. **`scripts/ptySmokeInc15.py`** — stub daemon refuses connections at launch:
   expect reconnecting segment + queued row; then accept connections: queued prompt
   auto-fires and its answer renders. Live end-to-end proof.
5. **Standard gates:** bun suite at baseline, cargo workspace green except the sole
   permitted audit test, vendor scan 0, tsc file-level diff vs `main`.

## 6. Non-Goals

- Mid-turn transparent resumption (requires daemon generation-state durability).
- General busy-time queueing beyond the disconnected case (charter OOS).
- `/reconnect` manual command (D2 makes it unnecessary).
- Multi-server/failover routing, remote daemons, mDNS discovery.
- Daemon-side liveness announcements.

## 7. Constraints

- Preserve Brain architecture, IPC contracts, runtime boundaries; **zero daemon/wire
  changes**.
- No Claude/Anthropic models, APIs, auth, pricing, billing, or LLM product concepts;
  no vendor-derived code — implementation is Brain-owned from first principles.
- Stack unchanged: Bun + React 19 + Ink 7 + yoga-layout + Rust daemon.
- Every commit carries only explicitly added paths; pushes to origin require
  explicit user approval each time.
