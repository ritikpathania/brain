import { describe, expect, test, afterAll } from 'bun:test';
import * as net from 'node:net';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { UdsBrainBackendClient } from '../../client/UdsBrainBackendClient.js';

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-stream-resilience-'));
const sockPath = path.join(dir, 't.sock');

// Scripted daemon for Inc 14 boundary-resilience tests: v1/session/create
// replies success; v1/generation/stream emits stream_start(0), then a RAW
// NON-JSON LINE, then token(1)/finished(2). One corrupt frame must not kill
// the turn — the client warns and keeps reading.
const server = net.createServer((socket) => {
  let buffer = '';
  socket.on('data', (data) => {
    buffer += data.toString('utf8');
    let idx: number;
    while ((idx = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, idx);
      buffer = buffer.slice(idx + 1);
      if (!line.trim()) continue;
      const req = JSON.parse(line) as {
        action?: string;
        payload?: Record<string, unknown>;
      };
      const reply = (obj: unknown) => socket.write(JSON.stringify(obj) + '\n');
      const emit = (o: Record<string, unknown>) => socket.write(JSON.stringify(o) + '\n');
      if (req.action === 'v1/session/create') {
        reply({
          type: 'Response',
          status: 'success',
          body: { session_id: 'stub-sr', title: 't' },
        });
      } else if (req.action === 'v1/generation/stream') {
        emit({ type: 'stream_start', session_id: 'stub-sr', sequence: 0 });
        socket.write('<<<definitely not json>>>\n');
        emit({ type: 'token', session_id: 'stub-sr', sequence: 1, token: 'survivor', status: 'in_progress' });
        emit({ type: 'finished', session_id: 'stub-sr', sequence: 2, status: 'completed' });
      }
    }
  });
});
server.listen(sockPath);

afterAll(() => {
  server.close();
  fs.rmSync(dir, { recursive: true, force: true });
});

async function collect(
  chunks: AsyncIterable<{ type: string; token?: string }>,
): Promise<Array<{ type: string; token?: string }>> {
  const out: Array<{ type: string; token?: string }> = [];
  for await (const c of chunks) out.push(c);
  return out;
}

describe('UDS client survives malformed frames (Inc 14)', () => {
  test('a garbage line warns and is skipped instead of killing the stream', async () => {
    const warnings: string[] = [];
    const origWarn = console.warn;
    console.warn = (...args: unknown[]) => {
      warnings.push(args.map(String).join(' '));
    };
    let chunks: Array<{ type: string; token?: string }>;
    try {
      const client = new UdsBrainBackendClient(sockPath);
      await client.createSession();
      chunks = await collect(client.streamText({ sessionId: 'stub-sr', messages: [] }));
    } finally {
      console.warn = origWarn;
    }
    // The good frames on both sides of the corrupt line still arrive…
    expect(chunks.some((c) => c.type === 'token' && c.token === 'survivor')).toBe(true);
    // …no fatal error chunk was synthesized…
    expect(chunks.some((c) => c.type === 'error')).toBe(false);
    // …and the skip left a trace on stderr rather than vanishing.
    expect(warnings.some((w) => /malformed/i.test(w))).toBe(true);
  });
});
