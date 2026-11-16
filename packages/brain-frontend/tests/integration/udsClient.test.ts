import { afterEach, describe, expect, test } from 'bun:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { createServer, type Server } from 'node:net';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

import {
  UdsClient,
  UdsProtocolError,
  UdsResponseError,
} from '../../src/brain/transport/udsClient.js';

const cleanup: Array<() => void> = [];

afterEach(() => {
  for (const dispose of cleanup.splice(0)) dispose();
});

async function serve(
  reply: (request: Record<string, unknown>) => Record<string, unknown>,
): Promise<string> {
  const directory = mkdtempSync(join(tmpdir(), 'brain-frontend-uds-'));
  const socketPath = join(directory, 'brain.sock');
  const server = createServer(socket => {
    socket.once('data', chunk => {
      const request = JSON.parse(chunk.toString('utf8').trim()) as Record<string, unknown>;
      socket.end(`${JSON.stringify(reply(request))}\n`);
    });
  });
  await new Promise<void>((resolve, reject) => {
    server.once('error', reject);
    server.listen(socketPath, () => {
      server.off('error', reject);
      resolve();
    });
  });
  cleanup.push(() => {
    server.close();
    rmSync(directory, { recursive: true, force: true });
  });
  return socketPath;
}

describe('UDS transport', () => {
  test('accepts only a response correlated to the sent request ID', async () => {
    const socketPath = await serve(request => {
      expect(request.version).toBe('1.0');
      expect(request.type).toBe('Request');
      expect(typeof request.id).toBe('number');
      return {
        version: '1.0',
        type: 'Response',
        id: request.id,
        status: 'success',
        body: JSON.stringify({ ok: true }),
      };
    });

    await expect(new UdsClient({ socketPath }).request('v1/session/list', {}))
      .resolves.toEqual({ ok: true });
  });

  test('rejects a mismatched UDS response ID instead of accepting a foreign response', async () => {
    const socketPath = await serve(() => ({
      version: '1.0',
      type: 'Response',
      id: 'foreign-request',
      status: 'success',
      body: '{}',
    }));

    await expect(new UdsClient({ socketPath }).request('v1/session/list', {}))
      .rejects.toMatchObject({ code: 'request_correlation' } satisfies Partial<UdsProtocolError>);
  });

  test('preserves daemon error codes when a versioned error body is text rather than JSON', async () => {
    const socketPath = await serve(request => ({
      version: '1.0',
      type: 'Error',
      id: request.id,
      status: 'error',
      code: 'session_not_found',
      body: 'Session not found',
    }));

    await expect(new UdsClient({ socketPath }).request('v1/session/load', { session_id: '01ARZ3NDEKTSV4RRFFQ69G5FAV' }))
      .rejects.toMatchObject({ code: 'session_not_found' } satisfies Partial<UdsResponseError>);
  });
});
