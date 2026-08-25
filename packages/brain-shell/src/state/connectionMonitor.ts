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
