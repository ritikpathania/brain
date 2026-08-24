import { describe, expect, test, afterAll } from 'bun:test';
import * as net from 'node:net';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { UdsBrainBackendClient } from '../../client/UdsBrainBackendClient.js';

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-tool-result-wire-'));
const sockPath = path.join(dir, 't.sock');

// Scripted daemon: v1/session/create replies success; v1/generation/stream
// emits stream_start(0), tool_use(1), tool_permission_requested(2) then parks;
// the next v1/tool/resolve acks and resumes the SAME stream socket with the
// granted or denied continuation.
let streamSocket: net.Socket | null = null;

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
      const emit = (o: Record<string, unknown>) => {
        if (streamSocket) streamSocket.write(JSON.stringify(o) + '\n');
      };
      if (req.action === 'v1/session/create') {
        reply({
          type: 'Response',
          status: 'success',
          body: { session_id: 'stub-tr', title: 't' },
        });
      } else if (req.action === 'v1/generation/stream') {
        streamSocket = socket;
        const sid = 'stub-tr';
        emit({ type: 'stream_start', session_id: sid, sequence: 0 });
        emit({
          type: 'tool_use',
          session_id: sid,
          sequence: 1,
          toolUse: { id: 'call_tr', name: 'bash', input: { command: 'echo hi' } },
        });
        emit({
          type: 'tool_permission_requested',
          session_id: sid,
          sequence: 2,
          call_id: 'call_tr',
          tool_name: 'bash',
          input: { command: 'echo hi' },
          reason: 'gate',
        });
      } else if (req.action === 'v1/tool/resolve') {
        const granted = Boolean(req.payload?.['granted']);
        reply({ type: 'resolved', status: 'ok' });
        const sid = 'stub-tr';
        if (granted) {
          emit({
            type: 'tool_result',
            session_id: sid,
            sequence: 3,
            call_id: 'call_tr',
            tool_name: 'bash',
            output: 'hi\n',
            is_error: false,
            exit_code: 0,
            status: 'in_progress',
          });
          emit({ type: 'token', session_id: sid, sequence: 4, token: 'ok', status: 'in_progress' });
          emit({ type: 'finished', session_id: sid, sequence: 5, status: 'completed' });
        } else {
          emit({
            type: 'tool_denied',
            session_id: sid,
            sequence: 3,
            call_id: 'call_tr',
            tool_name: 'bash',
            status: 'in_progress',
          });
          emit({ type: 'finished', session_id: sid, sequence: 4, status: 'completed' });
        }
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
  chunks: AsyncIterable<{ type: string }>,
): Promise<Array<{ type: string }>> {
  const out: Array<{ type: string }> = [];
  for await (const c of chunks) out.push(c);
  return out;
}

describe('UDS client parses tool_result frames', () => {
  test('granted flow yields a typed tool_result chunk with camelCase fields', async () => {
    const client = new UdsBrainBackendClient(sockPath);
    await client.createSession();
    const chunksPromise = collect(client.streamText({ sessionId: 'stub-tr', messages: [] }));
    await new Promise((r) => setTimeout(r, 150));
    await client.resolveToolPermission('call_tr', true);
    const chunks = await chunksPromise;
    const tr = chunks.find((c) => c.type === 'tool_result') as
      | { callId?: string; output?: string; isError?: boolean; exitCode?: number; sequence?: number }
      | undefined;
    expect(tr).toBeDefined();
    expect(tr!.callId).toBe('call_tr');
    expect(tr!.output).toBe('hi\n');
    expect(tr!.isError).toBe(false);
    expect(tr!.exitCode).toBe(0);
    expect(tr!.sequence).toBe(3);
  });

  test('denied flow completes without a tool_result chunk', async () => {
    const client = new UdsBrainBackendClient(sockPath);
    await client.createSession();
    const chunksPromise = collect(client.streamText({ sessionId: 'stub-tr', messages: [] }));
    await new Promise((r) => setTimeout(r, 150));
    await client.resolveToolPermission('call_tr', false);
    // Iteration completing IS the client's end-of-stream contract: a
    // 'finished' frame terminates the generator WITHOUT being yielded, and
    // 'tool_denied' frames have no chunk mapping (the Inc 4 dialog owns the
    // denial UX). Assert the gate was reached and no result followed.
    const chunks = await chunksPromise;
    expect(chunks.some((c) => c.type === 'permission_request')).toBe(true);
    expect(chunks.some((c) => c.type === 'tool_result')).toBe(false);
  });
});
