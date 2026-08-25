import { describe, it, expect } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import type {
  BrainBackendClient,
  BrainStreamChunk,
  BrainGenerationRequest,
} from '../../client/BrainBackendClient.js';

/** Fake client whose stream parks on a gate until the test releases it. */
function gatedClient(): { client: BrainBackendClient; release: () => void } {
  let release!: () => void;
  const gate = new Promise<void>((r) => {
    release = r;
  });
  const client = {
    async createSession() {
      return { sessionId: 'stub-session-1', title: 'stub', createdAtMs: 0 };
    },
    async *streamText(_request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
      await gate;
      yield { type: 'token', token: 'done' };
      yield { type: 'finished', status: 'completed' };
    },
  } as unknown as BrainBackendClient;
  return { client, release };
}

describe('Inc 14: busy-submit feedback', () => {
  it('notices instead of silently dropping a submit during a live turn', async () => {
    const { client, release } = gatedClient();
    const ctl = new SessionController(client);
    const first = ctl.submit('first');
    // The first submit set busy synchronously; a second one must say so —
    // same contract as runShellCommand and resumeSession — not vanish.
    await ctl.submit('second');
    const notice = ctl.getSnapshot().rows.find((r) => r.kind === 'system');
    expect(notice?.text).toContain('Busy — wait');
    // The ignored submit left no phantom user turn behind.
    const userTexts = ctl
      .getSnapshot()
      .rows.filter((r) => r.kind === 'user')
      .map((r) => r.text);
    expect(userTexts).toEqual(['first']);
    release();
    await first;
  });
});
