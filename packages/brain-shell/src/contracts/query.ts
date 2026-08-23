/**
 * The single seam between the UI layer and Brain intelligence.
 * The REPL loop (Inc 1) consumes `callModel`; the daemon stays the only backend.
 *
 * Shape mirrors what the adapter actually produces: an async generator that
 * yields request lifecycle markers, content-block stream events, and a final
 * assistant message (or an error message marked isError).
 */
import type { AssistantMessage, ContentBlock, Message } from './messages.js';
import type { ThinkingConfig, Tool } from './tools.js';
import { createBrainCallModel } from '../adapter/brainCallModel.js';
import { UdsBrainBackendClient } from '../client/UdsBrainBackendClient.js';

export interface QueryParams {
  messages: Message[];
  systemPrompt?: string | string[];
  tools?: Tool[];
  thinkingConfig?: ThinkingConfig;
  options?: { model?: string };
  signal?: AbortSignal;
  sessionId?: string;
  turnId?: string;
}

export interface MessageStartEvent {
  type: 'message_start';
  message: {
    id: string;
    type: 'message';
    role: 'assistant';
    content: ContentBlock[];
    model: string;
    stop_reason: null;
    stop_sequence: null;
    usage: { input_tokens: number; output_tokens: number };
  };
}

export type StreamBlockEvent =
  | MessageStartEvent
  | {
      type: 'content_block_start';
      index: number;
      content_block:
        | { type: 'text'; text: string }
        | { type: 'thinking'; thinking: string }
        | { type: 'tool_use'; id: string; name: string; input: Record<string, unknown> };
    }
  | {
      type: 'content_block_delta';
      index: number;
      delta:
        | { type: 'text_delta'; text: string }
        | { type: 'thinking_delta'; thinking: string }
        | { type: 'signature_delta'; signature: string }
        | { type: 'input_json_delta'; partial_json: string };
    }
  | { type: 'content_block_stop'; index: number }
  | {
      type: 'message_delta';
      delta: { stop_reason: 'end_turn' | 'tool_use'; stop_sequence: null };
      usage: { output_tokens: number };
    }
  | { type: 'message_stop' };

export type QueryEvent =
  | { type: 'stream_request_start' }
  | { type: 'stream_event'; event: StreamBlockEvent }
  | AssistantMessage;

export interface QueryDeps {
  callModel(params: QueryParams): AsyncGenerator<QueryEvent, void, undefined>;
}

let cached: QueryDeps | undefined;

/** Lazily wire production deps to the UDS daemon client. */
export function getProductionDeps(): QueryDeps {
  cached ??= {
    callModel: createBrainCallModel(new UdsBrainBackendClient()) as QueryDeps['callModel'],
  };
  return cached;
}

export const productionDeps: QueryDeps = new Proxy({} as QueryDeps, {
  get(_target, prop) {
    return getProductionDeps()[prop as keyof QueryDeps];
  },
});
