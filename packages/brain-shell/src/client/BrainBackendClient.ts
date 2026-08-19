/**
 * Transport-Agnostic Brain Backend Client Interface (Phase 5.5 — Thinking Support)
 *
 * Defines the clean boundary between the TypeScript CallModel adapter
 * and Brain backend execution runtimes, including reasoning / thinking blocks.
 */

export interface BrainToolDefinition {
  name: string;
  description: string;
  inputSchema?: Record<string, unknown>;
}

export interface BrainToolUseBlock {
  type: 'tool_use';
  id: string;
  name: string;
  input: Record<string, unknown>;
}

export interface BrainToolResultBlock {
  type: 'tool_result';
  tool_use_id: string;
  content: string;
  is_error?: boolean;
}

export interface BrainTextBlock {
  type: 'text';
  text: string;
}

export interface BrainThinkingBlock {
  type: 'thinking';
  thinking: string;
  signature?: string;
}

export interface BrainRedactedThinkingBlock {
  type: 'redacted_thinking';
  data: string;
}

export type BrainContentBlock =
  | BrainTextBlock
  | BrainThinkingBlock
  | BrainRedactedThinkingBlock
  | BrainToolUseBlock
  | BrainToolResultBlock;

export interface BrainChatMessage {
  role: 'user' | 'assistant' | 'system';
  content: string | BrainContentBlock[];
}

export interface BrainThinkingConfig {
  mode: 'adaptive' | 'enabled' | 'disabled';
  budgetTokens?: number;
}

export interface BrainGenerationRequest {
  sessionId?: string;
  messages: BrainChatMessage[];
  systemPrompt?: string;
  tools?: BrainToolDefinition[];
  thinkingConfig?: BrainThinkingConfig;
  model?: string;
  signal?: AbortSignal;
}

export interface BrainStreamChunk {
  type: 'token' | 'thinking' | 'redacted_thinking' | 'tool_use' | 'error' | 'finished';
  token?: string;
  thinking?: string;
  signature?: string;
  redactedData?: string;
  toolUse?: {
    id: string;
    name: string;
    input: Record<string, unknown>;
  };
  error?: string;
  metadata?: {
    model?: string;
    inputTokens?: number;
    outputTokens?: number;
  };
}

export interface BrainBackendClient {
  /**
   * Stream text tokens, thinking tokens, or tool calls sequentially from the Brain backend.
   */
  streamText(request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk>;
}

/**
 * Configurable Test Double supporting functional generators.
 */
export class MockBrainBackendClient implements BrainBackendClient {
  constructor(
    private handlerOrTokens?:
      | string[]
      | ((request: BrainGenerationRequest) => AsyncIterable<BrainStreamChunk> | BrainStreamChunk[]),
    private emitError?: string
  ) {}

  async *streamText(request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
    if (this.emitError) {
      yield { type: 'error', error: this.emitError };
      return;
    }

    if (typeof this.handlerOrTokens === 'function') {
      for await (const chunk of this.handlerOrTokens(request)) {
        if (request.signal?.aborted) break;
        yield chunk;
      }
      return;
    }

    if (Array.isArray(this.handlerOrTokens)) {
      let inputTokens = request.messages.reduce(
        (acc, m) => acc + (typeof m.content === 'string' ? m.content.length : 10),
        0
      );
      for (const token of this.handlerOrTokens) {
        if (request.signal?.aborted) break;
        yield {
          type: 'token',
          token,
          metadata: {
            model: request.model || 'brain-engine-v1',
            inputTokens,
            outputTokens: this.handlerOrTokens.length,
          },
        };
      }
      yield { type: 'finished' };
      return;
    }

    yield {
      type: 'token',
      token: 'Default mock response',
      metadata: { model: 'mock-engine', inputTokens: 10, outputTokens: 3 },
    };
    yield { type: 'finished' };
  }
}
