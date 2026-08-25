import { describe, it, expect } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import type {
  BrainBackendClient,
  BrainStreamChunk,
  BrainGenerationRequest,
} from '../../client/BrainBackendClient.js';

/** Fake client: replays scripted chunks. Same idiom as sessionController.test.ts. */
function fakeClient(chunks: BrainStreamChunk[]) {
  const client = {
    async createSession() {
      return { sessionId: 'stub-session-1', title: 'stub', createdAtMs: 0 };
    },
    async *streamText(request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
      for (const c of chunks) yield c;
    },
  } as unknown as BrainBackendClient;
  return client;
}

describe('Inc 13: thinking lifecycle completion', () => {
  it('freezes the thinking row with the daemon-reported duration', async () => {
    const ctl = new SessionController(
      fakeClient([
        { type: 'thinking_start' },
        { type: 'thinking', thinking: 'pondering…' },
        { type: 'thinking_end', durationMs: 1200 },
        { type: 'token', token: 'Answer.' },
        { type: 'finished', status: 'completed' },
      ]),
    );
    await ctl.submit('deep question');
    const row = ctl
      .getSnapshot()
      .rows.find((r) => r.kind === 'thinking');
    expect(row).toMatchObject({ kind: 'thinking', text: 'pondering…', durationMs: 1200 });
  });

  it('falls back to a locally measured duration when the wire omits it', async () => {
    const ctl = new SessionController(
      fakeClient([
        { type: 'thinking_start' },
        { type: 'thinking', thinking: 'slow thought' },
        { type: 'thinking_end' },
        { type: 'finished', status: 'completed' },
      ]),
    );
    await ctl.submit('old daemon');
    const row = ctl
      .getSnapshot()
      .rows.find((r) => r.kind === 'thinking');
    if (row?.kind !== 'thinking') throw new Error('no thinking row');
    expect(typeof row.durationMs).toBe('number');
    expect(row.durationMs as number).toBeGreaterThanOrEqual(0);
  });
});
