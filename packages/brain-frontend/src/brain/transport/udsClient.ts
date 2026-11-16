import { createConnection, type Socket } from 'node:net';
import { randomUUID } from 'node:crypto';
import { createInterface } from 'node:readline';

import { isRecord, type JsonRecord } from '../protocol/json.js';

export type UdsClientOptions = Readonly<{
  socketPath: string;
  connectTimeoutMs?: number;
}>;

export class UdsProtocolError extends Error {
  public constructor(
    public readonly code: 'connection_unavailable' | 'request_correlation' | 'malformed_response' | 'daemon_error',
    message: string,
  ) {
    super(`${code}: ${message}`);
    this.name = 'UdsProtocolError';
  }
}

export class UdsResponseError extends Error {
  public constructor(
    public readonly code: string,
    message: string,
  ) {
    super(`${code}: ${message}`);
    this.name = 'UdsResponseError';
  }
}

/**
 * Narrow, presentation-agnostic client for the daemon's versioned request/
 * response UDS protocol. Each request uses a fresh socket, which also makes
 * single-flight actions with legacy direct responses safe to correlate.
 */
export class UdsClient {
  private readonly connectTimeoutMs: number;
  private nextRequestId = 1;

  public constructor(private readonly options: UdsClientOptions) {
    if (options.socketPath.length === 0) {
      throw new UdsProtocolError('connection_unavailable', 'a UDS socket path is required');
    }
    this.connectTimeoutMs = options.connectTimeoutMs ?? 5_000;
  }

  public async request(action: string, body: unknown): Promise<unknown> {
    const requestId = this.nextRequestId;
    this.nextRequestId += 1;
    const response = await this.exchange({
      version: '1.0',
      type: 'Request',
      id: requestId,
      action,
      body: JSON.stringify(body),
    });
    return decodeCorrelatedResponse(response, requestId);
  }

  /**
   * Sends an action whose established daemon contract returns a direct,
   * uncorrelated acknowledgement. The request is isolated on its own socket;
   * callers must not use this for multiplexed actions.
   */
  public async requestSingleFlight(action: string, body: unknown): Promise<unknown> {
    const response = await this.exchange({
      id: randomUUID(),
      action,
      payload: body,
    });
    return decodeDaemonResponse(response);
  }

  /**
   * Opens the daemon's streaming-action frame form. Transport preserves frame
   * order; semantic sequence and correlation validation belongs to the
   * generation guard.
   */
  public async *stream(action: string, payload: unknown): AsyncGenerator<unknown, void, void> {
    const socket = await this.connect();
    const reader = createInterface({ input: socket, crlfDelay: Infinity });
    socket.write(`${JSON.stringify({ id: randomUUID(), action, payload })}\n`);

    try {
      for await (const line of reader) {
        if (line.trim().length === 0) continue;
        yield parseFrame(line);
      }
    } catch (error) {
      if (error instanceof UdsProtocolError || error instanceof UdsResponseError) throw error;
      throw new UdsProtocolError('connection_unavailable', `stream read failed: ${messageOf(error)}`);
    } finally {
      reader.close();
      socket.destroy();
    }
  }

  private async exchange(frame: unknown): Promise<JsonRecord> {
    const socket = await this.connect();
    const reader = createInterface({ input: socket, crlfDelay: Infinity });
    socket.write(`${JSON.stringify(frame)}\n`);

    try {
      for await (const line of reader) {
        if (line.trim().length === 0) continue;
        return parseFrame(line);
      }
      throw new UdsProtocolError('connection_unavailable', 'daemon closed the socket before a response');
    } catch (error) {
      if (error instanceof UdsProtocolError || error instanceof UdsResponseError) throw error;
      throw new UdsProtocolError('connection_unavailable', `request failed: ${messageOf(error)}`);
    } finally {
      reader.close();
      socket.destroy();
    }
  }

  private async connect(): Promise<Socket> {
    return new Promise<Socket>((resolve, reject) => {
      const socket = createConnection(this.options.socketPath);
      const timeout = setTimeout(() => {
        socket.destroy();
        reject(new UdsProtocolError('connection_unavailable', `timed out connecting to ${this.options.socketPath}`));
      }, this.connectTimeoutMs);
      socket.once('connect', () => {
        clearTimeout(timeout);
        resolve(socket);
      });
      socket.once('error', error => {
        clearTimeout(timeout);
        reject(new UdsProtocolError('connection_unavailable', `could not connect to ${this.options.socketPath}: ${error.message}`));
      });
    });
  }
}

function parseFrame(line: string): JsonRecord {
  try {
    const value: unknown = JSON.parse(line);
    if (!isRecord(value)) {
      throw new UdsProtocolError('malformed_response', 'daemon frame must be a JSON object');
    }
    return value;
  } catch (error) {
    if (error instanceof UdsProtocolError) throw error;
    throw new UdsProtocolError('malformed_response', `daemon sent invalid JSON: ${messageOf(error)}`);
  }
}

function decodeCorrelatedResponse(frame: JsonRecord, requestId: number): unknown {
  if (frame.id !== requestId) {
    throw new UdsProtocolError('request_correlation', 'daemon response does not match the request ID');
  }
  return decodeDaemonResponse(frame);
}

function decodeDaemonResponse(frame: JsonRecord): unknown {
  if (frame.type === 'Error' || frame.status === 'error') {
    const body = decodeErrorBody(frame.body);
    const code = typeof frame.code === 'string'
      ? frame.code
      : typeof body?.code === 'string'
        ? body.code
        : 'daemon_error';
    const message = typeof frame.message === 'string'
      ? frame.message
      : typeof body?.message === 'string'
        ? body.message
        : typeof frame.body === 'string'
          ? frame.body
          : 'daemon returned an error without a message';
    throw new UdsResponseError(code, message);
  }
  if (frame.body === undefined) return frame;
  if (typeof frame.body === 'string') {
    try {
      return JSON.parse(frame.body) as unknown;
    } catch (error) {
      throw new UdsProtocolError('malformed_response', `response body is not JSON: ${messageOf(error)}`);
    }
  }
  return frame.body;
}

function decodeErrorBody(body: unknown): JsonRecord | undefined {
  if (isRecord(body)) return body;
  if (typeof body !== 'string') return undefined;
  try {
    const parsed: unknown = JSON.parse(body);
    return isRecord(parsed) ? parsed : undefined;
  } catch {
    return undefined;
  }
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
