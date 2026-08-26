import { describe, expect, test, afterAll } from 'bun:test';
import * as net from 'node:net';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { UdsBrainBackendClient } from '../../client/UdsBrainBackendClient.js';

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-memory-wire-'));
const sockPath = path.join(dir, 't.sock');

// Scripted daemon: memory/search replies with one memory whose DTO carries
// relations — proving the client mapping preserves them.
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
      if (req.action === 'memory/search') {
        reply({
          type: 'Response',
          status: 'success',
          body: {
            memories: [
              {
                node_id: 'n1',
                label: 'Alpha Cortex Node',
                excerpt: 'Cortex excerpt body',
                channel: 'knowledge_graph',
                score: 97,
                timestamp: 1756160000000,
                scope: 'workspace',
                relations: [
                  { target_id: 'b1', relation: 'supports', target_label: 'Beta Concept' },
                ],
              },
            ],
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

describe('searchMemory wire mapping (Inc 21)', () => {
  test('preserves relations from the daemon DTO', async () => {
    const client = new UdsBrainBackendClient(sockPath);
    const res = await client.searchMemory({ query: 'cortex', limit: 10 });
    expect(res.memories).toHaveLength(1);
    expect(res.memories[0]!.label).toBe('Alpha Cortex Node');
    expect(res.memories[0]!.score).toBe(97);
    expect(res.memories[0]!.relations).toEqual([
      { target_id: 'b1', relation: 'supports', target_label: 'Beta Concept' },
    ]);
  });
});
