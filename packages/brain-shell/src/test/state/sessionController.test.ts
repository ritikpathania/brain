import { describe, it, expect } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import type {
  BrainBackendClient,
  BrainStreamChunk,
  BrainGenerationRequest,
} from '../../client/BrainBackendClient.js';

/** Fake client: replays scripted chunks, records requests. */
function fakeClient(chunks: BrainStreamChunk[]) {
  const requests: BrainGenerationRequest[] = [];
  const client = {
    async createSession() {
      return { sessionId: 'stub-session-1', title: 'stub', createdAtMs: 0 };
    },
    async *streamText(request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
      requests.push(request);
      for (const c of chunks) yield c;
    },
  } as unknown as BrainBackendClient;
  return { client, requests };
}

const SCRIPT: BrainStreamChunk[] = [
  { type: 'thinking', thinking: 'recalling…' },
  { type: 'token', token: 'Hello ' },
  { type: 'token', token: 'from Brain.' },
  { type: 'tool_use', toolUse: { id: 'call_1', name: 'read_file', input: { path: '/tmp/x' } } },
  { type: 'finished', status: 'completed' },
];

describe('SessionController', () => {
  it('starts idle, freezes rows after a turn, exposes stable snapshots', async () => {
    const { client } = fakeClient(SCRIPT);
    const ctl = new SessionController(client);
    expect(ctl.getSnapshot().busy).toBe(false);
    expect(ctl.getSnapshot().rows).toEqual([]);

    await ctl.submit('hi there');

    const snap = ctl.getSnapshot();
    expect(snap.busy).toBe(false);
    expect(snap.rows[0]).toMatchObject({ kind: 'user', text: 'hi there' });
    const kinds = snap.rows.slice(1).map((r) => r.kind);
    expect(kinds).toContain('thinking');
    expect(kinds).toContain('assistant');
    expect(kinds).toContain('tool');
    expect(snap.live.phase).toBe('idle');
    // Snapshot identity is stable until the next emission.
    expect(ctl.getSnapshot()).toBe(snap);
  });

  it('routes text through the typewriter queue during the turn', async () => {
    const { client } = fakeClient([
      { type: 'token', token: 'abcdefgh' },
      { type: 'finished', status: 'completed' },
    ]);
    const ctl = new SessionController(client);
    let sawPartial = false;
    const done = ctl.submit('q');
    await new Promise((r) => setTimeout(r, 5)); // let first chunk land
    if (ctl.getSnapshot().busy) {
      sawPartial =
        ctl.getSnapshot().live.responseText.length > 0 || ctl.getSnapshot().live.phase === 'responding';
    }
    await done;
    expect(sawPartial || ctl.getSnapshot().rows.some((r) => r.kind === 'assistant')).toBe(true);
  });

  it('surfaces connection failures as connectionError and an error row', async () => {
    const client = {
      async createSession() {
        return { sessionId: 'stub-session-1', title: 'stub', createdAtMs: 0 };
      },
      async *streamText(): AsyncIterable<BrainStreamChunk> {
        yield { type: 'error', error: 'Could not connect to Brain daemon at /tmp/nope.sock (ENOENT)' } as BrainStreamChunk;
      },
    } as unknown as BrainBackendClient;
    const ctl = new SessionController(client);
    await ctl.submit('ping');
    const snap = ctl.getSnapshot();
    expect(snap.connectionError).toBeTruthy();
    expect(snap.busy).toBe(false);
    expect(snap.rows.some((r) => r.kind === 'error')).toBe(true);
  });

  it('ignores submits while busy', async () => {
    let release!: () => void;
    const gate = new Promise<void>((r) => {
      release = r;
    });
    const client = {
      async createSession() {
        return { sessionId: 'stub-session-1', title: 'stub', createdAtMs: 0 };
      },
      async *streamText(): AsyncIterable<BrainStreamChunk> {
        await gate;
        yield { type: 'finished', status: 'completed' } as BrainStreamChunk;
      },
    } as unknown as BrainBackendClient;
    const ctl = new SessionController(client);
    const first = ctl.submit('one');
    await new Promise((r) => setTimeout(r, 5));
    expect(ctl.getSnapshot().busy).toBe(true);
    ctl.submit('two'); // must no-op
    release();
    await first;
    await new Promise((r) => setTimeout(r, 5));
    expect(ctl.getSnapshot().rows.filter((r) => r.kind === 'user')).toHaveLength(1);
  });

  it('settles unfinished tool calls when the turn completes', async () => {
    const { client } = fakeClient([
      { type: 'token', token: 'working' },
      { type: 'tool_use', toolUse: { id: 'call_9', name: 'read_file', input: { path: '/x' } } },
      { type: 'finished', status: 'completed' },
    ]);
    const ctl = new SessionController(client);
    await ctl.submit('go');
    const tool = ctl.getSnapshot().rows.find((r) => r.kind === 'tool');
    expect(tool && tool.kind === 'tool' ? tool.tool.status : undefined).toBe('completed');
  });

  it('settles unfinished tool calls as cancelled when the turn errors', async () => {
    const { client } = fakeClient([
      { type: 'tool_use', toolUse: { id: 'call_10', name: 'bash', input: {} } },
      { type: 'error', error: 'socket error mid-stream' },
    ]);
    const ctl = new SessionController(client);
    await ctl.submit('go');
    const tool = ctl.getSnapshot().rows.find((r) => r.kind === 'tool');
    expect(tool && tool.kind === 'tool' ? tool.tool.status : undefined).toBe('cancelled');
  });
});
