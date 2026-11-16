import { describe, expect, test } from 'bun:test';

import { SessionClient, SessionNotFoundError } from '../../src/brain/sessions.js';
import { UdsResponseError } from '../../src/brain/transport/udsClient.js';

describe('Brain-authoritative sessions', () => {
  test('does not synthesize IDs and projects only daemon-returned history', async () => {
    const calls: string[] = [];
    const session = new SessionClient({
      request: async (action) => {
        calls.push(action);
        if (action === 'v1/session/create') return { session_id: '01ARZ3NDEKTSV4RRFFQ69G5FAV' };
        return {
          session: {
            sessionId: '01ARZ3NDEKTSV4RRFFQ69G5FAV',
            title: 'Daemon session',
            messages: [{ role: 'user', content: 'persisted by Brain' }],
          },
        };
      },
    });

    const created = await session.create({ title: 'requested title' });
    const loaded = await session.load(created.sessionId);

    expect(created.sessionId).toBe('01ARZ3NDEKTSV4RRFFQ69G5FAV');
    expect(loaded.messages).toEqual([{ role: 'user', content: 'persisted by Brain' }]);
    expect(calls).toEqual(['v1/session/create', 'v1/session/load']);
  });

  test('retains daemon session-not-found as a typed frontend outcome', async () => {
    const session = new SessionClient({
      request: async () => {
        throw new UdsResponseError('session_not_found', 'daemon did not find the session');
      },
    });

    await expect(session.load('01ARZ3NDEKTSV4RRFFQ69G5FAV')).rejects.toBeInstanceOf(SessionNotFoundError);
  });

  test('projects daemon session-list summaries without inventing transcript messages', async () => {
    const session = new SessionClient({
      request: async () => ({
        sessions: [{
          sessionId: '01ARZ3NDEKTSV4RRFFQ69G5FAV',
          title: 'Daemon summary',
          messageCount: 2,
          createdAtMs: 1,
          updatedAtMs: 2,
          pinned: false,
          archived: false,
          system_prompt: null,
        }],
        total: 1,
      }),
    });

    await expect(session.list()).resolves.toEqual([{
      sessionId: '01ARZ3NDEKTSV4RRFFQ69G5FAV',
      title: 'Daemon summary',
      messageCount: 2,
      pinned: false,
      archived: false,
    }]);
  });
});
