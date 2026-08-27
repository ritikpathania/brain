/**
 * Inc 29: Unit tests for /rename and /fork commands and SessionController mutation methods.
 */
import { describe, it, expect, mock } from 'bun:test';
import { getCommand } from '../../commands/registry.js';
import '../../commands/builtin.js';
import { SessionController } from '../../state/sessionController.js';
import type { BrainBackendClient, BrainSession } from '../../client/BrainBackendClient.js';

describe('/rename command specification', () => {
  it('returns usage text when no arguments provided', () => {
    const cmd = getCommand('rename');
    expect(cmd).toBeDefined();
    const res = cmd!.run({ args: [] });
    expect(res).toEqual({ type: 'text', value: 'Usage: /rename <title>' });
  });

  it('returns rename action with args when title provided', () => {
    const cmd = getCommand('rename');
    expect(cmd).toBeDefined();
    const res = cmd!.run({ args: ['Alpha', 'Compiler', 'Session'] });
    expect(res).toEqual({
      type: 'action',
      action: 'rename',
      args: ['Alpha', 'Compiler', 'Session'],
    });
  });
});

describe('/fork command specification', () => {
  it('returns fork action with empty args when no title provided', () => {
    const cmd = getCommand('fork');
    expect(cmd).toBeDefined();
    const res = cmd!.run({ args: [] });
    expect(res).toEqual({
      type: 'action',
      action: 'fork',
      args: [],
    });
  });

  it('returns fork action with title args when provided', () => {
    const cmd = getCommand('fork');
    expect(cmd).toBeDefined();
    const res = cmd!.run({ args: ['Forked', 'Beta', 'Branch'] });
    expect(res).toEqual({
      type: 'action',
      action: 'fork',
      args: ['Forked', 'Beta', 'Branch'],
    });
  });
});

describe('SessionController mutation methods', () => {
  function makeMockClient(overrides: Partial<BrainBackendClient> = {}): BrainBackendClient {
    return {
      streamText: mock(() => (async function* () {})()),
      retrieveContext: mock(async () => ({ memories: [] })),
      createSession: mock(async (req) => ({
        sessionId: 'sess-new-1',
        title: req?.title || 'New Session',
        createdAtMs: Date.now(),
      })),
      loadSession: mock(async (sid) => ({
        session: {
          id: sid,
          title: 'Loaded Session',
          createdAtMs: Date.now(),
          updatedAtMs: Date.now(),
          pinned: false,
          archived: false,
          messages: [
            { id: 'm1', role: 'user', content: 'Turn 1', timestamp: 100 },
            { id: 'm2', role: 'assistant', content: 'Reply 1', timestamp: 200 },
          ],
        } as BrainSession,
      })),
      forkSession: mock(async (req) => ({
        newSessionId: 'sess-forked-2',
        sourceSessionId: req.sourceSessionId,
        clonedMessagesCount: 2,
      })),
      archiveSession: mock(async () => {}),
      restoreSession: mock(async () => {}),
      renameSession: mock(async () => {}),
      pinSession: mock(async () => {}),
      listSessions: mock(async () => []),
      ...overrides,
    };
  }

  it('renameCurrentSession renames existing active session and records notice', async () => {
    const client = makeMockClient();
    const controller = new SessionController(client);
    // Resume an existing session first
    await controller.resumeSession('sess-1');

    await controller.renameCurrentSession('Updated Alpha Title');

    expect(client.renameSession).toHaveBeenCalledWith('sess-1', 'Updated Alpha Title');
    const snapshot = controller.getSnapshot();
    const lastRow = snapshot.rows[snapshot.rows.length - 1];
    expect(lastRow.kind).toBe('system');
    if (lastRow.kind === 'system') {
      expect(lastRow.text).toContain('Session renamed to “Updated Alpha Title”');
    }
  });

  it('renameCurrentSession creates a session if none is active', async () => {
    const client = makeMockClient();
    const controller = new SessionController(client);

    await controller.renameCurrentSession('First Title');

    expect(client.createSession).toHaveBeenCalledWith({ title: 'First Title' });
    expect(controller.activeSessionId).toBe('sess-new-1');
  });

  it('forkCurrentSession forks active session, switches sessionId, and replays history', async () => {
    const client = makeMockClient({
      loadSession: mock(async (sid) => ({
        session: {
          id: sid,
          title: 'Forked Beta Session',
          createdAtMs: Date.now(),
          updatedAtMs: Date.now(),
          pinned: false,
          archived: false,
          messages: [
            { id: 'm1', role: 'user', content: 'Parent turn', timestamp: 100 },
            { id: 'm2', role: 'assistant', content: 'Parent answer', timestamp: 200 },
          ],
        } as BrainSession,
      })),
    });
    const controller = new SessionController(client);
    await controller.resumeSession('sess-parent');

    const forkedId = await controller.forkCurrentSession('Forked Beta Session');

    expect(forkedId).toBe('sess-forked-2');
    expect(controller.activeSessionId).toBe('sess-forked-2');
    expect(client.forkSession).toHaveBeenCalledWith({
      sourceSessionId: 'sess-parent',
      newTitle: 'Forked Beta Session',
    });

    const snapshot = controller.getSnapshot();
    // Verify rows contain replayed parent turn + assistant answer + system forked notice
    expect(snapshot.rows.length).toBe(3);
    expect(snapshot.rows[0].kind).toBe('user');
    expect(snapshot.rows[1].kind).toBe('assistant');
    expect(snapshot.rows[2].kind).toBe('system');
    if (snapshot.rows[2].kind === 'system') {
      expect(snapshot.rows[2].text).toContain('Forked to “Forked Beta Session” (2 turns cloned)');
    }
  });
});
