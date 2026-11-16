import { createConnection, createServer, type Socket } from 'node:net';

export type WireObservation = Readonly<{
  atMs: number;
  direction: 'request' | 'response' | 'marker';
  action?: string;
  type?: string;
  status?: string;
  sessionId?: string;
  generationId?: string;
  sequence?: number;
}>;

/** Test-only transparent NDJSON relay; an explicit gate models delayed stream delivery. */
export class GenerationTrace {
  public readonly observations: WireObservation[] = [];
  private readonly sockets = new Set<Socket>();
  private readonly server = createServer(socket => this.accept(socket));
  private readonly started = performance.now();
  private gateNext = false;
  private releaseHeld: (() => void) | undefined;

  private constructor(public readonly socketPath: string, private readonly daemonSocket: string) {}

  public static async start(socketPath: string, daemonSocket: string): Promise<GenerationTrace> {
    const trace = new GenerationTrace(socketPath, daemonSocket);
    await new Promise<void>((resolve, reject) => {
      trace.server.once('error', reject);
      trace.server.listen(socketPath, resolve);
    });
    return trace;
  }

  public holdNextGeneration(): void { this.gateNext = true; }
  public hasHeldGeneration(): boolean { return this.releaseHeld !== undefined; }
  public releaseGeneration(): void { this.releaseHeld?.(); this.releaseHeld = undefined; }
  public mark(type: string): void { this.record({ direction: 'marker', type }); }

  public async close(): Promise<void> {
    for (const socket of this.sockets) socket.destroy();
    await new Promise<void>(resolve => this.server.close(() => resolve()));
  }

  private record(event: Omit<WireObservation, 'atMs'>): void {
    this.observations.push(Object.freeze({ atMs: performance.now() - this.started, ...event }));
  }

  private accept(client: Socket): void {
    const backend = createConnection(this.daemonSocket);
    this.sockets.add(client);
    this.sockets.add(backend);
    for (const socket of [client, backend]) {
      socket.on('error', () => { client.destroy(); backend.destroy(); });
      socket.on('close', () => { this.sockets.delete(socket); });
    }
    client.on('end', () => backend.end());
    backend.on('end', () => client.end());
    let action: string | undefined;
    let requests = '';
    let responses = '';
    client.on('data', chunk => {
      requests += chunk.toString();
      let newline: number;
      while ((newline = requests.indexOf('\n')) >= 0) {
        const line = requests.slice(0, newline);
        requests = requests.slice(newline + 1);
        const frame = JSON.parse(line);
        action = frame.action;
        const payload = frame.payload ?? {};
        this.record({ direction: 'request', action, sessionId: payload.session_id ?? payload.sessionId, generationId: payload.generation_id ?? payload.generationId });
        const forward = () => { this.mark(`forward:${action}`); backend.write(`${line}\n`); };
        if (action === 'v1/generation/stream' && this.gateNext) {
          this.gateNext = false;
          this.releaseHeld = forward;
        } else forward();
      }
    });
    backend.on('data', chunk => {
      responses += chunk.toString();
      let newline: number;
      while ((newline = responses.indexOf('\n')) >= 0) {
        const line = responses.slice(0, newline);
        responses = responses.slice(newline + 1);
        const frame = JSON.parse(line);
        this.record({ direction: 'response', action, type: frame.type, status: frame.status, sessionId: frame.session_id, generationId: frame.generation_id, sequence: frame.sequence });
        client.write(`${line}\n`);
      }
    });
  }
}
