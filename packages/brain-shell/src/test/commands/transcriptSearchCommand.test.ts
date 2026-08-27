/**
 * Inc 32: Unit tests for /search command and SessionController.searchTranscripts.
 */
import { describe, it, expect, mock } from 'bun:test';
import { getCommand } from '../../commands/registry.js';
import '../../commands/builtin.js';
import { SessionController } from '../../state/sessionController.js';
import type { BrainBackendClient, TranscriptSearchMatch } from '../../client/BrainBackendClient.js';

describe('/search command specification', () => {
  it('returns usage text when no query provided', () => {
    const cmd = getCommand('search');
    expect(cmd).toBeDefined();
    const res = cmd!.run({ args: [] });
    expect(res).toEqual({ type: 'text', value: 'Usage: /search <query>' });
  });

  it('returns search action with args when query provided', () => {
    const cmd = getCommand('search');
    expect(cmd).toBeDefined();
    const res = cmd!.run({ args: ['quantum', 'superposition'] });
    expect(res).toEqual({
      type: 'action',
      action: 'search',
      args: ['quantum', 'superposition'],
    });
  });
});

describe('SessionController.searchTranscripts', () => {
  function makeMockClient(overrides: Partial<BrainBackendClient> = {}): BrainBackendClient {
    return {
      streamText: mock(() => (async function* () {})()),
      retrieveContext: mock(async () => ({ memories: [] })),
      createSession: mock(async () => ({ sessionId: 'sess-1', title: 'New', createdAtMs: Date.now() })),
      loadSession: mock(async (sid) => ({ session: { id: sid, title: 'Loaded', createdAtMs: 1, updatedAtMs: 1, pinned: false, archived: false, messages: [] } })),
      forkSession: mock(async () => ({ newSessionId: 'sess-2', sourceSessionId: 'sess-1', clonedMessagesCount: 0 })),
      archiveSession: mock(async () => {}),
      restoreSession: mock(async () => {}),
      renameSession: mock(async () => {}),
      pinSession: mock(async () => {}),
      listSessions: mock(async () => []),
      deleteMemory: mock(async (nodeId: string) => ({ success: true, nodeId })),
      searchTranscripts: mock(async (input) => ({
        matches: [
          {
            id: 'msg-1',
            kind: 'message',
            sessionId: 'sess-alpha',
            title: 'user',
            body: `Matched query ${input.query}`,
            snippet: `Matched query ${input.query}`,
            role: 'user',
          } as TranscriptSearchMatch,
        ],
        total: 1,
      })),
      ...overrides,
    };
  }

  it('returns empty array when query is empty or whitespace', async () => {
    const client = makeMockClient();
    const controller = new SessionController(client);

    const res = await controller.searchTranscripts('   ');
    expect(res).toEqual([]);
    expect(client.searchTranscripts).not.toHaveBeenCalled();
  });

  it('dispatches search to client and returns matches', async () => {
    const client = makeMockClient();
    const controller = new SessionController(client);

    const matches = await controller.searchTranscripts('Josephson', 10);
    expect(client.searchTranscripts).toHaveBeenCalledWith({ query: 'Josephson', limit: 10 });
    expect(matches.length).toBe(1);
    expect(matches[0].sessionId).toBe('sess-alpha');
    expect(matches[0].body).toBe('Matched query Josephson');
  });

  it('handles client failure gracefully and returns empty array', async () => {
    const client = makeMockClient({
      searchTranscripts: mock(async () => {
        throw new Error('Database connection failed');
      }),
    });
    const controller = new SessionController(client);

    const matches = await controller.searchTranscripts('quantum');
    expect(matches).toEqual([]);
  });
});
