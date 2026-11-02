import {
  isRecord,
  requiredNonNegativeInteger,
  requiredString,
  requiredStringArray,
  type JsonRecord,
} from './protocol/json.js';
import { safeToolText, toolAction } from './protocol/safeText.js';

export type GenerationTransport = Readonly<{
  stream(action: string, payload: unknown): AsyncGenerator<unknown, void, void>;
  requestSingleFlight(action: string, body: unknown): Promise<unknown>;
}>;

export type GenerationRequest = Readonly<{
  sessionId: string;
  generationId: string;
  messages: readonly Readonly<{ role: string; content: string }> [];
  model?: string;
}>;

export class GenerationRequestError extends Error {
  public constructor(
    public readonly code: 'malformed_cancel_response',
    message: string,
  ) {
    super(`${code}: ${message}`);
    this.name = 'GenerationRequestError';
  }
}

/** Headless generation action boundary; semantic validation stays in the guard. */
export class GenerationClient {
  public constructor(private readonly transport: GenerationTransport) {}

  public stream(request: GenerationRequest): AsyncGenerator<unknown, void, void> {
    return this.transport.stream('v1/generation/stream', {
      sessionId: request.sessionId,
      generationId: request.generationId,
      messages: request.messages,
      ...(request.model === undefined ? {} : { model: request.model }),
    });
  }

  public async cancel(correlation: Readonly<{ sessionId: string; generationId: string }>): Promise<void> {
    const response = await this.transport.requestSingleFlight('v1/generation/cancel', {
      session_id: correlation.sessionId,
      generation_id: correlation.generationId,
    });
    if (!isRecord(response) || response.type !== 'cancelled' || response.status !== 'ok') {
      throw new GenerationRequestError('malformed_cancel_response', 'daemon did not acknowledge generation cancellation');
    }
  }
}

export type BrainContextMetadata = Readonly<{
  version: 1;
  retrievalEpochId: string;
  retrievalCount: number;
  retrievalChannels: readonly ('lexical' | 'semantic' | 'graph' | 'hybrid')[];
  contextTokenCount: number;
  sourceRefs: readonly string[];
}>;

export type GenerationTerminal = 'completed' | 'cancelled' | 'failed';

export type GenerationStreamEvent = Readonly<{
  type: string;
  sequence: number;
  sessionId: string;
  generationId: string;
  terminal?: GenerationTerminal;
  context?: BrainContextMetadata;
  text?: string;
  thinking?: string;
  tool?: Readonly<{ callId: string; name: string; status: 'pending' | 'succeeded' | 'failed' | 'cancelled' | 'timed_out'; summary?: string; output?: string }>;
  permission?: Readonly<{ callId: string; permissionRequestId: string; name: string; input: Readonly<Record<string, unknown>>; summary: string }>;
  callId?: string;
  permissionRequestId?: string;
}>;

export class GenerationStreamError extends Error {
  public constructor(
    public readonly code:
      | 'malformed_frame'
      | 'foreign_frame'
      | 'sequence_mismatch'
      | 'thinking_lifecycle'
      | 'tool_correlation'
      | 'permission_correlation'
      | 'duplicate_terminal'
      | 'post_terminal'
      | 'unsafe_context_metadata',
    message: string,
  ) {
    super(`${code}: ${message}`);
    this.name = 'GenerationStreamError';
  }
}

export class GenerationStreamGuard {
  private expectedSequence = 0;
  private terminal: GenerationTerminal | undefined;
  private thinkingActive = false;
  private started = false;
  private startedContext: BrainContextMetadata | undefined;
  private readonly toolCalls = new Map<string, { name: string; input: Readonly<Record<string, unknown>>; permissionRequestId?: string; settled: boolean }>();

  public constructor(private readonly correlation: Readonly<{
    sessionId: string;
    generationId: string;
    connectionEpoch?: number;
  }>) {}

  public accept(raw: unknown, connectionEpoch = this.correlation.connectionEpoch): GenerationStreamEvent {
    if (this.correlation.connectionEpoch !== undefined && connectionEpoch !== this.correlation.connectionEpoch) {
      throw this.error('foreign_frame', 'frame belongs to a stale connection epoch');
    }
    if (this.terminal !== undefined) {
      throw this.error('post_terminal', 'received a frame after terminal settlement');
    }
    if (!isRecord(raw)) throw this.error('malformed_frame', 'frame must be an object');

    const type = requiredString(raw, 'type', message => this.error('malformed_frame', message));
    const sequence = requiredNonNegativeInteger(raw, 'sequence', message => this.error('malformed_frame', message));
    if (sequence !== this.expectedSequence) {
      throw this.error('sequence_mismatch', `expected sequence ${this.expectedSequence}, received ${sequence}`);
    }

    const sessionId = requiredString(raw, 'session_id', message => this.error('malformed_frame', message));
    const generationId = requiredString(raw, 'generation_id', message => this.error('malformed_frame', message));
    if (sessionId !== this.correlation.sessionId || generationId !== this.correlation.generationId) {
      throw this.error('foreign_frame', 'frame session or generation does not match the active stream');
    }

    const context = decodeSafeContext(raw.metadata, type);
    let callId: string | undefined;
    let permissionRequestId: string | undefined;
    let terminal: GenerationTerminal | undefined;
    let text: string | undefined;
    let thinking: string | undefined;
    let tool: GenerationStreamEvent['tool'];
    let toolName: string | undefined;
    let permission: GenerationStreamEvent['permission'];

    switch (type) {
      case 'stream_start':
        if (sequence !== 0 || this.started) throw this.error('sequence_mismatch', 'stream_start must be the first frame');
        this.started = true;
        this.startedContext = context;
        break;
      case 'thinking_start':
        this.requireStarted(type);
        if (this.thinkingActive) throw this.error('thinking_lifecycle', 'thinking is already active');
        this.thinkingActive = true;
        break;
      case 'thinking_delta':
        this.requireStarted(type);
        if (!this.thinkingActive) throw this.error('thinking_lifecycle', 'thinking delta arrived without thinking_start');
        thinking = requiredString(raw, 'thinking', message => this.error('malformed_frame', message));
        break;
      case 'thinking_end':
        this.requireStarted(type);
        if (!this.thinkingActive) throw this.error('thinking_lifecycle', 'thinking_end arrived without thinking_start');
        this.thinkingActive = false;
        break;
      case 'token':
      case 'text_delta':
        this.requireStarted(type);
        if (this.thinkingActive) throw this.error('thinking_lifecycle', 'text arrived before thinking_end');
        text = requiredString(raw, raw.token === undefined ? 'text' : 'token', message => this.error('malformed_frame', message));
        break;
      case 'tool_use':
        this.requireStarted(type);
        ({ callId, name: toolName } = decodeToolUse(raw, this.error.bind(this)));
        if (this.toolCalls.has(callId)) throw this.error('tool_correlation', `duplicate tool call '${callId}'`);
        {
          const input = Object.freeze({ ...(raw.toolUse as JsonRecord).input as JsonRecord });
          this.toolCalls.set(callId, { name: toolName, input, settled: false });
          tool = Object.freeze({ callId, name: safeToolText(toolName, 80, 1), status: 'pending', summary: toolAction(toolName, input) });
        }
        break;
      case 'tool_permission_requested':
      case 'permission_request':
        this.requireStarted(type);
        ({ callId, permissionRequestId, permission } = this.decodePermission(raw));
        break;
      case 'tool_result':
      case 'tool_denied':
        this.requireStarted(type);
        ({ callId, permissionRequestId, tool } = this.decodeToolResult(raw));
        break;
      case 'finished':
        this.requireStarted(type);
        terminal = decodeTerminal(raw, this.error.bind(this));
        if (this.thinkingActive && terminal === 'completed') throw this.error('thinking_lifecycle', 'successful terminal settlement arrived before thinking_end');
        // The daemon's cancelled/failed terminal closes unfinished thinking.
        // It need not emit thinking_end after a provider is interrupted.
        this.thinkingActive = false;
        break;
      case 'cancelled':
        this.requireStarted(type);
        this.thinkingActive = false;
        terminal = 'cancelled';
        break;
      case 'error':
        this.requireStarted(type);
        this.thinkingActive = false;
        terminal = 'failed';
        break;
      case 'stream_end':
        this.requireStarted(type);
        if (this.thinkingActive) throw this.error('thinking_lifecycle', 'stream-end arrived before thinking_end');
        if (context !== undefined && this.startedContext !== undefined && !sameContext(context, this.startedContext)) {
          throw this.error('unsafe_context_metadata', 'stream-end context does not match stream-start metadata');
        }
        break;
      default:
        throw this.error('malformed_frame', `unsupported generation frame '${type}'`);
    }

    this.expectedSequence += 1;
    if (terminal !== undefined) this.terminal = terminal;
    return Object.freeze({
      type,
      sequence,
      sessionId,
      generationId,
      ...(terminal === undefined ? {} : { terminal }),
      ...(context === undefined ? {} : { context }),
      ...(text === undefined ? {} : { text }),
      ...(thinking === undefined ? {} : { thinking }),
      ...(tool === undefined ? {} : { tool }),
      ...(permission === undefined ? {} : { permission }),
      ...(callId === undefined ? {} : { callId }),
      ...(permissionRequestId === undefined ? {} : { permissionRequestId }),
    });
  }

  public isTerminal(): boolean {
    return this.terminal !== undefined;
  }

  private decodePermission(raw: JsonRecord): { callId: string; permissionRequestId: string; permission: NonNullable<GenerationStreamEvent['permission']> } {
    const callId = requiredString(raw, 'call_id', message => this.error('permission_correlation', message));
    const permissionRequestId = requiredString(
      raw,
      'permission_request_id',
      message => this.error('permission_correlation', message),
    );
    const call = this.toolCalls.get(callId);
    if (call === undefined || call.settled || call.permissionRequestId !== undefined) {
      throw this.error('permission_correlation', `permission request does not match pending call '${callId}'`);
    }
    if (raw.tool_name !== undefined && raw.tool_name !== call.name) {
      throw this.error('permission_correlation', 'permission tool name does not match the invocation');
    }
    if ([...this.toolCalls.values()].some(known => known.permissionRequestId === permissionRequestId)) {
      throw this.error('permission_correlation', 'permission identity is already correlated to another call');
    }
    call.permissionRequestId = permissionRequestId;
    return {
      callId,
      permissionRequestId,
      permission: Object.freeze({ callId, permissionRequestId, name: call.name, input: call.input, summary: toolAction(call.name, call.input) }),
    };
  }

  private decodeToolResult(raw: JsonRecord): {
    callId: string;
    permissionRequestId: string | undefined;
    tool: NonNullable<GenerationStreamEvent['tool']>;
  } {
    const callId = requiredString(raw, 'call_id', message => this.error('tool_correlation', message));
    const call = this.toolCalls.get(callId);
    if (call === undefined || call.settled) {
      throw this.error('tool_correlation', `result does not match pending call '${callId}'`);
    }
    const permissionRequestId = raw.permission_request_id;
    if (permissionRequestId !== call.permissionRequestId) {
      throw this.error('permission_correlation', `result does not match permission for call '${callId}'`);
    }
    if (permissionRequestId !== undefined && typeof permissionRequestId !== 'string') {
      throw this.error('permission_correlation', 'permission_request_id must be a string');
    }
    const outcome = raw.outcome;
    if (raw.tool_name !== undefined && raw.tool_name !== call.name) throw this.error('tool_correlation', 'result tool name does not match invocation');
    if (typeof outcome !== 'string' || !['success', 'error', 'timed_out', 'cancelled', 'denied'].includes(outcome)
      || (raw.type === 'tool_denied' && outcome !== 'denied')
      || (raw.type === 'tool_result' && outcome === 'denied')) {
      throw this.error('tool_correlation', 'unsupported tool outcome');
    }
    if (raw.output !== undefined && typeof raw.output !== 'string') throw this.error('tool_correlation', 'tool output must be text');
    call.settled = true;
    return {
      callId,
      permissionRequestId: permissionRequestId as string | undefined,
      tool: Object.freeze({
        callId,
        name: safeToolText(call.name, 80, 1),
        status: outcome === 'success' ? 'succeeded' : outcome === 'timed_out' ? 'timed_out' : outcome === 'cancelled' ? 'cancelled' : 'failed',
        summary: outcome === 'denied' ? 'Permission denied' : outcome === 'error' ? 'Brain reported a tool failure' : undefined,
        ...(typeof raw.output === 'string' ? { output: safeToolText(raw.output) } : {}),
      }),
    };
  }

  private error(code: GenerationStreamError['code'], message: string): GenerationStreamError {
    return new GenerationStreamError(code, message);
  }

  private requireStarted(type: string): void {
    if (!this.started) throw this.error('sequence_mismatch', `${type} arrived before stream_start`);
  }
}

function decodeToolUse(
  raw: JsonRecord,
  error: (code: GenerationStreamError['code'], message: string) => GenerationStreamError,
): Readonly<{ callId: string; name: string }> {
  if (!isRecord(raw.toolUse)) throw error('tool_correlation', 'tool_use is missing toolUse');
  const callId = raw.toolUse.id;
  const name = raw.toolUse.name;
  if (typeof callId !== 'string' || callId.length === 0 || typeof name !== 'string' || name.length === 0 || !isRecord(raw.toolUse.input)) {
    throw error('tool_correlation', 'tool_use has malformed id, name, or input');
  }
  return Object.freeze({ callId, name });
}

function decodeTerminal(
  raw: JsonRecord,
  error: (code: GenerationStreamError['code'], message: string) => GenerationStreamError,
): GenerationTerminal {
  const status = raw.status;
  if (status === 'completed') return 'completed';
  if (status === 'cancelled') return 'cancelled';
  if (status === 'failed') return 'failed';
  throw error('malformed_frame', `finished frame has unsupported status '${String(status)}'`);
}

function decodeSafeContext(metadata: unknown, type?: string): BrainContextMetadata | undefined {
  if (metadata === undefined) return undefined;
  if (!isRecord(metadata)) throw new GenerationStreamError('unsafe_context_metadata', 'metadata must be an object');
  if ('memory_provenance' in metadata || (type !== 'stream_end' && type !== 'finished' && 'telemetry' in metadata)) {
    throw new GenerationStreamError('unsafe_context_metadata', 'raw provenance and telemetry are not frontend metadata');
  }
  if (metadata.brain_context === undefined) return undefined;
  if (!isRecord(metadata.brain_context)) {
    throw new GenerationStreamError('unsafe_context_metadata', 'brain_context must be an object');
  }
  const context = metadata.brain_context;
  if (context.version !== 1) {
    throw new GenerationStreamError('unsafe_context_metadata', 'brain_context version must be 1');
  }
  const channels = requiredStringArray(context, 'retrieval_channels', message => new GenerationStreamError('unsafe_context_metadata', message));
  if (channels.some(channel => !['lexical', 'semantic', 'graph', 'hybrid'].includes(channel))) {
    throw new GenerationStreamError('unsafe_context_metadata', 'brain_context includes an unknown retrieval channel');
  }
  const sourceRefs = requiredStringArray(context, 'source_refs', message => new GenerationStreamError('unsafe_context_metadata', message));
  const retrievalCount = requiredNonNegativeInteger(context, 'retrieval_count', message => new GenerationStreamError('unsafe_context_metadata', message));
  if (retrievalCount !== sourceRefs.length) {
    throw new GenerationStreamError('unsafe_context_metadata', 'retrieval_count must equal source_refs length');
  }
  return Object.freeze({
    version: 1,
    retrievalEpochId: requiredString(context, 'retrieval_epoch_id', message => new GenerationStreamError('unsafe_context_metadata', message)),
    retrievalCount,
    retrievalChannels: Object.freeze([...channels] as BrainContextMetadata['retrievalChannels']),
    contextTokenCount: requiredNonNegativeInteger(context, 'context_token_count', message => new GenerationStreamError('unsafe_context_metadata', message)),
    sourceRefs,
  });
}

function sameContext(left: BrainContextMetadata, right: BrainContextMetadata): boolean {
  return left.version === right.version
    && left.retrievalEpochId === right.retrievalEpochId
    && left.retrievalCount === right.retrievalCount
    && left.contextTokenCount === right.contextTokenCount
    && left.retrievalChannels.length === right.retrievalChannels.length
    && left.retrievalChannels.every((channel, index) => channel === right.retrievalChannels[index])
    && left.sourceRefs.length === right.sourceRefs.length
    && left.sourceRefs.every((source, index) => source === right.sourceRefs[index]);
}
