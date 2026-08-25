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

/**
 * Instant-for-the-monitor but MACROTASK-YIELDING backoff clock. A pure
 * Promise.resolve() delay lets a false-probe loop spin the microtask
 * queue forever (the exact hang the ConnectionMonitor tests exposed):
 * bun drains microtasks before ANY timer fires, starving the test
 * runner itself. One setTimeout turn per attempt keeps every
 * macrotask — waitFor's poller included — breathing.
 */
const instantDelay = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

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
      async () => up, // probeOverride: poll the flag
      instantDelay, // delayOverride: yielding backoff clock
    );
    await ctl.submit('first question'); // fails -> reconnecting arms
    expect(ctl.getSnapshot().connection.status).toBe('reconnecting');
    await ctl.submit('second question'); // offline -> queued row
    await ctl.submit('third question'); // offline -> queued behind it
    const textsOf = (kind: 'system' | 'error'): string[] =>
      ctl.getSnapshot().rows.filter((r) => r.kind === kind).map((r) => r.text);
    expect(textsOf('system')).toEqual([
      'queued — will send on reconnect',
      'queued — will send on reconnect',
    ]);
    expect(textsOf('error')).toContain('Connection lost — reconnecting…');
    // Restore: the monitor restores, then BOTH queued prompts replay in
    // order. (The failed first prompt is NOT retried — D3 fails that
    // turn cleanly; only offline-typed input is held.) Markdown is
    // matched with includes(): the freeze path re-appends undrained
    // typewriter remainder on instantly-completing stub streams — a
    // pre-existing quirk outside Inc 15's scope.
    client.setUp(true);
    up = true;
    await waitFor(() => ctl.getSnapshot().connection.status === 'connected');
    await waitFor(
      () =>
        ctl
          .getSnapshot()
          .rows.filter((r) => r.kind === 'assistant' && r.markdown.includes('back-online'))
          .length === 2,
    );
    const userRows = ctl
      .getSnapshot()
      .rows.filter((r) => r.kind === 'user')
      .map((r) => r.text);
    expect(userRows).toEqual(['first question', 'second question', 'third question']);
  });

  it('fails a mid-turn drop cleanly and arms the monitor', async () => {
    const client = {
      async createSession() {
        return { sessionId: 'stub-session-15', title: 'stub', createdAtMs: 0 };
      },
      async *streamText(): AsyncIterable<BrainStreamChunk> {
        yield { type: 'token', token: 'partial answer' };
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
    expect(
      snap.rows.some((r) => r.kind === 'assistant' && r.markdown.includes('partial answer')),
    ).toBe(true);
    // …the loss copy replaces the raw wire error…
    expect(
      snap.rows.some((r) => r.kind === 'error' && r.text === 'Connection lost — reconnecting…'),
    ).toBe(true);
    // …and the monitor armed.
    expect(snap.connection.status).toBe('reconnecting');
    expect(snap.busy).toBe(false);
    ctl.dispose(); // stop the deliberately-never-restoring monitor loop
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
    ctl.dispose(); // nothing armed, but keep the hygiene explicit
  });
});
