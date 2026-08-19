/**
 * Live UDS Brain Backend Client (Phase 5.6 / 5.7)
 *
 * Implements BrainBackendClient over a local Unix Domain Socket (UDS).
 * Adheres strictly to the deterministic disconnect / zero-reconnect invariant.
 */

import * as net from 'net';
import * as readline from 'readline';
import type {
  BrainBackendClient,
  BrainGenerationRequest,
  BrainStreamChunk,
} from './BrainBackendClient.js';

export class UdsBrainBackendClient implements BrainBackendClient {
  constructor(private socketPath: string = process.env.BRAIN_SOCKET_PATH || '/tmp/brain.sock') {}

  async *streamText(request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
    if (request.signal?.aborted) {
      return;
    }

    const requestId = `req_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    let socket: net.Socket | null = null;
    let rl: readline.Interface | null = null;
    let isStreamDone = false;

    // Create a queue for incoming stream chunks
    const chunkQueue: BrainStreamChunk[] = [];
    let resolveNextChunk: (() => void) | null = null;

    const pushChunk = (chunk: BrainStreamChunk) => {
      chunkQueue.push(chunk);
      if (resolveNextChunk) {
        const resolve = resolveNextChunk;
        resolveNextChunk = null;
        resolve();
      }
    };

    try {
      socket = await new Promise<net.Socket>((resolve, reject) => {
        const s = net.createConnection(this.socketPath);
        
        const onError = (err: Error) => {
          s.removeListener('connect', onConnect);
          reject(err);
        };
        const onConnect = () => {
          s.removeListener('error', onError);
          resolve(s);
        };

        s.once('error', onError);
        s.once('connect', onConnect);
      });
    } catch (err: any) {
      yield {
        type: 'error',
        error: `Could not connect to Brain daemon at ${this.socketPath} (${err.code || err.message})`,
      };
      return;
    }

    // Always attach persistent error handler on connected socket
    socket.on('error', (err: any) => {
      if (!isStreamDone && !request.signal?.aborted) {
        pushChunk({
          type: 'error',
          error: `Brain daemon socket error: ${err.message || 'connection failed'}`,
        });
      }
      isStreamDone = true;
    });

    socket.on('close', () => {
      if (!isStreamDone && !request.signal?.aborted) {
        // Socket severed mid-stream: deterministic error, NO reconnect
        pushChunk({
          type: 'error',
          error: 'Brain daemon socket disconnected mid-stream',
        });
      }
      isStreamDone = true;
    });

    // Bind abort listener to cancel and destroy socket
    const abortHandler = () => {
      if (socket && !socket.destroyed) {
        socket.write(
          JSON.stringify({
            id: requestId,
            action: 'v1/generation/cancel',
          }) + '\n',
          () => {}
        );
        socket.destroy();
      }
      isStreamDone = true;
      pushChunk({ type: 'finished' });
    };

    if (request.signal) {
      request.signal.addEventListener('abort', abortHandler, { once: true });
    }

    // Set up line reader
    rl = readline.createInterface({
      input: socket,
      crlfDelay: Infinity,
    });

    rl.on('line', (line) => {
      if (!line || line.trim() === '') return;

      try {
        const parsed = JSON.parse(line);

        if (parsed.type === 'token' && typeof parsed.token === 'string') {
          pushChunk({
            type: 'token',
            token: parsed.token,
            metadata: parsed.metadata,
          });
        } else if (parsed.type === 'thinking' && typeof parsed.thinking === 'string') {
          pushChunk({
            type: 'thinking',
            thinking: parsed.thinking,
            signature: parsed.signature,
            metadata: parsed.metadata,
          });
        } else if (parsed.type === 'redacted_thinking' && typeof parsed.redactedData === 'string') {
          pushChunk({
            type: 'redacted_thinking',
            redactedData: parsed.redactedData,
            metadata: parsed.metadata,
          });
        } else if (parsed.type === 'tool_use' && parsed.toolUse) {
          pushChunk({
            type: 'tool_use',
            toolUse: parsed.toolUse,
            metadata: parsed.metadata,
          });
        } else if (parsed.type === 'error') {
          pushChunk({
            type: 'error',
            error: parsed.error || 'Brain daemon error',
          });
          isStreamDone = true;
        } else if (parsed.type === 'finished') {
          isStreamDone = true;
          pushChunk({ type: 'finished' });
        }
      } catch (err: any) {
        pushChunk({
          type: 'error',
          error: `Malformed frame from Brain daemon: ${err.message}`,
        });
        isStreamDone = true;
      }
    });

    // Send the query request frame
    try {
      const payload = {
        id: requestId,
        action: 'v1/generation/stream',
        payload: {
          sessionId: request.sessionId,
          messages: request.messages,
          systemPrompt: request.systemPrompt,
          tools: request.tools,
          thinkingConfig: request.thinkingConfig,
          model: request.model,
        },
      };
      socket.write(JSON.stringify(payload) + '\n', () => {});
    } catch (err: any) {
      yield {
        type: 'error',
        error: `Failed to write request to Brain daemon: ${err.message}`,
      };
      if (socket && !socket.destroyed) socket.destroy();
      return;
    }

    // Yield chunks as they arrive
    try {
      while (!isStreamDone || chunkQueue.length > 0) {
        if (chunkQueue.length === 0) {
          await new Promise<void>((resolve) => {
            resolveNextChunk = resolve;
          });
        }

        while (chunkQueue.length > 0) {
          const chunk = chunkQueue.shift()!;
          if (chunk.type === 'finished') {
            return;
          }
          yield chunk;
          if (chunk.type === 'error') {
            return;
          }
        }
      }
    } finally {
      if (request.signal) {
        request.signal.removeEventListener('abort', abortHandler);
      }
      if (rl) {
        rl.close();
      }
      if (socket && !socket.destroyed) {
        socket.destroy();
      }
    }
  }
}
