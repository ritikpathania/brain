import { describe, expect, test, afterAll } from 'bun:test';
import * as net from 'node:net';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { MockBrainBackendClient } from '../../client/BrainBackendClient.js';
import { UdsBrainBackendClient as LiveUdsClient } from '../../client/UdsBrainBackendClient.js';

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-resolve-wire-'));
const sockPath = path.join(dir, 't.sock');

type Frame = { action?: string; payload?: Record<string, unknown> };
const received: Frame[] = [];
const server = net.createServer((socket) => {
  let buffer = '';
  socket.on('data', (data) => {
    buffer += data.toString('utf8');
    let idx: number;
    while ((idx = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, idx);
      buffer = buffer.slice(idx + 1);
      if (!line.trim()) continue;
      const frame = JSON.parse(line) as Frame;
      received.push(frame);
      const unknownCall =
        frame.action === 'v1/tool/resolve' && frame.payload?.['call_id'] === 'bogus';
      socket.write(
        JSON.stringify(
          unknownCall
            ? { type: 'Error', status: 'error', body: "Unknown or already-resolved tool call 'bogus'" }
            : { type: 'resolved', status: 'ok' },
        ) + '\n',
      );
    }
  });
});
server.listen(sockPath);

afterAll(() => {
  server.close();
  fs.rmSync(dir, { recursive: true, force: true });
});

describe('wire resolution of pending permissions', () => {
  test('mock client records resolutions for assertions', async () => {
    const mock = new MockBrainBackendClient(['ok']);
    await mock.resolveToolPermission!('call_9', true);
    expect(mock.permissionResolutions).toEqual([{ callId: 'call_9', granted: true }]);
  });

  test('live client sends v1/tool/resolve with snake_case payload over UDS', async () => {
    const client = new LiveUdsClient(sockPath);
    await client.resolveToolPermission('call_9', true);
    const frame = received.find((f) => f.action === 'v1/tool/resolve');
    expect(frame).toBeDefined();
    expect(frame!.payload).toMatchObject({ call_id: 'call_9', granted: true });
  });

  test('live client surfaces unknown-call errors as rejections', async () => {
    const client = new LiveUdsClient(sockPath);
    expect(client.resolveToolPermission('bogus', false)).rejects.toThrow(/Unknown or already-resolved/);
    await new Promise((r) => setTimeout(r, 50));
  });
});
