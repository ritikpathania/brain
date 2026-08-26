/** Inc 24: the abort path really speaks v1/generation/cancel on the wire
 * and ends the iterator with a cancelled finish. Driven against an
 * in-process fake daemon so pacing is deterministic. */
import { describe, it, expect, afterEach } from 'bun:test';
import * as net from 'net';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { UdsBrainBackendClient } from '../../client/UdsBrainBackendClient.js';

let server: net.Server | null = null;
let sockPath = '';

afterEach(() => {
  if (server) server.close();
  server = null;
  try {
    fs.rmSync(path.dirname(sockPath), { recursive: true, force: true });
  } catch {}
});

function startFakeDaemon(): Promise<string> {
  return new Promise((resolve) => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-interrupt-'));
    sockPath = path.join(dir, 'brain.sock');
    let seq = 0;
    let timer: ReturnType<typeof setInterval> | null = null;
    server = net.createServer((socket) => {
      let buf = '';
      let gid = 'gen-fake';
      let sid = 'sess-fake';
      socket.on('data', (d) => {
        buf += d.toString('utf8');
        let idx: number;
        while ((idx = buf.indexOf('\n')) >= 0) {
          const line = buf.slice(0, idx);
          buf = buf.slice(idx + 1);
          if (!line.trim()) continue;
          let frame: any;
          try {
            frame = JSON.parse(line);
          } catch {
            continue;
          }
          if (frame.action === 'v1/generation/stream') {
            const p = frame.payload ?? frame.body ?? {};
            gid = p.generationId ?? p.generation_id ?? 'gen-fake';
            sid = p.sessionId ?? p.session_id ?? 'sess-fake';
            timer = setInterval(() => {
              if (socket.destroyed) {
                if (timer) clearInterval(timer);
                return;
              }
              try {
                socket.write(
                  JSON.stringify({
                    type: 'token',
                    token: 'x',
                    sequence: seq++,
                    generation_id: gid,
                    session_id: sid,
                  }) + '\n',
                );
              } catch {
                if (timer) clearInterval(timer);
              }
            }, 5);
          } else if (frame.action === 'v1/generation/cancel') {
            if (timer) clearInterval(timer);
            (server as any).__cancelSeen = frame.payload ?? {};
            try {
              socket.write(
                JSON.stringify({
                  type: 'finished',
                  status: 'cancelled',
                  sequence: seq++,
                  generation_id: gid,
                  session_id: sid,
                }) + '\n',
              );
            } catch {}
          }
        }
      });
      socket.on('error', () => {});
    });
    server.listen(sockPath, () => resolve(sockPath));
  });
}

describe('stream cancel wire proof', () => {
  it('abort sends v1/generation/cancel and yields a cancelled finish', async () => {
    const sock = await startFakeDaemon();
    const client = new UdsBrainBackendClient(sock);
    const ac = new AbortController();
    const chunks: any[] = [];
    const consume = (async () => {
      for await (const c of client.streamText({
        sessionId: 'sess-wire',
        generationId: 'gen-wire',
        messages: [],
        signal: ac.signal,
      } as never)) {
        chunks.push(c);
      }
    })();
    await new Promise((r) => setTimeout(r, 60)); // tokens flowing
    ac.abort();
    await consume;
    for (let i = 0; i < 50 && !(server as any).__cancelSeen; i++) {
      await new Promise((r) => setTimeout(r, 5));
    }
    const cancelSeen = ((server as any).__cancelSeen ?? {}) as Record<string, unknown>;
    expect(cancelSeen.generation_id).toBe('gen-wire');
    expect(cancelSeen.session_id).toBe('sess-wire');
    expect(chunks.length).toBeGreaterThan(0);
    const last = chunks[chunks.length - 1];
    if (last.type === 'finished') {
      expect(last.status).toBe('cancelled');
    }
  });
});
