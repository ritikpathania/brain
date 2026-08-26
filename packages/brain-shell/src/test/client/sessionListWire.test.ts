import { describe, expect, test, afterAll } from 'bun:test';
import * as net from 'node:net';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { UdsBrainBackendClient } from '../../client/UdsBrainBackendClient.js';

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-session-list-wire-'));
const sockPath = path.join(dir, 't.sock');

const server = net.createServer((socket) => {
  let buffer = '';
  socket.on('data', (data) => {
    buffer += data.toString('utf8');
    let idx: number;
    while ((idx = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, idx);
      buffer = buffer.slice(idx + 1);
      if (!line.trim()) continue;
      const req = JSON.parse(line) as { action?: string };
      const reply = (obj: unknown) => socket.write(JSON.stringify(obj) + '\n');
      if (req.action === 'session/list' || req.action === 'v1/session/list') {
        reply({
          type: 'Response',
          status: 'success',
          body: {
            sessions: [
              {
                sessionId: 's-pinned',
                title: 'Pinned Session',
                messageCount: 3,
                createdAtMs: 1700000000000,
                updatedAtMs: 1700000005000,
                pinned: true,
                archived: false,
              },
              {
                sessionId: 's-archived',
                title: 'Archived Session',
                messageCount: 1,
                createdAtMs: 1700000000000,
                updatedAtMs: 1700000002000,
                pinned: false,
                archived: true,
              },
              {
                sessionId: 's-plain',
                title: 'Plain Session',
                messageCount: 2,
                createdAtMs: 1700000000000,
                updatedAtMs: 1700000001000,
                // missing pinned and archived -> should default to false
              },
              {
                sessionId: 's-malformed',
                title: 'Malformed Session',
                messageCount: 0,
                createdAtMs: 1700000000000,
                updatedAtMs: 1700000000500,
                pinned: 'false', // string truthy value -> strict comparison must yield false
                archived: 0, // non-boolean -> strict comparison must yield false
              },
            ],
            total: 4,
          },
        });
      } else {
        reply({ type: 'Response', status: 'success', body: {} });
      }
    }
  });
});
server.listen(sockPath);
afterAll(() => {
  server.close();
});

describe('session/list wire mapping (Inc 26)', () => {
  test('preserves pinned and archived booleans strictly and defaults missing/malformed to false', async () => {
    const client = new UdsBrainBackendClient(sockPath);
    const summaries = await client.listSessions();
    expect(summaries).toHaveLength(4);

    expect(summaries[0]).toEqual({
      id: 's-pinned',
      title: 'Pinned Session',
      updatedAtMs: 1700000005000,
      pinned: true,
      archived: false,
    });

    expect(summaries[1]).toEqual({
      id: 's-archived',
      title: 'Archived Session',
      updatedAtMs: 1700000002000,
      pinned: false,
      archived: true,
    });

    expect(summaries[2]).toEqual({
      id: 's-plain',
      title: 'Plain Session',
      updatedAtMs: 1700000001000,
      pinned: false,
      archived: false,
    });

    expect(summaries[3]).toEqual({
      id: 's-malformed',
      title: 'Malformed Session',
      updatedAtMs: 1700000000500,
      pinned: false,
      archived: false,
    });
  });
});
