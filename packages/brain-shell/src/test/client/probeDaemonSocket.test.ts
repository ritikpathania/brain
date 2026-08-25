import { describe, expect, test, afterAll } from 'bun:test';
import * as net from 'node:net';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { probeDaemonSocket } from '../../client/probeDaemonSocket.js';

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-probe-'));
const sockPath = path.join(dir, 'live.sock');

// A real listener proves the happy path; a missing path proves the
// failure path. No protocol bytes involved — transport liveness only.
const server = net.createServer(() => {});
server.listen(sockPath);

afterAll(() => {
  server.close();
  fs.rmSync(dir, { recursive: true, force: true });
});

describe('probeDaemonSocket', () => {
  test('resolves true against a live listener', async () => {
    expect(await probeDaemonSocket(sockPath, 1000)).toBe(true);
  });

  test('resolves false (never rejects) against a dead path', async () => {
    expect(await probeDaemonSocket(path.join(dir, 'absent.sock'), 1000)).toBe(false);
  });
});
