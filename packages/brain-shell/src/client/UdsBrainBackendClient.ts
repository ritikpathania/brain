/**
 * Live UDS Brain Backend Client (Phase 5.6 / 5.7)
 *
 * Implements BrainBackendClient over a local Unix Domain Socket (UDS).
 * Adheres strictly to the deterministic disconnect / zero-reconnect invariant.
 */

import * as net from 'net';
import * as readline from 'readline';
import * as fs from 'fs';
import type {
  BrainBackendClient,
  BrainBackend,
  StartSessionInput,
  StartSessionOutput,
  AppendTurnInput,
  AppendTurnOutput,
  CompleteTurnInput,
  CompleteTurnOutput,
  MemorySearchInput,
  MemorySearchOutput,
  MemoryStoreInput,
  MemoryStoreOutput,
  BrainGenerationRequest,
  BrainStreamChunk,
  BrainSession,
  BrainSessionSummary,
  CreateSessionRequest,
  CreateSessionResponse,
  ForkSessionRequest,
  ForkSessionResponse,
  ContextRetrieveRequest,
  ContextRetrieveResponse,
  AuthoritativeSessionSummary,
  AuthoritativeSessionDetail,
  ConsolidationResult,
  BrainModelDescriptor,
  ShellExecResult,
} from './BrainBackendClient.js';

export class UdsBrainBackendClient implements BrainBackendClient {
  constructor(private socketPath: string = process.env.BRAIN_SOCKET_PATH || '/tmp/brain.sock') {}

  async *streamText(request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
    if (request.signal?.aborted) {
      return;
    }

    const generationId = request.generationId || (typeof crypto !== 'undefined' && crypto.randomUUID ? crypto.randomUUID() : `gen_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`);
    const requestId = `req_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    let socket: net.Socket | null = null;
    let rl: readline.Interface | null = null;
    let isStreamDone = false;
    let expectedSequence = 0;

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
        generationId,
        sessionId: request.sessionId,
        status: 'failed',
        error: `Could not connect to Brain daemon at ${this.socketPath} (${err.code || err.message})`,
      };
      return;
    }

    // Always attach persistent error handler on connected socket
    socket.on('error', (err: any) => {
      if (!isStreamDone && !request.signal?.aborted) {
        pushChunk({
          type: 'error',
          generationId,
          sessionId: request.sessionId,
          status: 'failed',
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
          generationId,
          sessionId: request.sessionId,
          status: 'failed',
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
            payload: {
              generation_id: generationId,
              session_id: request.sessionId,
            },
          }) + '\n',
          () => {
            if (socket && !socket.destroyed) {
              socket.destroy();
            }
          }
        );
      }
      isStreamDone = true;
      pushChunk({
        type: 'finished',
        generationId,
        sessionId: request.sessionId,
        status: 'cancelled',
      });
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
        const raw = (parsed.event === 'stream_chunk' && parsed.chunk) ? parsed.chunk : parsed;

        // Invariant 9: Session Guard
        const frameSessionId = raw.session_id || raw.sessionId;
        if (request.sessionId && frameSessionId && frameSessionId !== request.sessionId) {
          // Stale frame from another session, ignore
          return;
        }

        // Invariant 2: Strict Sequence Validation & Gap Detection
        if (typeof raw.sequence === 'number') {
          if (raw.sequence === expectedSequence) {
            expectedSequence++;
          } else if (raw.sequence < expectedSequence) {
            // Duplicate/stale frame -> ignore
            return;
          } else {
            // Gap detected (raw.sequence > expectedSequence) -> protocol violation
            pushChunk({
              type: 'error',
              generationId,
              sessionId: request.sessionId,
              sequence: raw.sequence,
              status: 'failed',
              error: `Protocol violation: expected sequence ${expectedSequence}, received ${raw.sequence} (gap detected)`,
            });
            isStreamDone = true;
            return;
          }
        }

        const chunkGenId = raw.generation_id || raw.generationId || generationId;
        const chunkSessionId = frameSessionId || request.sessionId;
        const chunkStatus = raw.status || (raw.type === 'finished' ? 'completed' : 'in_progress');

        if (raw.type === 'token' && typeof raw.token === 'string') {
          pushChunk({
            type: 'token',
            token: raw.token,
            generationId: chunkGenId,
            sessionId: chunkSessionId,
            sequence: raw.sequence,
            status: chunkStatus,
            metadata: raw.metadata,
          });
        } else if (raw.type === 'thinking' && typeof raw.thinking === 'string') {
          pushChunk({
            type: 'thinking',
            thinking: raw.thinking,
            signature: raw.signature,
            generationId: chunkGenId,
            sessionId: chunkSessionId,
            sequence: raw.sequence,
            status: chunkStatus,
            metadata: raw.metadata,
          });
        } else if (raw.type === 'thinking_delta' && typeof raw.thinking === 'string') {
          pushChunk({
            type: 'thinking',
            thinking: raw.thinking,
            signature: raw.signature,
            generationId: chunkGenId,
            sessionId: chunkSessionId,
            sequence: raw.sequence,
            status: chunkStatus,
            metadata: raw.metadata,
          });
        } else if (raw.type === 'redacted_thinking' && typeof raw.redactedData === 'string') {
          pushChunk({
            type: 'redacted_thinking',
            redactedData: raw.redactedData,
            generationId: chunkGenId,
            sessionId: chunkSessionId,
            sequence: raw.sequence,
            status: chunkStatus,
            metadata: raw.metadata,
          });
        } else if (raw.type === 'tool_use' && raw.toolUse) {
          pushChunk({
            type: 'tool_use',
            toolUse: raw.toolUse,
            generationId: chunkGenId,
            sessionId: chunkSessionId,
            sequence: raw.sequence,
            status: chunkStatus,
            metadata: raw.metadata,
          });
        } else if (raw.type === 'tool_permission_requested') {
          // Tolerant reception: the daemon does not emit these yet; when it
          // does, the shell surfaces an approval dialog without protocol change.
          pushChunk({
            type: 'permission_request',
            callId: raw.callId ?? raw.call_id,
            toolName: raw.toolName ?? raw.tool_name,
            input: (raw.input ?? {}) as Record<string, unknown>,
            reason: raw.reason,
            generationId: chunkGenId,
            sessionId: chunkSessionId,
            sequence: raw.sequence,
          });
        } else if (raw.type === 'tool_result') {
          // Inc 5: daemon-side execution reports one result frame per
          // approved call; tolerant snake_case reception, same as above.
          pushChunk({
            type: 'tool_result',
            callId: raw.callId ?? raw.call_id,
            toolName: raw.toolName ?? raw.tool_name,
            output: typeof raw.output === 'string' ? raw.output : '',
            isError: Boolean(raw.is_error),
            exitCode: typeof raw.exit_code === 'number' ? raw.exit_code : undefined,
            durationMs: typeof raw.duration_ms === 'number' ? raw.duration_ms : undefined,
            generationId: chunkGenId,
            sessionId: chunkSessionId,
            sequence: raw.sequence,
          });
        } else if (raw.type === 'error') {
          pushChunk({
            type: 'error',
            error: raw.error || 'Brain daemon error',
            generationId: chunkGenId,
            sessionId: chunkSessionId,
            sequence: raw.sequence,
            status: 'failed',
          });
          isStreamDone = true;
        } else if (raw.type === 'thinking_start') {
          pushChunk({
            type: 'thinking_start',
            generationId: chunkGenId,
            sessionId: chunkSessionId,
            sequence: raw.sequence,
            status: chunkStatus,
          });
        } else if (raw.type === 'thinking_end') {
          pushChunk({
            type: 'thinking_end',
            durationMs: typeof raw.duration_ms === 'number' ? raw.duration_ms : undefined,
            generationId: chunkGenId,
            sessionId: chunkSessionId,
            sequence: raw.sequence,
            status: chunkStatus,
          });
        } else if (raw.type === 'stream_start') {
          pushChunk({
            type: 'token',
            token: '',
            generationId: chunkGenId,
            sessionId: chunkSessionId,
            sequence: raw.sequence,
            status: 'in_progress',
            metadata: raw.metadata,
          });
        } else if (raw.type === 'finished' || raw.type === 'stream_end') {
          isStreamDone = true;
          pushChunk({
            type: 'finished',
            generationId: chunkGenId,
            sessionId: chunkSessionId,
            sequence: raw.sequence,
            status: raw.status || 'completed',
            metadata: raw.metadata,
          });
        }
      } catch (err: any) {
        pushChunk({
          type: 'error',
          generationId,
          sessionId: request.sessionId,
          status: 'failed',
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
          generationId,
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

  private async callRpc<T>(
    action: string,
    payload: any = {},
    timeoutMs = 10_000,
    signal?: AbortSignal,
  ): Promise<T> {
    if (!fs.existsSync(this.socketPath)) {
      throw new Error(`Brain daemon socket not found at ${this.socketPath}`);
    }
    const requestId = `req_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    return new Promise<T>((resolve, reject) => {
      let resolved = false;
      const socket = new net.Socket();

      const finishError = (err: Error) => {
        if (resolved) return;
        resolved = true;
        try {
          socket.destroy();
        } catch {}
        reject(err);
      };

      socket.once('error', (err) => {
        finishError(new Error(`Brain daemon socket error on ${action}: ${err.message}`));
      });

      socket.once('close', () => {
        finishError(new Error(`Brain daemon connection closed unexpectedly on ${action}`));
      });

      socket.setTimeout(timeoutMs);
      socket.once('timeout', () => {
        finishError(new Error(`Brain daemon RPC timeout (${timeoutMs}ms) on ${action}`));
      });

      if (signal) {
        if (signal.aborted) {
          finishError(new Error(`${action} aborted`));
          return;
        }
        signal.addEventListener(
          'abort',
          () => finishError(new Error(`${action} aborted`)),
          { once: true },
        );
      }

      try {
        socket.connect(this.socketPath);
      } catch (err: any) {
        finishError(new Error(`Brain daemon connection failed on ${action}: ${err.message}`));
        return;
      }

      const rl = readline.createInterface({
        input: socket,
        crlfDelay: Infinity,
      });

      rl.on('line', (line) => {
        if (!line || line.trim() === '') return;
        try {
          const parsed = JSON.parse(line);
          if (parsed.status === 'error' || parsed.type === 'Error') {
            finishError(new Error(parsed.body || parsed.message || 'Brain RPC error'));
            return;
          }
          if (
            parsed.status === 'success' ||
            parsed.status === 'ok' ||
            parsed.type === 'Response'
          ) {
            resolved = true;
            rl.close();
            socket.destroy();
            let resData = parsed.body ?? parsed.result ?? parsed.message;
            if (
              typeof resData === 'string' &&
              (resData.startsWith('{') || resData.startsWith('['))
            ) {
              try {
                resData = JSON.parse(resData);
              } catch (_) {}
            }
            resolve(resData as T);
          }
        } catch (e: any) {
          finishError(new Error(`Failed to parse RPC response from Brain daemon: ${e.message}`));
        }
      });

      socket.once('connect', () => {
        const frame =
          JSON.stringify({
            id: requestId,
            action,
            payload,
            body: typeof payload === 'string' ? payload : JSON.stringify(payload),
          }) + '\n';
        socket.write(frame);
      });
    });
  }

  async createSession(req?: CreateSessionRequest): Promise<CreateSessionResponse> {
    const res = await this.callRpc<any>('v1/session/create', {
      title: req?.title,
      workspace_path: req?.workspacePath,
    });
    return {
      sessionId: res.session_id || res.sessionId,
      title: res.title,
      createdAtMs: res.created_at_ms || res.createdAtMs || Date.now(),
    };
  }

  async loadSession(sessionId: string): Promise<{ session: BrainSession }> {
    const res = await this.callRpc<any>('v1/session/load', {
      session_id: sessionId,
    });
    const s = res.session || res;
    return {
      session: {
        id: s.id,
        title: s.title,
        archived: s.archived,
        pinned: s.pinned,
        updatedAtMs: s.updated_at_ms || s.updatedAtMs || s.updated_at || Date.now(),
        messages: (s.messages || []).map((m: any) => ({
          id: m.id,
          role: m.role,
          content: m.content,
          timestampMs: m.timestamp_ms || m.timestampMs || m.timestamp * 1000 || Date.now(),
        })),
        goals: s.goals || [],
      },
    };
  }

  async forkSession(req: ForkSessionRequest): Promise<ForkSessionResponse> {
    const res = await this.callRpc<any>('v1/session/fork', {
      source_session_id: req.sourceSessionId,
      new_title: req.newTitle,
      at_message_id: req.atMessageId,
    });
    return {
      newSessionId: res.new_session_id || res.newSessionId,
      sourceSessionId: res.source_session_id || res.sourceSessionId,
      clonedMessagesCount: res.cloned_messages_count || res.clonedMessagesCount || 0,
    };
  }

  async archiveSession(sessionId: string): Promise<void> {
    await this.callRpc<void>('v1/session/archive', {
      session_id: sessionId,
    });
  }

  async restoreSession(sessionId: string): Promise<void> {
    await this.callRpc<void>('v1/session/restore', {
      session_id: sessionId,
    });
  }

  async renameSession(sessionId: string, newTitle: string): Promise<void> {
    await this.callRpc<void>('v1/session/rename', {
      session_id: sessionId,
      new_title: newTitle,
    });
  }

  async pinSession(sessionId: string, pinned: boolean): Promise<void> {
    await this.callRpc<void>('v1/session/pin', {
      session_id: sessionId,
      pinned,
    });
  }

  async listAuthoritativeSessions(options?: {
    limit?: number;
    offset?: number;
    workspacePath?: string;
  }): Promise<{ sessions: AuthoritativeSessionSummary[]; total: number }> {
    const res = await this.callRpc<any>('session/list', {
      limit: options?.limit ?? 50,
      offset: options?.offset ?? 0,
      workspace_path: options?.workspacePath,
    });

    const rawList = Array.isArray(res?.sessions) ? res.sessions : Array.isArray(res) ? res : [];
    const sessions: AuthoritativeSessionSummary[] = rawList.map((s: any) => ({
      sessionId: s.sessionId || s.session_id || s.id || '',
      title: s.title || 'Session',
      messageCount: typeof s.messageCount === 'number' ? s.messageCount : typeof s.message_count === 'number' ? s.message_count : 0,
      createdAtMs: s.createdAtMs || s.created_at_ms || (s.created_at ? s.created_at * 1000 : Date.now()),
      updatedAtMs: s.updatedAtMs || s.updated_at_ms || (s.updated_at ? s.updated_at * 1000 : Date.now()),
      workspacePath: s.workspacePath || s.workspace_path,
    }));

    return {
      sessions,
      total: typeof res?.total === 'number' ? res.total : sessions.length,
    };
  }

  async loadAuthoritativeSession(sessionId: string): Promise<AuthoritativeSessionDetail> {
    const res = await this.callRpc<any>('session/load', {
      session_id: sessionId,
    });

    const s = res?.session || res;
    const rawMessages = Array.isArray(s?.messages) ? s.messages : [];

    return {
      sessionId: s?.sessionId || s?.session_id || sessionId,
      title: s?.title || 'Session',
      createdAtMs: s?.createdAtMs || s?.created_at_ms || Date.now(),
      updatedAtMs: s?.updatedAtMs || s?.updated_at_ms || Date.now(),
      workspacePath: s?.workspacePath || s?.workspace_path,
      messages: rawMessages.map((m: any) => ({
        id: m.id || `msg_${Date.now()}`,
        role: m.role === 'user' ? 'user' : m.role === 'assistant' ? 'assistant' : 'system',
        content: m.content || '',
        timestamp: typeof m.timestamp === 'number' ? m.timestamp : Math.floor(Date.now() / 1000),
      })),
    };
  }

  async triggerConsolidation(options?: {
    promotionWeightThreshold?: number;
    pruningWeightThreshold?: number;
    stalenessAgeThresholdSecs?: number;
  }): Promise<ConsolidationResult> {
    const res = await this.callRpc<any>('memory/consolidate', {
      promotion_weight_threshold: options?.promotionWeightThreshold,
      pruning_weight_threshold: options?.pruningWeightThreshold,
      staleness_age_threshold_secs: options?.stalenessAgeThresholdSecs,
    });

    return {
      actionsApplied:
        typeof res?.actionsApplied === 'number'
          ? res.actionsApplied
          : typeof res?.actions_applied === 'number'
          ? res.actions_applied
          : 0,
      promoted: typeof res?.promoted === 'number' ? res.promoted : 0,
      merged: typeof res?.merged === 'number' ? res.merged : 0,
      archived: typeof res?.archived === 'number' ? res.archived : 0,
      pruned: typeof res?.pruned === 'number' ? res.pruned : 0,
      durationMs:
        typeof res?.durationMs === 'number'
          ? res.durationMs
          : typeof res?.duration_ms === 'number'
          ? res.duration_ms
          : 0,
      errors: Array.isArray(res?.errors) ? res.errors : [],
    };
  }

  async listModels(): Promise<BrainModelDescriptor[]> {
    const res = await this.callRpc<any>('model/list', {});
    const rawModels = Array.isArray(res?.models) ? res.models : [];
    return rawModels.map((m: any) => ({
      id: m.id || '',
      name: m.name || m.id || '',
      provider: m.provider || 'unknown',
      contextWindow:
        typeof m.contextWindow === 'number'
          ? m.contextWindow
          : typeof m.context_window === 'number'
          ? m.context_window
          : 128000,
      maxOutputTokens:
        typeof m.maxOutputTokens === 'number'
          ? m.maxOutputTokens
          : typeof m.max_output_tokens === 'number'
          ? m.max_output_tokens
          : 8192,
      supportsThinking: Boolean(m.supportsThinking ?? m.supports_thinking),
      supportsTools: Boolean(m.supportsTools ?? m.supports_tools),
      isDefault: Boolean(m.isDefault ?? m.is_default),
    }));
  }

  async resolveModel(query?: string): Promise<BrainModelDescriptor> {
    const res = await this.callRpc<any>('model/resolve', {
      query: query && query.trim() !== '' ? query.trim() : null,
    });
    const m = res?.model || res;
    if (!m || !m.id) {
      throw new Error(`Failed to resolve model query '${query}'`);
    }
    return {
      id: m.id,
      name: m.name || m.id,
      provider: m.provider || 'unknown',
      contextWindow:
        typeof m.contextWindow === 'number'
          ? m.contextWindow
          : typeof m.context_window === 'number'
          ? m.context_window
          : 128000,
      maxOutputTokens:
        typeof m.maxOutputTokens === 'number'
          ? m.maxOutputTokens
          : typeof m.max_output_tokens === 'number'
          ? m.max_output_tokens
          : 8192,
      supportsThinking: Boolean(m.supportsThinking ?? m.supports_thinking),
      supportsTools: Boolean(m.supportsTools ?? m.supports_tools),
      isDefault: Boolean(m.isDefault ?? m.is_default),
    };
  }

  async listSessions(): Promise<BrainSessionSummary[]> {
    const res = await this.listAuthoritativeSessions();
    return res.sessions.map((s) => ({
      id: s.sessionId,
      title: s.title,
      updatedAtMs: s.updatedAtMs,
      pinned: false,
      archived: false,
    }));
  }

  async isAvailable(): Promise<boolean> {
    return new Promise<boolean>((resolve) => {
      const socket = net.createConnection(this.socketPath);
      let done = false;
      const finish = (ok: boolean) => {
        if (done) return;
        done = true;
        socket.destroy();
        resolve(ok);
      };
      socket.setTimeout(200);
      socket.once('connect', () => finish(true));
      socket.once('error', () => finish(false));
      socket.once('timeout', () => finish(false));
    });
  }

  async startSession(input: StartSessionInput): Promise<StartSessionOutput> {
    const res = await this.callRpc<any>('session/start', {
      session_id: input.sessionId,
      title: input.title,
      workspace_path: input.workspacePath,
    });
    return {
      sessionId: res.session_id || res.sessionId || input.sessionId,
      title: res.title || input.title || 'New Session',
      createdAtMs: res.created_at_ms || res.createdAtMs || Date.now(),
    };
  }

  async appendTurn(input: AppendTurnInput): Promise<AppendTurnOutput> {
    const textContent = typeof input.content === 'string' ? input.content : JSON.stringify(input.content);
    const res = await this.callRpc<any>('session/append_turn', {
      session_id: input.sessionId,
      turn_id: input.turnId,
      role: input.role,
      content: textContent,
      tool_results: input.toolResults,
      timestamp_ms: input.timestampMs || Date.now(),
    });
    return {
      success: res.success !== false,
      messageId: res.message_id || res.messageId || `msg_${Date.now()}`,
      sessionId: input.sessionId,
    };
  }

  async completeTurn(input: CompleteTurnInput): Promise<CompleteTurnOutput> {
    const res = await this.callRpc<any>('session/complete_turn', {
      session_id: input.sessionId,
      turn_id: input.turnId,
      assistant_response: input.assistantResponse,
      duration_ms: input.durationMs,
      input_tokens: input.inputTokens,
      output_tokens: input.outputTokens,
      timestamp_ms: input.timestampMs || Date.now(),
    });
    return {
      success: res.success !== false,
      sessionId: input.sessionId,
      totalTurns: res.total_turns || res.totalTurns || 1,
    };
  }

  async searchMemory(input: MemorySearchInput): Promise<MemorySearchOutput> {
    const res = await this.callRpc<any>('memory/search', {
      session_id: input.sessionId,
      query: input.query,
      workspace_path: input.workspacePath,
      limit: input.limit || 10,
    });
    return {
      memories: (res.memories || []).map((m: any) => ({
        node_id: m.node_id || m.id,
        label: m.label || m.title || '',
        excerpt: m.excerpt || m.content || '',
        channel: m.channel || 'default',
        score: m.score || 100,
        timestamp: m.timestamp || Date.now(),
        scope: m.scope || 'workspace',
      })),
      provenance: res.provenance || { count: (res.memories || []).length, sources: [], channels: [] },
      tokenCount: res.token_count || res.tokenCount || 0,
      serializedContext: res.serialized_context || res.serializedContext || '',
    };
  }

  async storeMemory(input: MemoryStoreInput): Promise<MemoryStoreOutput> {
    const res = await this.callRpc<any>('memory/store', {
      session_id: input.sessionId,
      label: input.label,
      content: input.content,
      scope: input.scope || 'workspace',
      relations: input.relations || [],
    });
    return {
      success: res.success !== false,
      nodeId: res.node_id || res.nodeId || `node_${Date.now()}`,
    };
  }

  async sendToolFeedback(feedback: ToolExecutionFeedback): Promise<ToolFeedbackResult> {
    const res = await this.callRpc<any>('tool/feedback', {
      identity: {
        sessionId: feedback.identity.sessionId,
        turnId: feedback.identity.turnId,
        toolUseId: feedback.identity.toolUseId,
        sequence: feedback.identity.sequence,
        timestamp: feedback.identity.timestamp || Date.now(),
      },
      tool: {
        name: feedback.tool.name,
        status: feedback.tool.status,
      },
      operation: {
        path: feedback.operation?.path,
        commandName: feedback.operation?.commandName,
        affectedPaths: feedback.operation?.affectedPaths || [],
      },
      result: {
        summary: feedback.result?.summary,
        isError: feedback.result?.isError || false,
        durationMs: feedback.result?.durationMs,
      },
      payloadHash: feedback.payloadHash,
    });

    return {
      success: res.success !== false,
      eventId: res.event_id || res.eventId || `fb_evt_${Date.now()}`,
      factsIngested: typeof res.facts_ingested === 'number' ? res.facts_ingested : (res.factsIngested || 0),
      entitiesLinked: Array.isArray(res.entities_linked) ? res.entities_linked : (res.entitiesLinked || []),
      isDuplicate: res.is_duplicate === true || res.isDuplicate === true,
    };
  }

  /**
   * Resolves a pending tool-permission request on its own short-lived UDS
   * connection — the stream occupies the stream connection's read loop, so
   * verdicts intentionally ride a separate connection.
   */
  async resolveToolPermission(callId: string, granted: boolean): Promise<void> {
    await this.callRpc<void>('v1/tool/resolve', {
      call_id: callId,
      granted,
    });
  }

  /**
   * Inc 11: executes one user-typed `!` command through the daemon's shared
   * bash tool stack on its own short-lived connection. The generous timeout
   * lets the executor's own 30 s policy bound win the race, never the socket.
   */
  async execShell(
    sessionId: string,
    command: string,
    signal?: AbortSignal,
  ): Promise<ShellExecResult> {
    const raw = await this.callRpc<any>(
      'v1/shell/exec',
      { session_id: sessionId, command },
      35_000,
      signal,
    );
    return {
      callId: typeof raw?.call_id === 'string' ? raw.call_id : '',
      name: typeof raw?.name === 'string' ? raw.name : 'bash',
      input:
        raw?.input && typeof raw.input === 'object'
          ? (raw.input as Record<string, unknown>)
          : {},
      outcome: 'executed',
      output: typeof raw?.output === 'string' ? raw.output : '',
      isError: raw?.is_error === true,
      exitCode: typeof raw?.exit_code === 'number' ? raw.exit_code : -1,
      durationMs: typeof raw?.duration_ms === 'number' ? raw.duration_ms : undefined,
    };
  }

  async retrieveContext(req: ContextRetrieveRequest): Promise<ContextRetrieveResponse> {
    const res = await this.callRpc<ContextRetrieveResponse>('v1/context/retrieve', {
      session_id: req.session_id,
      query: req.query,
      workspace_id: req.workspace_id,
      limit: req.limit,
      max_tokens: req.max_tokens,
    });
    return res;
  }

  disconnect(): void {
    // Stateless per-call UDS connection does not keep persistent open handles
  }
}
