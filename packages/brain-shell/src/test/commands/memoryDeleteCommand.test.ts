/**
 * Inc 31: Unit tests for /memory delete command and SessionController.deleteMemoryNode.
 */
import { describe, it, expect, mock } from 'bun:test';
import { getCommand } from '../../commands/registry.js';
import '../../commands/builtin.js';
import { SessionController } from '../../state/sessionController.js';
import type { BrainBackendClient } from '../../client/BrainBackendClient.js';

describe('/memory command specification', () => {
  it('opens overlay when invoked without arguments', () => {
    const cmd = getCommand('memory');
    expect(cmd).toBeDefined();
    const res = cmd!.run({ args: [] });
    expect(res).toEqual({ type: 'overlay', overlay: 'memory' });
  });

  it('returns usage text when delete subcommand has no target ID', () => {
    const cmd = getCommand('memory');
    expect(cmd).toBeDefined();
    const res = cmd!.run({ args: ['delete'] });
    expect(res).toEqual({ type: 'text', value: 'Usage: /memory delete <node_id>' });
  });

  it('returns memory-delete action when target ID is provided', () => {
    const cmd = getCommand('memory');
    expect(cmd).toBeDefined();
    const res = cmd!.run({ args: ['delete', 'node-uuid-1234'] });
    expect(res).toEqual({
      type: 'action',
      action: 'memory-delete',
      args: ['node-uuid-1234'],
    });
  });
});

describe('SessionController.deleteMemoryNode', () => {
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
      ...overrides,
    };
  }

  it('invokes client deleteMemory and posts notice on success', async () => {
    const client = makeMockClient();
    const controller = new SessionController(client);

    const ok = await controller.deleteMemoryNode('target-node-789');
    expect(ok).toBe(true);
    expect(client.deleteMemory).toHaveBeenCalledWith('target-node-789');

    const snap = controller.getSnapshot();
    const lastRow = snap.rows[snap.rows.length - 1];
    expect(lastRow.kind).toBe('system');
    if (lastRow.kind === 'system') {
      expect(lastRow.text).toContain('Deleted memory node target-node-789');
    }
  });

  it('handles client failure gracefully and posts error notice', async () => {
    const client = makeMockClient({
      deleteMemory: mock(async () => {
        throw new Error('Node not found');
      }),
    });
    const controller = new SessionController(client);

    const ok = await controller.deleteMemoryNode('missing-node-000');
    expect(ok).toBe(false);

    const snap = controller.getSnapshot();
    const lastRow = snap.rows[snap.rows.length - 1];
    expect(lastRow.kind).toBe('system');
    if (lastRow.kind === 'system') {
      expect(lastRow.text).toContain('Could not delete memory node: Node not found');
    }
  });

  it('rejects empty node ID and warns with usage', async () => {
    const client = makeMockClient();
    const controller = new SessionController(client);

    const ok = await controller.deleteMemoryNode('   ');
    expect(ok).toBe(false);
    expect(client.deleteMemory).not.toHaveBeenCalled();

    const snap = controller.getSnapshot();
    const lastRow = snap.rows[snap.rows.length - 1];
    expect(lastRow.kind).toBe('system');
    if (lastRow.kind === 'system') {
      expect(lastRow.text).toBe('Usage: /memory delete <node_id>');
    }
  });
});
