# Brain Shell Inc 15 — Daemon Reconnection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The shell survives daemon outages — visible reconnecting state with capped exponential backoff, prompts typed offline auto-replay in order on restore, and a mid-turn drop fails that turn cleanly without losing streamed output.

**Architecture:** A controller-owned `ConnectionMonitor` state machine arms only after a classified connection loss, probing the UDS path at transport level (bare connect, no protocol, zero daemon changes). Queued input drains through the normal public `submit()` so replayed turns are indistinguishable from hand-typed ones. UI reads `connection` off the existing snapshot.

**Tech Stack:** Bun + TypeScript + React 19 + Ink 7 (brain-shell); Python 3 PTY harness for live proof; Rust daemon untouched.

**Spec:** `docs/superpowers/specs/2026-08-25-brain-shell-inc15-daemon-reconnect-design.md` — read together with this plan; decisions D1–D4 and copy strings come from it.

## Global Constraints

From the spec (§7) — binding on every task:

- Preserve Brain architecture, IPC contracts, runtime boundaries; **zero daemon/wire changes**.
- No Claude/Anthropic models, APIs, auth, pricing, billing, or LLM product concepts; no vendor-derived code — implementation is Brain-owned from first principles.
- Stack unchanged: Bun + React 19 + Ink 7 + yoga-layout + Rust daemon.
- Every commit carries ONLY explicitly added paths (`git add <paths>`, NEVER `git add .`); commit trailer `Co-Authored-By: Claude <noreply@anthropic.com>`; NEVER `git stash` this repo (~1k uncommitted user-WIP files); pushes to origin require explicit user approval each time.
- macOS cargo wrapper (every cargo invocation):
  `bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo ...'`
- Known-good baseline noise: cargo `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers` may fail (sole permitted failure); bun suite has exactly 5 documented pre-existing failures (2 tracked `visualCellParity` invariants + 3 files that are untracked WIP absent from `main`).
- Working tree carries user WIP (`AGENTS.md`, `Cargo.lock`, others) — never stage, never revert, never stash it.
- zsh cwd persists between tool calls — re-anchor with absolute paths (`cd /Users/ritikpathania/Developer/PyCharm/brain`) whenever a prior command moved.

Exact copy strings (spec §3.3/§3.4/§4):

- Terminal loss row: `Connection lost — reconnecting…`
- Queue row: `queued — will send on reconnect`
- Status-bar segment: `reconnecting (attempt N)`

---

### Task 1: Branch + `ConnectionMonitor` state machine

**Files:**
- Create: `packages/brain-shell/src/state/connectionMonitor.ts`
- Test: `packages/brain-shell/src/test/state/connectionMonitor.test.ts`

**Interfaces:**
- Consumes: nothing (leaf module).
- Produces (used verbatim by Tasks 3–4):
  - `export type ConnectionState = { status: 'connected' } | { status: 'reconnecting'; attempt: number };`
  - `export function nextDelayMs(attempt: number, rng?: () => number): number;` — 1-based attempt, `500 × 2^(attempt−1)` capped at 5 000 ms, jitter ±20%.
  - `export class ConnectionMonitor { constructor(opts: ConnectionMonitorOpts); start(): void; stop(): void; get state(): ConnectionState; }` with `ConnectionMonitorOpts = { probe: () => Promise<boolean>; delay: (ms: number) => Promise<void>; onChange: (s: ConnectionState) => void; onRestored: () => void; }`.
  - Semantics: `start()` arms immediately — `onChange({status:'reconnecting', attempt:1})` fires synchronously before `start()` returns; the delay BEFORE probe N is `nextDelayMs(N)`; exactly one `onRestored()` per recovery; `stop()` invalidates in-flight loops via generation counter.

- [ ] **Step 0: Create the branch**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git checkout -b feature/brain-shell-inc15-daemon-reconnect
```

- [ ] **Step 1: Write the failing tests**

Create `packages/brain-shell/src/test/state/connectionMonitor.test.ts`:

```ts
import { describe, it, expect } from 'bun:test';
import {
  ConnectionMonitor,
  nextDelayMs,
  type ConnectionState,
} from '../../state/connectionMonitor.js';

/** Flush the microtask queue plus one macrotask so async loops make progress. */
const tick = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

describe('nextDelayMs', () => {
  it('doubles per attempt, caps at 5000, and applies ±20% jitter', () => {
    const mid = (): number => 0.5; // jitter factor exactly 1.0
    expect(nextDelayMs(1, mid)).toBe(500);
    expect(nextDelayMs(2, mid)).toBe(1000);
    expect(nextDelayMs(3, mid)).toBe(2000);
    expect(nextDelayMs(4, mid)).toBe(4000);
    expect(nextDelayMs(5, mid)).toBe(5000); // cap: 8000 would overflow
    expect(nextDelayMs(50, mid)).toBe(5000);
    // Jitter bounds: rng 0 → ×0.8, rng 1 → ×1.2.
    expect(nextDelayMs(1, () => 0)).toBe(400);
    expect(nextDelayMs(1, () => 1)).toBe(600);
    expect(nextDelayMs(99, () => 1)).toBe(6000); // cap × 1.2 upper bound
  });
});

interface Harness {
  transitions: ConnectionState[];
  delays: number[];
  restored: number;
  probeCalls: number;
}

/**
 * Wire a monitor to scripted probe results, a fixed RNG, and a MANUAL
 * clock: each delay parks on a gate the test releases explicitly.
 * Wall-clock tick pacing is nondeterministic under bun (all due zero-ms
 * timers fire in one event-loop turn), so gates are the only honest way
 * to single-step the loop.
 */
function harness(results: boolean[]): {
  h: Harness;
  monitor: ConnectionMonitor;
  /** Release the N oldest parked delays, settling between each. */
  pump: (n: number) => Promise<void>;
} {
  const h: Harness = { transitions: [], delays: [], restored: 0, probeCalls: 0 };
  const queue = [...results];
  const gates: Array<() => void> = [];
  const monitor = new ConnectionMonitor({
    probe: () => {
      h.probeCalls += 1;
      return Promise.resolve(queue.length > 0 ? (queue.shift() as boolean) : false);
    },
    delay: (ms) => {
      h.delays.push(ms);
      return new Promise<void>((r) => gates.push(r));
    },
    rng: () => 0.5,
    onChange: (s) => h.transitions.push(s),
    onRestored: () => {
      h.restored += 1;
    },
  });
  const pump = async (n: number): Promise<void> => {
    for (let i = 0; i < n; i++) {
      gates.shift()?.();
      await tick();
    }
  };
  return { h, monitor, pump };
}

describe('ConnectionMonitor', () => {
  it('walks attempts with growing delays and restores exactly once', async () => {
    const { h, monitor, pump } = harness([false, false, true]);
    monitor.start();
    // Arming is synchronous: attempt 1 visible, first delay parked.
    expect(monitor.state).toEqual({ status: 'reconnecting', attempt: 1 });
    expect(h.probeCalls).toBe(0);
    await pump(3);
    expect(h.transitions).toEqual([
      { status: 'reconnecting', attempt: 1 },
      { status: 'reconnecting', attempt: 2 },
      { status: 'reconnecting', attempt: 3 },
      { status: 'connected' },
    ]);
    // Delay precedes each probe: nextDelayMs(N), unjittered (rng 0.5).
    expect(h.delays).toEqual([500, 1000, 2000]);
    expect(h.restored).toBe(1);
    expect(h.probeCalls).toBe(3);
  });

  it('ignores start() while already running', async () => {
    const { h, monitor, pump } = harness([true]);
    monitor.start();
    monitor.start();
    monitor.start();
    await pump(1);
    expect(h.probeCalls).toBe(1);
    expect(h.restored).toBe(1);
  });

  it('stop() cancels the in-flight loop permanently', async () => {
    const { h, monitor, pump } = harness([false, false, false]);
    monitor.start();
    await pump(1); // probe #1 fails -> attempt 2 armed, delay parked
    expect(h.probeCalls).toBe(1);
    expect(h.transitions).toEqual([
      { status: 'reconnecting', attempt: 1 },
      { status: 'reconnecting', attempt: 2 },
    ]);
    monitor.stop();
    await pump(4); // release every future wakeup — none may act on them
    expect(h.probeCalls).toBe(1);
    expect(h.transitions).toEqual([
      { status: 'reconnecting', attempt: 1 },
      { status: 'reconnecting', attempt: 2 },
    ]);
    expect(h.restored).toBe(0);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/state/connectionMonitor.test.ts
```

Expected: FAIL — cannot resolve `../../state/connectionMonitor.js`.

- [ ] **Step 3: Implement the module**

Create `packages/brain-shell/src/state/connectionMonitor.ts`:

```ts
/**
 * Inc 15: connection-lifecycle state machine for daemon outages.
 * Pure control flow — the probe and the clock arrive as injectable seams,
 * so tests resolve instantly and production wires a real socket probe.
 * The monitor runs ONLY after a classified loss; healthy sessions never
 * pay for it.
 */

export type ConnectionState =
  | { status: 'connected' }
  | { status: 'reconnecting'; attempt: number };

const BASE_DELAY_MS = 500;
const CAP_DELAY_MS = 5_000;

/**
 * Delay before probe N (1-based): 500 × 2^(N−1), capped at 5 s, jittered
 * ±20% through the injectable RNG so schedules are reproducible in tests.
 */
export function nextDelayMs(attempt: number, rng: () => number = Math.random): number {
  const raw = Math.min(BASE_DELAY_MS * 2 ** (attempt - 1), CAP_DELAY_MS);
  return Math.round(raw * (1 + (rng() * 0.4 - 0.2)));
}

export interface ConnectionMonitorOpts {
  /** Resolve true when the daemon accepts connections. Never rejects. */
  probe: () => Promise<boolean>;
  /** Injectable clock; production sleeps, tests resolve instantly. */
  delay: (ms: number) => Promise<void>;
  /** Injectable jitter source so schedules are reproducible under test. */
  rng?: () => number;
  onChange: (state: ConnectionState) => void;
  /** Fired exactly once when a probe succeeds. */
  onRestored: () => void;
}

export class ConnectionMonitor {
  private epoch = 0; // stop() bumps this; stale loops bail on mismatch
  private current: ConnectionState = { status: 'connected' };

  constructor(private opts: ConnectionMonitorOpts) {}

  get state(): ConnectionState {
    return this.current;
  }

  /** Arm the loop after a classified loss. Idempotent while running. */
  start(): void {
    if (this.current.status === 'reconnecting') return;
    void this.run(++this.epoch);
  }

  /** Cancel any in-flight loop; subsequent wakeups become no-ops. */
  stop(): void {
    this.epoch++;
  }

  private async run(epoch: number): Promise<void> {
    let attempt = 1;
    for (;;) {
      if (epoch !== this.epoch) return;
      this.set({ status: 'reconnecting', attempt });
      // Spec: the delay BEFORE probe N is nextDelayMs(N).
      await this.opts.delay(nextDelayMs(attempt, this.opts.rng));
      if (epoch !== this.epoch) return;
      if (await this.opts.probe()) {
        if (epoch !== this.epoch) return;
        this.set({ status: 'connected' });
        this.opts.onRestored();
        return;
      }
      attempt += 1;
    }
  }

  private set(state: ConnectionState): void {
    this.current = state;
    this.opts.onChange(state);
  }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/state/connectionMonitor.test.ts
```

Expected: 4 pass / 0 fail.

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git add packages/brain-shell/src/state/connectionMonitor.ts packages/brain-shell/src/test/state/connectionMonitor.test.ts
git commit -m "feat(shell): ConnectionMonitor backoff state machine

Inc 15 task 1: pure control flow for daemon-outage retry — 1-based
attempts, 500ms×2^n capped at 5s with ±20% jitter, injectable probe and
clock seams, generation-counter cancellation. Runs only after a
classified loss; arming publishes attempt 1 synchronously.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: `probeDaemonSocket` + client surface

**Files:**
- Create: `packages/brain-shell/src/client/probeDaemonSocket.ts`
- Modify: `packages/brain-shell/src/client/UdsBrainBackendClient.ts:5` (header comment) and `:42` (constructor parameter property)
- Test: `packages/brain-shell/src/test/client/probeDaemonSocket.test.ts`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces (Task 3 consumes both):
  - `export function probeDaemonSocket(socketPath: string, timeoutMs?: number): Promise<boolean>;` — resolves (never rejects); `true` iff a bare connect succeeds within the timeout; destroys the socket either way.
  - `UdsBrainBackendClient` exposes `readonly socketPath: string` (previously `private`).

- [ ] **Step 1: Write the failing tests**

Create `packages/brain-shell/src/test/client/probeDaemonSocket.test.ts`:

```ts
import { describe, expect, test, afterAll } from 'bun:test';
import * as net from 'node:net';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { probeDaemonSocket } from '../../client/probeDaemonSocket.js';

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-probe-'));
const sockPath = path.join(dir, 'live.sock');

// A real listener proves the happy path; a missing path proves the
// failure path. No protocol bytes involved — transport liveness only.
const server = net.createServer(() => {});
server.listen(sockPath);

afterAll(() => {
  server.close();
  fs.rmSync(dir, { recursive: true, force: true });
});

describe('probeDaemonSocket', () => {
  test('resolves true against a live listener', async () => {
    expect(await probeDaemonSocket(sockPath, 1000)).toBe(true);
  });

  test('resolves false (never rejects) against a dead path', async () => {
    expect(await probeDaemonSocket(path.join(dir, 'absent.sock'), 1000)).toBe(false);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/client/probeDaemonSocket.test.ts
```

Expected: FAIL — cannot resolve `../../client/probeDaemonSocket.js`.

- [ ] **Step 3: Implement the probe**

Create `packages/brain-shell/src/client/probeDaemonSocket.ts`:

```ts
/**
 * Inc 15: transport-level daemon liveness probe. Opens a bare connection
 * to the UDS path — no protocol bytes, no invented health action — and
 * reports whether anything is accepting. Resolves (never rejects) and
 * destroys the socket either way.
 */
import * as net from 'net';

export function probeDaemonSocket(socketPath: string, timeoutMs = 1500): Promise<boolean> {
  return new Promise((resolve) => {
    let settled = false;
    const socket = net.createConnection(socketPath);
    const timer = setTimeout(() => finish(false), timeoutMs);
    timer.unref?.();

    function finish(ok: boolean): void {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      socket.destroy();
      resolve(ok);
    }

    socket.once('connect', () => finish(true));
    socket.once('error', () => finish(false));
  });
}
```

- [ ] **Step 4: Expose the socket path and update the header comment**

In `packages/brain-shell/src/client/UdsBrainBackendClient.ts`:

Replace line 5:

```ts
 * Adheres strictly to the deterministic disconnect / zero-reconnect invariant.
```

with:

```ts
 * Sockets are single-request; outage recovery lives above this class in the
 * shell's ConnectionMonitor (Inc 15) — deliberately retired zero-reconnect.
```

Replace line 42:

```ts
  constructor(private socketPath: string = process.env.BRAIN_SOCKET_PATH || '/tmp/brain.sock') {}
```

with:

```ts
  constructor(readonly socketPath: string = process.env.BRAIN_SOCKET_PATH || '/tmp/brain.sock') {}
```

Run the probe tests again (client change must not break compilation):

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/client/
```

Expected: all existing wire tests plus the 2 new probe tests pass.

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git add packages/brain-shell/src/client/probeDaemonSocket.ts packages/brain-shell/src/client/UdsBrainBackendClient.ts packages/brain-shell/src/test/client/probeDaemonSocket.test.ts
git commit -m "feat(shell): transport-level daemon probe; expose socket path

Inc 15 task 2: bare-connect liveness probe (no protocol bytes, resolves
never rejects) plus readonly socketPath on the UDS client so the
controller can arm the monitor. Retires the vendor-era zero-reconnect
invariant wording deliberately per the approved Inc 15 spec.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: SessionController integration

**Files:**
- Modify: `packages/brain-shell/src/state/sessionController.ts`
- Test: `packages/brain-shell/src/test/state/sessionControllerReconnect.test.ts`

**Interfaces:**
- Consumes from Task 1: `ConnectionState`, `ConnectionMonitor`. From Task 2: `probeDaemonSocket`, `socketPath` field.
- Produces (Tasks 4–5 consume):
  - `ShellSnapshot.connection: ConnectionState` (default `{status:'connected'}`).
  - `new SessionController(client, probeOverride?, delayOverride?)` — optional seams used by tests and prod alike; prod passes neither.
  - `dispose(): void`.
  - Copy constants exported for reuse: `CONNECTION_LOSS_ROW = 'Connection lost — reconnecting…'`, `QUEUED_ROW = 'queued — will send on reconnect'`.

- [ ] **Step 1: Write the failing tests**

Create `packages/brain-shell/src/test/state/sessionControllerReconnect.test.ts`:

```ts
import { describe, it, expect } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import type {
  BrainBackendClient,
  BrainStreamChunk,
  BrainGenerationRequest,
} from '../../client/BrainBackendClient.js';

/** Poll until predicate holds or 2 s elapse. */
async function waitFor(pred: () => boolean): Promise<void> {
  const deadline = Date.now() + 2000;
  while (!pred()) {
    if (Date.now() > deadline) throw new Error('waitFor timeout');
    await new Promise((r) => setTimeout(r, 5));
  }
}

const instantDelay = (): Promise<void> => Promise.resolve();

/** Client whose createSession fails while `up` is false; streams otherwise. */
function outageClient(): BrainBackendClient & { setUp(up: boolean): void } {
  let up = false;
  return {
    setUp(v: boolean) {
      up = v;
    },
    async createSession() {
      if (!up) throw new Error('Brain daemon socket not found at /tmp/brain.sock');
      return { sessionId: 'stub-session-15', title: 'stub', createdAtMs: 0 };
    },
    async *streamText(_req: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
      yield { type: 'token', token: 'back-online' };
      yield { type: 'finished', status: 'completed' };
    },
  } as BrainBackendClient & { setUp(up: boolean): void };
}

describe('Inc 15: reconnection', () => {
  it('queues prompts while offline and replays them in order on restore', async () => {
    const client = outageClient();
    let up = false;
    const ctl = new SessionController(
      client,
      async () => up,          // probeOverride: poll the flag
      instantDelay,            // delayOverride: instant backoff clock
    );
    await ctl.submit('first question'); // fails -> reconnecting arms
    expect(ctl.getSnapshot().connection.status).toBe('reconnecting');
    await ctl.submit('second question'); // offline -> queued row
    const textsOf = (kind: 'system' | 'error'): string[] =>
      ctl.getSnapshot().rows.filter((r) => r.kind === kind).map((r) => r.text);
    expect(textsOf('system')).toContain('queued — will send on reconnect');
    expect(textsOf('error')).toContain('Connection lost — reconnecting…');
    // Restore: the monitor restores, then BOTH prompts replay in order.
    client.setUp(true);
    up = true;
    await waitFor(() => ctl.getSnapshot().connection.status === 'connected');
    await waitFor(
      () =>
        ctl
          .getSnapshot()
          .rows.filter((r) => r.kind === 'assistant' && r.markdown === 'back-online')
          .length === 2,
    );
    const userRows = ctl
      .getSnapshot()
      .rows.filter((r) => r.kind === 'user')
      .map((r) => r.text);
    expect(userRows).toEqual(['first question', 'second question']);
  });

  it('fails a mid-turn drop cleanly and arms the monitor', async () => {
    let sawLoss = false;
    const client = {
      async createSession() {
        return { sessionId: 'stub-session-15', title: 'stub', createdAtMs: 0 };
      },
      async *streamText(): AsyncIterable<BrainStreamChunk> {
        yield { type: 'token', token: 'partial answer' };
        sawLoss = true;
        yield {
          type: 'error',
          error: 'Brain daemon connection closed unexpectedly on v1/generation/stream',
        };
      },
    } as unknown as BrainBackendClient;
    const ctl = new SessionController(client, async () => false, instantDelay);
    await ctl.submit('drop me');
    const snap = ctl.getSnapshot();
    // Partial streamed output survives as a frozen row…
    expect(snap.rows.some((r) => r.kind === 'assistant' && r.markdown.includes('partial answer'))).toBe(true);
    // …the loss copy replaces the raw wire error…
    expect(snap.rows.some((r) => r.kind === 'error' && r.text === 'Connection lost — reconnecting…')).toBe(true);
    // …and the monitor armed.
    expect(snap.connection.status).toBe('reconnecting');
    expect(snap.busy).toBe(false);
  });

  it('never arms the monitor for abort-classified errors', async () => {
    const client = {
      async createSession() {
        return { sessionId: 'stub-session-15', title: 'stub', createdAtMs: 0 };
      },
      async *streamText(): AsyncIterable<BrainStreamChunk> {
        yield { type: 'token', token: 'halfway' };
        yield { type: 'error', error: 'v1/generation/stream aborted' };
      },
    } as unknown as BrainBackendClient;
    const ctl = new SessionController(client, async () => false, instantDelay);
    await ctl.submit('cancel me');
    expect(ctl.getSnapshot().connection.status).toBe('connected');
  });
});
```

Note on the first test's closure trick: `up` is captured by the probe closure and flipped alongside `setUp(true)` — the probe keeps answering `false` until the flag flips, mirroring a daemon that starts accepting later.

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/state/sessionControllerReconnect.test.ts
```

Expected: FAIL — `SessionController` accepts one argument, snapshot has no `connection`.

- [ ] **Step 3: Integrate into the controller**

All changes in `packages/brain-shell/src/state/sessionController.ts`. Apply each edit exactly:

**(3a)** Extend the import block (after the `chunkToTurnEvents` import):

```ts
import {
  ConnectionMonitor,
  type ConnectionState,
} from './connectionMonitor.js';
import { probeDaemonSocket } from '../client/probeDaemonSocket.js';
```

**(3b)** Below the existing `CONNECTION_RE` constant (`:43`) replace the single-line regex with the classification pair, keeping the constant name for the banner path:

```ts
const CONNECTION_RE = /Could not connect|socket error|disconnected/i;
const CONNECTION_LOSS_RE =
  /Could not connect|socket not found|socket error|disconnected|connection closed|RPC timeout/i;
const ABORT_RE = /abort/i;

/** Inc 15: a classified connection loss arms the monitor; aborts never do. */
function isConnectionLoss(text: string): boolean {
  return !ABORT_RE.test(text) && CONNECTION_LOSS_RE.test(text);
}

export const CONNECTION_LOSS_ROW = 'Connection lost — reconnecting…';
export const QUEUED_ROW = 'queued — will send on reconnect';
```

**(3c)** Add `connection` to `ShellSnapshot` and default state fields. Interface becomes:

```ts
export interface ShellSnapshot {
  rows: TranscriptRow[];
  live: LiveStreamView;
  busy: boolean;
  connectionError?: string;
  connection: ConnectionState;
  permission?: PendingPermissionView;
}
```

**(3d)** Constructor gains the two optional seams; new private fields land beside `thinkingStartedAt`:

```ts
  constructor(
    private client: BrainBackendClient,
    private probeOverride?: () => Promise<boolean>,
    private delayOverride?: (ms: number) => Promise<void>,
  ) {}
```

```ts
  private connection: ConnectionState = { status: 'connected' };
  private queuedInputs: string[] = [];
  private monitor: ConnectionMonitor | null = null;
  private lostDuringTurn = false;
```

**(3e)** New members (place after `resolvePermission`, before `clear`):

```ts
  /** Inc 15: stop the reconnect loop (AppShell cleanup effect). */
  dispose(): void {
    this.monitor?.stop();
  }

  private ensureMonitor(): ConnectionMonitor {
    if (this.monitor === null) {
      const socketPath = (this.client as { socketPath?: string }).socketPath;
      const probe =
        this.probeOverride ??
        (typeof socketPath === 'string'
          ? () => probeDaemonSocket(socketPath)
          : async () => true); // fakes without a transport restore immediately
      const delay =
        this.delayOverride ?? ((ms: number) => new Promise<void>((r) => setTimeout(r, ms)));
      this.monitor = new ConnectionMonitor({
        probe,
        delay,
        onChange: (s) => {
          this.connection = s;
          this.emit();
        },
        onRestored: () => {
          void this.drainQueue();
        },
      });
    }
    return this.monitor;
  }

  /** Classified loss anywhere: show the banner, arm the loop once. */
  private handleConnectionLoss(): void {
    if (this.connection.status === 'reconnecting') return;
    if (this.busy) this.lostDuringTurn = true;
    this.ensureMonitor().start();
  }

  private async drainQueue(): Promise<void> {
    while (this.queuedInputs.length > 0 && this.connection.status === 'connected') {
      await this.submit(this.queuedInputs[0]);
      // A second outage during this submit leaves the item queued.
      if (this.connection.status === 'connected') this.queuedInputs.shift();
    }
  }
```

**(3f)** `submit()` — insert the offline branch between the busy guard and the existing body, and route terminal text through the loss copy. The head of the method becomes:

```ts
  async submit(text: string): Promise<void> {
    // Inc 14: a submit during a live turn gets the same feedback as every
    // other busy-path entry point instead of vanishing.
    if (this.busy) {
      this.notice('Busy — wait for the current turn to finish.');
      return;
    }
    // Inc 15: offline submits join the replay queue instead of failing.
    if (this.connection.status !== 'connected') {
      this.queuedInputs.push(text);
      this.notice(QUEUED_ROW);
      return;
    }
    this.busy = true;
    this.connectionError = undefined;
    this.lostDuringTurn = false;
```

(the last line joins the existing assignments; keep everything below unchanged)

and the tail of the method becomes:

```ts
      for await (const chunk of this.client.streamText(request)) {
        this.handleChunk(chunk);
      }
      // An error chunk doesn't throw — the stream just ends — but the turn
      // still failed and must settle its tools accordingly.
      this.finishTurn(
        this.sawError ? 'error' : 'completed',
        this.lostDuringTurn ? CONNECTION_LOSS_ROW : undefined,
      );
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (isConnectionLoss(msg)) this.handleConnectionLoss();
      this.finishTurn('error', isConnectionLoss(msg) ? CONNECTION_LOSS_ROW : msg);
    }
```

**(3g)** `handleChunk()` — classify error chunks (the existing banner assignment stays, the classification is added):

```ts
    if (chunk.type === 'error' && chunk.error) {
      if (CONNECTION_RE.test(chunk.error)) {
        this.connectionError = chunk.error;
      }
      if (isConnectionLoss(chunk.error)) {
        this.handleConnectionLoss();
      }
    }
```

**(3h)** `emit()` — include the new field:

```ts
    this.snapshot = {
      rows: this.rows,
      live: this.live,
      busy: this.busy,
      connectionError: this.connectionError,
      connection: this.connection,
      permission: this.pendingPermission,
    };
```

- [ ] **Step 4: Run the new tests, then the whole suite**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/state/sessionControllerReconnect.test.ts
bun test
```

Expected: 3 pass on the new file. Full suite: previous baseline plus these 3 — same 5 documented pre-existing failures, no new ones.

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git add packages/brain-shell/src/state/sessionController.ts packages/brain-shell/src/test/state/sessionControllerReconnect.test.ts
git commit -m "feat(shell): controller reconnect integration with replay queue

Inc 15 task 3: classified connection losses arm the ConnectionMonitor;
offline submits join a FIFO surfaced as 'queued' rows and drain through
the normal submit() on restore (re-checked per item). Mid-turn drops
freeze partial output under the 'Connection lost — reconnecting…' copy;
abort-classified errors never arm the loop. Snapshot gains connection.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: UI wiring — status-bar segment, banner, dispose

**Files:**
- Create: `packages/brain-shell/src/ui/shell/connectionStatusLogic.ts`
- Modify: `packages/brain-shell/src/ui/shell/StatusBar.tsx`
- Modify: `packages/brain-shell/src/ui/shell/AppShell.tsx:39-41` (dispose effect), `:201-203` (banner)
- Test: `packages/brain-shell/src/test/ui/shell/connectionStatusLogic.test.ts`

**Interfaces:**
- Consumes from Task 3: `ShellSnapshot.connection`, `controller.dispose()`.
- Produces: `export function connectionStatusText(state: ConnectionState | undefined): string | null;` — `null` hides the segment; otherwise `'reconnecting (attempt N)'`.

- [ ] **Step 1: Write the failing test**

Create `packages/brain-shell/src/test/ui/shell/connectionStatusLogic.test.ts`:

```ts
import { describe, it, expect } from 'bun:test';
import { connectionStatusText } from '../../../ui/shell/connectionStatusLogic.js';

describe('connectionStatusText', () => {
  it('hides when connected or unknown', () => {
    expect(connectionStatusText(undefined)).toBeNull();
    expect(connectionStatusText({ status: 'connected' })).toBeNull();
  });

  it('reports the attempt count while reconnecting', () => {
    expect(connectionStatusText({ status: 'reconnecting', attempt: 1 })).toBe(
      'reconnecting (attempt 1)',
    );
    expect(connectionStatusText({ status: 'reconnecting', attempt: 7 })).toBe(
      'reconnecting (attempt 7)',
    );
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/ui/shell/connectionStatusLogic.test.ts
```

Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement the logic module**

Create `packages/brain-shell/src/ui/shell/connectionStatusLogic.ts`:

```ts
/** Pure projection: status-bar text for a non-connected shell. Null hides. */
import type { ConnectionState } from '../../state/connectionMonitor.js';

export function connectionStatusText(state: ConnectionState | undefined): string | null {
  if (!state || state.status === 'connected') return null;
  return `reconnecting (attempt ${state.attempt})`;
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/ui/shell/connectionStatusLogic.test.ts
```

Expected: 2 pass / 0 fail.

- [ ] **Step 5: Wire StatusBar and AppShell**

`StatusBar.tsx` — extend props and insert the conditional segment (exact edit):

```tsx
export function StatusBarView(props: {
  model: string;
  workspace: string;
  theme: string;
  expandTools: boolean;
  tokens: BrainTokens;
  connectionText?: string | null;
}): React.ReactElement {
  void props.tokens; // reserved: segments gain token colors in later increments
  return (
    <Text dimColor>
      {props.workspace} · model {props.model} · theme {props.theme} · ! bash · / commands · ↑↓
      history · esc stop · ctrl+o {props.expandTools ? 'collapse' : 'expand'} tools
      {props.connectionText ? <Text color="yellow"> · {props.connectionText}</Text> : null} ·
      ctrl+c exit
    </Text>
  );
}
```

`AppShell.tsx` — three edits:

Import (beside the StatusBar import):

```tsx
import { connectionStatusText } from './connectionStatusLogic.js';
```

Dispose effect (directly after the controller `useMemo` at `:38-41`):

```tsx
  React.useEffect(() => () => controller.dispose(), [controller]);
```

Banner (`:201-203`) — yellow outage banner wins over the raw red error while reconnecting:

```tsx
      {snapshot.connection.status !== 'connected' ? (
        <Text color="yellow">⚠ Connection lost — reconnecting…</Text>
      ) : snapshot.connectionError !== undefined ? (
        <Text color="red">⚠ {snapshot.connectionError}</Text>
      ) : null}
```

Find where `<StatusBarView` renders and pass the new prop:

```tsx
        connectionText={connectionStatusText(snapshot.connection)}
```

- [ ] **Step 6: Run the full suite**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test
```

Expected: baseline + the 2 new logic tests; same 5 documented failures.

- [ ] **Step 7: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git add packages/brain-shell/src/ui/shell/connectionStatusLogic.ts packages/brain-shell/src/ui/shell/StatusBar.tsx packages/brain-shell/src/ui/shell/AppShell.tsx packages/brain-shell/src/test/ui/shell/connectionStatusLogic.test.ts
git commit -m "feat(shell): status-bar reconnect segment and outage banner

Inc 15 task 4: pure connectionStatusText projection feeds a yellow
'reconnecting (attempt N)' segment; the outage banner takes precedence
over the raw-error banner while disconnected; controller disposal rides
a React cleanup effect.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: Live PTY proof — `ptySmokeInc15.py`

**Files:**
- Create: `scripts/ptySmokeInc15.py`

**Interfaces:**
- Consumes: the whole increment, end to end, against a stub UDS daemon that is ABSENT at launch and starts listening mid-run.
- Produces: exit code 0 with every assertion PASSing.

Discipline carried from `scripts/ptySmokeInc6.py`: stub UDS daemon, `TIOCSWINSZ` before exec, discrete keystroke writes with ≥0.3 s pumps, ANSI-stripped matching, occurrence-count waits on the CUMULATIVE buffer. Because the buffer accumulates ink repaint history, absence can never be asserted — every check here is positive.

- [ ] **Step 1: Write the script**

Create `scripts/ptySmokeInc15.py`:

```python
#!/usr/bin/env python3
"""Increment 15 PTY smoke: daemon outage lifecycle end to end.

The stub daemon is ABSENT when the TUI launches. The first submit fails
with the 'Connection lost — reconnecting…' banner and the status bar
shows the reconnecting segment; the second submit queues visibly. Then
the daemon appears, the monitor restores, and the queued prompt
auto-fires through the normal turn pipeline and its answer freezes into
the transcript. Cumulative-buffer caveat: no absence checks — positive
assertions only.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc15-smoke.sock"
CONFIG_FILE = "/tmp/brain-inc15-smoke-config.json"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")
PKG_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell"

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

serve_started = threading.Event()

def serve():
    if os.path.exists(SOCK):
        os.remove(SOCK)
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(SOCK)
    srv.listen(8)
    serve_started.set()
    while True:
        conn, _ = srv.accept()
        def handle(conn=conn):
            fobj = conn.makefile("rw")
            try:
                for line in fobj:
                    req = json.loads(line)
                    rid = req.get("id")
                    act = req.get("action")
                    def reply(obj):
                        fobj.write(json.dumps(obj) + "\n")
                        fobj.flush()
                    if act == "v1/session/create":
                        reply({"id": rid, "status": "success",
                               "body": {"session_id": "stub-s15"}})
                    elif act == "v1/generation/stream":
                        # Minimal clean turn: greet and finish. Sequence
                        # numbers strictly consecutive.
                        reply({"type": "stream_start", "session_id": "stub-s15",
                               "sequence": 0})
                        time.sleep(0.2)
                        reply({"type": "token", "session_id": "stub-s15",
                               "token": "Daemon is back.", "sequence": 1})
                        time.sleep(0.2)
                        reply({"type": "finished", "session_id": "stub-s15",
                               "status": "completed", "sequence": 2})
                    else:
                        reply({"id": rid, "status": "success", "body": {}})
            except Exception:
                pass
            finally:
                try:
                    conn.close()
                except Exception:
                    pass
        threading.Thread(target=handle, daemon=True).start()

pid, fd = pty.fork()
if pid == 0:
    os.environ["BRAIN_SOCKET_PATH"] = SOCK
    os.environ["TERM"] = "xterm-256color"
    os.environ["BRAIN_CONFIG_PATH"] = CONFIG_FILE
    if os.path.exists(CONFIG_FILE):
        os.remove(CONFIG_FILE)
    os.chdir(PKG_DIR)
    os.execvp("bun", ["bun", "run", "src/main.tsx"])

fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

buf = b""
def pump(seconds):
    global buf
    end = time.time() + seconds
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.05)
        if fd in r:
            try:
                chunk = os.read(fd, 65536)
            except OSError:
                break
            if not chunk:
                break
            buf += chunk

def expect(label, needle, timeout=10.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        pump(0.1)
        if needle in clean(buf):
            print("PASS " + label)
            return True
    print("FAIL %s: %r not seen" % (label, needle))
    return False

ok = True

# ── Flow A: TUI boots fine with the daemon absent ──────────────────────────
ok &= expect("welcome-wordmark", "◆ BRAIN")
ok &= expect("launch-prompt", "❯")

# ── Flow B: first submit fails loudly and arms the monitor ────────────────
os.write(fd, b"hello there")
pump(0.3)
os.write(fd, b"\r")
ok &= expect("loss-banner", "Connection lost — reconnecting")
ok &= expect("statusbar-segment", "reconnecting (attempt")

# ── Flow C: the next prompt queues visibly instead of vanishing ───────────
os.write(fd, b"are you still there")
pump(0.3)
os.write(fd, b"\r")
ok &= expect("queued-row", "queued — will send on reconnect")

# ── Flow D: daemon appears -> restore -> queued prompt auto-fires ─────────
time.sleep(1.0)   # let a couple of failed probes land first
t = threading.Thread(target=serve, daemon=True)
t.start()
if not serve_started.wait(timeout=5):
    print("FAIL stub-server-up")
    ok = False
ok &= expect("replay-user-row", "are you still there")
ok &= expect("replay-answer", "Daemon is back.")

# ── Teardown ───────────────────────────────────────────────────────────────
os.write(fd, b"\x03")
time.sleep(0.5)
try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass

sys.exit(0 if ok else 1)
```

The server thread itself starts at Flow D (`t.start()`), NOT at import time — the TUI must launch against a genuinely absent socket.

- [ ] **Step 2: Run the smoke**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
python3 scripts/ptySmokeInc15.py
```

Expected: all 8 assertions PASS (`welcome-wordmark`, `launch-prompt`, `loss-banner`, `statusbar-segment`, `queued-row`, `stub-server-up` implied, `replay-user-row`, `replay-answer`), exit 0.

If `loss-banner` misses: the first probe may already have succeeded because a stale `/tmp/brain-inc15-smoke.sock` exists — remove it (the script does at server start, but check manually) and rerun.

- [ ] **Step 3: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git add scripts/ptySmokeInc15.py
git commit -m "test(smoke): Inc 15 outage-lifecycle PTY proof

Launches the TUI with no daemon, proves the loss banner and
reconnecting segment, queues a prompt visibly, then starts the stub
daemon and watches the queue auto-fire through the normal turn
pipeline. Positive assertions only — cumulative-buffer caveat applies.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: Full gates + finishing

**Files:** none created — verification only.

**Interfaces:**
- Consumes: everything.
- Produces: gate evidence for the finishing-a-development-branch menu.

- [ ] **Step 1: Full bun suite**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test
```

Expected: pre-Inc-15 baseline (263 tests: 258 pass / 5 documented fails) PLUS the 9 new tests (4 monitor + 2 probe + 3 controller + 2 logic... adjust count to actual) — i.e. no NEW failures, only the documented five.

Wait — exact arithmetic: 4 (monitor) + 2 (probe) + 3 (controller) + 2 (logic) = 11 new tests → expect **274 tests: 269 pass / 5 documented fails**.

- [ ] **Step 2: tsc — no new error classes on touched files**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bunx tsc --noEmit > "$CLAUDE_JOB_DIR/tmp/inc15-tsc.log" 2>&1; echo "exit:$?"
sed $'s/\x1b\[[0-9;]*m//g' "$CLAUDE_JOB_DIR/tmp/inc15-tsc.log" \
  | grep -E "^src/(state/(sessionController|connectionMonitor)|client/(UdsBrainBackendClient|probeDaemonSocket)|ui/shell/(AppShell|StatusBar|connectionStatusLogic))\.ts" \
  | grep -oE "error TS[0-9]+" | sort | uniq -c
```

Expected: ambient classes only (`TS2591` node typedefs, `TS2307` bun:test/bun modules — the same classes every existing wire/state file shows on `main`). If any OTHER class appears on a touched file (e.g. `TS2339`), fix it before proceeding — precedent: Inc 14's `.map((r) => r.text)` became a narrowed `kind` check.

For the two PRE-EXISTING modified files, compare against pristine `main` with the proven worktree probe:

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git worktree add --detach "$CLAUDE_JOB_DIR/tmp/inc15-probe" origin/main
ln -s /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/node_modules \
  "$CLAUDE_JOB_DIR/tmp/inc15-probe/packages/brain-shell/node_modules"
cd "$CLAUDE_JOB_DIR/tmp/inc15-probe/packages/brain-shell"
bunx tsc --noEmit > "$CLAUDE_JOB_DIR/tmp/inc15-tsc-main.log" 2>&1
# Compare per-file counts for sessionController/UdsBrainBackendClient:
for f in inc15-tsc-main.log inc15-tsc.log; do
  echo "== $f =="
  sed $'s/\x1b\[[0-9;]*m//g' "$CLAUDE_JOB_DIR/tmp/$f" \
    | grep -E "^src/(client/UdsBrainBackendClient|state/sessionController)\.ts" \
    | grep -oE "error TS[0-9]+" | sort | uniq -c
done
```

Then clean up:

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
rm "$CLAUDE_JOB_DIR/tmp/inc15-probe/packages/brain-shell/node_modules"
git worktree remove --force "$CLAUDE_JOB_DIR/tmp/inc15-probe"
git worktree prune
```

Expected: identical counts both sides for those two files (baseline was `2×TS2304, 1×TS2353, 4×TS2591` on UdsBrainBackendClient; sessionController had none).

- [ ] **Step 3: Vendor scan on the increment diff**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
BASE=$(git merge-base HEAD origin/main)
git diff "$BASE"..HEAD -- crates daemon packages scripts docs \
  | grep '^+' | grep -icE "anthropic|api\.anthropic|claude"
```

Expected: `0`.

- [ ] **Step 4: Cargo workspace (Rust untouched, prove it anyway)**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test --workspace' 2>&1 | tail -15
```

Expected: green EXCEPT possibly `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers` (sole permitted failure).

- [ ] **Step 5: Rerun the Inc 15 PTY smoke + regression smoke**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
rm -f /tmp/brain-inc15-smoke.sock
python3 scripts/ptySmokeInc15.py
python3 scripts/ptySmokeInc6.py
```

Expected: inc15 all-PASS exit 0. inc6 all 16 assertions PASS (fixtures byte-drift from capture nondeterminism is expected and MUST NOT be committed — restore with `git checkout -- packages/brain-shell/src/test/fixtures/` if `git status --porcelain packages/brain-shell/src/test/fixtures/` lists anything).

- [ ] **Step 6: Finishing**

Announce finishing-a-development-branch, verify tests (Steps 1–5 ARE the verification), detect environment (normal repo — standard menu), base branch `main`, and present exactly:

```
Implementation complete. What would you like to do?

1. Merge back to main locally
2. Push and create a Pull Request
3. Keep the branch as-is (I'll handle it later)

Which option?
```

On Option 1: `git checkout main && git pull --ff-only && git merge feature/brain-shell-inc15-daemon-reconnect`, confirm fast-forward hash equals the gated tree tip, delete the branch, report `[ahead N]` state. Pushes require explicit user approval.

---

## Self-Review (completed during planning)

1. **Spec coverage:** D1 queue → Task 3(f)/(3e); D2 backoff → Task 1; D3 clean mid-turn failure → Task 3(f)/(g) + test 2; D4 placement → Task 1/3 structure; probe → Task 2; status bar/banner/dispose → Task 4; live proof → Task 5; gates → Task 6; header-invariant retirement → Task 2 Step 4. No gaps found.
2. **Placeholder scan:** none — every step carries full code or exact commands. The Task 5 drafting correction is called out explicitly with replacement code, not left as a TBD.
3. **Type consistency:** `ConnectionState` shape identical across Tasks 1/3/4; `nextDelayMs(attempt)` 1-based everywhere including the delay-before-probe-N loop; `probeDaemonSocket(path, timeoutMs?)` matches controller usage; `CONNECTION_LOSS_ROW`/`QUEUED_ROW` exported once (Task 3) and reused verbatim; `dispose()` name matches AppShell effect.
