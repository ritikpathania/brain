import { describe, expect, test } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import type {
  BrainBackendClient,
  BrainGenerationRequest,
  BrainStreamChunk,
} from '../../client/BrainBackendClient.js';

function scriptFake(chunks: BrainStreamChunk[]) {
  const client = {
    async createSession() {
      return { sessionId: 'stub-session-1', title: 'stub', createdAtMs: 0 };
    },
    async *streamText(_r: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
      for (const c of chunks) yield c;
    },
  } as unknown as BrainBackendClient;
  return client;
}

const PERM_SCRIPT: BrainStreamChunk[] = [
  {
    type: 'permission_request',
    callId: 'call_9',
    toolName: 'bash',
    input: { command: 'rm -rf build' },
    reason: 'destructive',
  },
  { type: 'finished', status: 'completed' },
];

describe('SessionController permission requests', () => {
  test('a permission_request chunk parks a pending dialog in the snapshot', async () => {
    const ctl = new SessionController(scriptFake(PERM_SCRIPT));
    await ctl.submit('clean this up');
    expect(ctl.getSnapshot().permission).toEqual({
      callId: 'call_9',
      toolName: 'bash',
      input: { command: 'rm -rf build' },
      reason: 'destructive',
    });
  });

  test('grant clears the pending dialog and posts an Allowed notice', async () => {
    const ctl = new SessionController(scriptFake(PERM_SCRIPT));
    await ctl.submit('clean this up');
    ctl.resolvePermission('call_9', true);
    const snap = ctl.getSnapshot();
    expect(snap.permission).toBeUndefined();
    expect(JSON.stringify(snap.rows)).toContain('Allowed bash');
    expect(JSON.stringify(snap.rows)).not.toContain('"status":"denied"');
  });

  test('deny posts a Denied notice', async () => {
    const ctl = new SessionController(
      scriptFake([
        { type: 'permission_request', callId: 'call_9', toolName: 'bash', input: {} },
        { type: 'finished', status: 'completed' },
      ]),
    );
    await ctl.submit('go');
    ctl.resolvePermission('call_9', false);
    expect(ctl.getSnapshot().permission).toBeUndefined();
    expect(JSON.stringify(ctl.getSnapshot().rows)).toContain('Denied bash');
  });

  test('deny flips a preceding tool card to denied by callId', async () => {
    const ctl = new SessionController(
      scriptFake([
        { type: 'tool_use', toolUse: { id: 'call_9', name: 'bash', input: { command: 'ls' } } },
        { type: 'permission_request', callId: 'call_9', toolName: 'bash', input: { command: 'ls' } },
        { type: 'finished', status: 'completed' },
      ]),
    );
    await ctl.submit('go');
    ctl.resolvePermission('call_9', false);
    const toolRow = ctl.getSnapshot().rows.find((r) => r.kind === 'tool');
    expect(toolRow && toolRow.kind === 'tool' && toolRow.tool.status).toBe('denied');
  });

  test('resolving an unknown callId is a no-op', () => {
    const ctl = new SessionController(scriptFake([]));
    ctl.resolvePermission('ghost', true);
    expect(ctl.getSnapshot().permission).toBeUndefined();
  });

  test('resolution travels to backends that support the wire call', async () => {
    const base = scriptFake(PERM_SCRIPT) as Record<string, unknown>;
    const resolutions: Array<{ callId: string; granted: boolean }> = [];
    base.resolveToolPermission = (callId: string, granted: boolean) => {
      resolutions.push({ callId, granted });
      return Promise.resolve();
    };
    const ctl = new SessionController(base as unknown as BrainBackendClient);
    await ctl.submit('clean this up');
    ctl.resolvePermission('call_9', true);
    expect(resolutions).toEqual([{ callId: 'call_9', granted: true }]);
    expect(JSON.stringify(ctl.getSnapshot().rows)).toContain('Allowed bash');
  });

  test('backends without wire support degrade to local-only UX', async () => {
    const ctl = new SessionController(scriptFake(PERM_SCRIPT));
    await ctl.submit('clean this up');
    expect(() => ctl.resolvePermission('call_9', true)).not.toThrow();
    expect(JSON.stringify(ctl.getSnapshot().rows)).toContain('Allowed bash');
  });

  test('wire rejection never disturbs the local notice', async () => {
    const base = scriptFake(PERM_SCRIPT) as Record<string, unknown>;
    base.resolveToolPermission = () => Promise.reject(new Error('socket gone'));
    const ctl = new SessionController(base as unknown as BrainBackendClient);
    await ctl.submit('clean this up');
    ctl.resolvePermission('call_9', true);
    await new Promise((r) => setTimeout(r, 10)); // flush the rejected promise
    expect(JSON.stringify(ctl.getSnapshot().rows)).toContain('Allowed bash');
  });
});
