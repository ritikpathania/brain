import { describe, expect, test, afterAll } from 'bun:test';
import * as net from 'node:net';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { UdsBrainBackendClient } from '../../client/UdsBrainBackendClient.js';

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-shell-exec-wire-'));
const sockPath = path.join(dir, 't.sock');

// Scripted daemon: echoes the exec payload back shaped like the daemon's
// success body; supports slow replies for abort testing.
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
      if (req.action !== 'v1/shell/exec') return;
      const cmd = String(req.payload?.['command'] ?? '');
      if (cmd === 'slow') {
        setTimeout(() => {
          reply({
            version: '1.0',
            type: 'Response',
            id: 'x',
            status: 'success',
            body: {
              call_id: 'shell-slow',
              name: 'bash',
              input: { command: 'slow' },
              outcome: 'executed',
              output: 'late',
              is_error: false,
              exit_code: 0,
              duration_ms: 5,
            },
          });
        }, 1500);
        return;
      }
      if (cmd === 'boom') {
        reply({ version: '1.0', type: 'Error', id: 'x', status: 'error', body: 'shell exec failed: nope' });
        return;
      }
      reply({
        version: '1.0',
        type: 'Response',
        id: 'x',
        status: 'success',
        body: {
          call_id: 'shell-abc',
          name: 'bash',
          input: { command: cmd },
          outcome: 'executed',
          output: `${cmd}\n`,
          is_error: false,
          exit_code: 0,
          duration_ms: 12,
        },
      });
    }
  });
});
server.listen(sockPath);

afterAll(() => {
  server.close();
  fs.rmSync(dir, { recursive: true, force: true });
});

describe('UDS client execShell', () => {
  test('maps snake_case body to camelCase ShellExecResult', async () => {
    const client = new UdsBrainBackendClient(sockPath);
    const res = await client.execShell('sess-1', 'echo hi');
    expect(res.callId).toBe('shell-abc');
    expect(res.name).toBe('bash');
    expect(res.input).toEqual({ command: 'echo hi' });
    expect(res.outcome).toBe('executed');
    expect(res.output).toBe('echo hi\n');
    expect(res.isError).toBe(false);
    expect(res.exitCode).toBe(0);
    expect(res.durationMs).toBe(12);
  });

  test('error-status responses reject with the daemon message', async () => {
    const client = new UdsBrainBackendClient(sockPath);
    await expect(client.execShell('sess-1', 'boom')).rejects.toThrow('shell exec failed');
  });

  test('aborting the signal tears down the wait deterministically', async () => {
    const client = new UdsBrainBackendClient(sockPath);
    const ac = new AbortController();
    const pending = client.execShell('sess-1', 'slow', ac.signal);
    setTimeout(() => ac.abort(), 100);
    await expect(pending).rejects.toThrow(/abort/i);
  });
});
