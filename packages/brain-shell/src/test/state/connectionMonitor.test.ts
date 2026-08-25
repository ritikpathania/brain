import { describe, it, expect } from 'bun:test';
import {
  ConnectionMonitor,
  nextDelayMs,
  type ConnectionState,
} from '../../state/connectionMonitor.js';

/** Flush pending microtasks and due timers before asserting. */
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
