import { describe, expect, test } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import type {
  BrainBackendClient,
  BrainGenerationRequest,
  BrainSession,
  BrainSessionSummary,
  BrainStreamChunk,
} from '../../client/BrainBackendClient.js';

function resumeFake(session: BrainSession | Error, summaries: BrainSessionSummary[] = []) {
  const client = {
    async createSession() {
      return { sessionId: 'stub-session-1', title: 'stub', createdAtMs: 0 };
    },
    async *streamText(_r: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
      yield { type: 'finished', status: 'completed' };
    },
    async listSessions(): Promise<BrainSessionSummary[]> {
      return summaries;
    },
    async loadSession(id: string): Promise<{ session: BrainSession }> {
      if (session instanceof Error) throw session;
      return { session: { ...session, id } };
    },
  } as unknown as BrainBackendClient;
  return client;
}

const SESSION: BrainSession = {
  id: 'old-1',
  title: 'Refactor graph indexer',
  createdAtMs: 0,
  updatedAtMs: 0,
  pinned: false,
  archived: false,
  messages: [
    { id: 'm1', role: 'user', content: 'hello' },
    { id: 'm2', role: 'assistant', content: 'world' },
  ],
};

describe('SessionController resume', () => {
  test('resume adopts the session id and replays messages as rows', async () => {
    const ctl = new SessionController(resumeFake(SESSION));
    await ctl.resumeSession('old-1');
    const snap = ctl.getSnapshot();
    expect(snap.busy).toBe(false);
    expect(snap.rows.map((r) => r.kind)).toEqual(['user', 'assistant', 'system']);
    expect(snap.rows[0]).toMatchObject({ kind: 'user', text: 'hello' });
    expect(JSON.stringify(snap.rows.at(-1))).toContain('Resumed');
  });

  test('failed loads surface a system notice, not a crash', async () => {
    const ctl = new SessionController(resumeFake(new Error('socket gone')));
    await ctl.resumeSession('old-1');
    expect(JSON.stringify(ctl.getSnapshot().rows)).toContain('Could not resume');
  });

  test('listSessions passes through to the client', async () => {
    const summaries: BrainSessionSummary[] = [
      { id: 'x', title: 'X', updatedAtMs: 1, pinned: false, archived: false },
    ];
    const ctl = new SessionController(resumeFake(SESSION, summaries));
    expect(await ctl.listSessions()).toEqual(summaries);
  });
});
