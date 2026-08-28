/**
 * Inc 35: Unit tests for /session delete command and SessionController.deleteSession.
 */
import { describe, it, expect, mock } from 'bun:test';
import { getCommand } from '../../commands/registry.js';
import '../../commands/builtin.js';
import { SessionController } from '../../state/sessionController.js';
import type { BrainBackendClient, BrainSessionSummary } from '../../client/BrainBackendClient.js';

describe('/session command specification', () => {
  it('returns usage text when /session is invoked with invalid subcommand', () => {
    const cmd = getCommand('session');
    expect(cmd).toBeDefined();
    const res = cmd!.run({ args: [] });
    expect(res).toEqual({
      type: 'text',
      value: 'Usage: /session <delete|pin|unpin|archive|restore> [id_or_prefix]',
    });
  });

  it('returns usage text when /session delete has no target ID', () => {
    const cmd = getCommand('session');
    expect(cmd).toBeDefined();
    const res = cmd!.run({ args: ['delete'] });
    expect(res).toEqual({ type: 'text', value: 'Usage: /session delete <id_or_prefix>' });
  });

  it('returns session-delete action when target ID is provided to /session delete', () => {
    const cmd = getCommand('session');
    expect(cmd).toBeDefined();
    const res = cmd!.run({ args: ['delete', '01M123456789'] });
    expect(res).toEqual({
      type: 'action',
      action: 'session-delete',
      args: ['01M123456789'],
    });
  });

  it('returns session-pin action when /session pin is used', () => {
    const cmd = getCommand('session');
    expect(cmd).toBeDefined();
    const res = cmd!.run({ args: ['pin', '01M123456789'] });
    expect(res).toEqual({
      type: 'action',
      action: 'session-pin',
      args: ['01M123456789'],
    });
  });

  it('returns session-restore action when /session restore has target', () => {
    const cmd = getCommand('session');
    expect(cmd).toBeDefined();
    const res = cmd!.run({ args: ['restore', '01M123456789'] });
    expect(res).toEqual({
      type: 'action',
      action: 'session-restore',
      args: ['01M123456789'],
    });
  });

  it('returns session-delete action when /resume delete is used', () => {
    const cmd = getCommand('resume');
    expect(cmd).toBeDefined();
    const res = cmd!.run({ args: ['delete', '01M123456789'] });
    expect(res).toEqual({
      type: 'action',
      action: 'session-delete',
      args: ['01M123456789'],
    });
  });
});

describe('SessionController.deleteSession', () => {
  const SESSIONS: BrainSessionSummary[] = [
    { id: '01M123AAA00000000000000000', title: 'Cryogenic Circuits', updatedAtMs: 1000, pinned: false, archived: false },
    { id: '01M123BBB00000000000000000', title: 'Trapped Ion Traps', updatedAtMs: 2000, pinned: false, archived: false },
    { id: '01M999CCC00000000000000000', title: 'Photonic Logic', updatedAtMs: 3000, pinned: false, archived: false },
  ];

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
      listSessions: mock(async () => SESSIONS),
      deleteMemory: mock(async (nodeId: string) => ({ success: true, nodeId })),
      deleteSession: mock(async (sessionId: string) => ({ success: true, sessionId })),
      ...overrides,
    };
  }

  it('deletes an inactive session by exact ID and posts notice', async () => {
    const deleteSpy = mock(async (sessionId: string) => ({ success: true, sessionId }));
    const client = makeMockClient({ deleteSession: deleteSpy });
    const controller = new SessionController(client);

    const ok = await controller.deleteSession('01M999CCC00000000000000000');
    expect(ok).toBe(true);
    expect(deleteSpy).toHaveBeenCalledWith('01M999CCC00000000000000000');

    const snap = controller.getSnapshot();
    const lastRow = snap.rows[snap.rows.length - 1];
    expect(lastRow.kind).toBe('system');
    if (lastRow.kind === 'system') {
      expect(lastRow.text).toContain('Deleted session “Photonic Logic” (01M999CC)');
    }
  });

  it('deletes by unique prefix', async () => {
    const deleteSpy = mock(async (sessionId: string) => ({ success: true, sessionId }));
    const client = makeMockClient({ deleteSession: deleteSpy });
    const controller = new SessionController(client);

    const ok = await controller.deleteSession('01M999');
    expect(ok).toBe(true);
    expect(deleteSpy).toHaveBeenCalledWith('01M999CCC00000000000000000');
  });

  it('handles active session deletion by resetting session ID and rows', async () => {
    const deleteSpy = mock(async (sessionId: string) => ({ success: true, sessionId }));
    const client = makeMockClient({ deleteSession: deleteSpy });
    const controller = new SessionController(client);

    // Assume active session
    await controller.resumeSession('01M999CCC00000000000000000');
    expect(controller.activeSessionId).toBe('01M999CCC00000000000000000');

    const ok = await controller.deleteSession('01M999CCC00000000000000000');
    expect(ok).toBe(true);

    expect(controller.activeSessionId).toBeUndefined();
    const snap = controller.getSnapshot();
    expect(snap.rows.length).toBe(1);
    expect(snap.rows[0].kind).toBe('system');
    if (snap.rows[0].kind === 'system') {
      expect(snap.rows[0].text).toContain('Deleted active session “Photonic Logic”');
    }
  });

  it('surfaces ambiguous candidates notice when prefix matches multiple sessions', async () => {
    const deleteSpy = mock(async (sessionId: string) => ({ success: true, sessionId }));
    const client = makeMockClient({ deleteSession: deleteSpy });
    const controller = new SessionController(client);

    const ok = await controller.deleteSession('01M123');
    expect(ok).toBe(false);
    expect(deleteSpy).not.toHaveBeenCalled();

    const snap = controller.getSnapshot();
    const lastRow = snap.rows[snap.rows.length - 1];
    if (lastRow.kind === 'system') {
      expect(lastRow.text).toContain('Ambiguous session prefix “01M123” matches:');
    }
  });

  it('surfaces unknown notice when target does not match', async () => {
    const deleteSpy = mock(async (sessionId: string) => ({ success: true, sessionId }));
    const client = makeMockClient({ deleteSession: deleteSpy });
    const controller = new SessionController(client);

    const ok = await controller.deleteSession('nonexistent_session_target');
    expect(ok).toBe(false);
    expect(deleteSpy).not.toHaveBeenCalled();

    const snap = controller.getSnapshot();
    const lastRow = snap.rows[snap.rows.length - 1];
    if (lastRow.kind === 'system') {
      expect(lastRow.text).toContain('No session found matching “nonexistent_session_target”.');
    }
  });
});
