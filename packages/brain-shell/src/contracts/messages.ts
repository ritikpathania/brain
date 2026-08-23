/**
 * Brain-owned message vocabulary for the shell UI and adapter seam.
 * Wire-compatible with the shapes the Rust daemon streams (see AGENTS.md,
 * UDS Streaming Protocol) but owned here, not by any external codebase.
 */

export interface TextBlock { type: 'text'; text: string }
export interface ThinkingBlock { type: 'thinking'; thinking: string; signature?: string }
export interface RedactedThinkingBlock { type: 'redacted_thinking'; data: string }
export interface ToolUseBlock { type: 'tool_use'; id: string; name: string; input: Record<string, unknown> }
export interface ToolResultBlock {
  type: 'tool_result';
  tool_use_id: string;
  content: string;
  is_error?: boolean;
}
export type ContentBlock =
  | TextBlock
  | ThinkingBlock
  | RedactedThinkingBlock
  | ToolUseBlock
  | ToolResultBlock;

interface Envelope<B extends ContentBlock[]> { content: string | B }

export interface UserMessage {
  type: 'user';
  uuid: string;
  timestamp: string;
  message: Envelope<[TextBlock, ToolResultBlock]>;
}
export interface AssistantMessage {
  type: 'assistant';
  uuid: string;
  timestamp: string;
  isError?: boolean;
  message: Envelope<[TextBlock, ThinkingBlock, RedactedThinkingBlock, ToolUseBlock]>;
}
export interface SystemMessage {
  type: 'system';
  subtype: string;
  uuid: string;
  timestamp: string;
  data?: unknown;
}
export type Message = UserMessage | AssistantMessage | SystemMessage;

/** View-level stream events emitted by the typewriter pipeline (Inc 1 consumes these). */
export type StreamEvent =
  | { type: 'stream_start'; turnId: string }
  | { type: 'stream_progress'; turnId: string; seq: number }
  | { type: 'stream_chunk'; turnId: string; seq: number; delta: string }
  | { type: 'stream_end'; turnId: string }
  | { type: 'stream_cancelled'; turnId: string };

function uid(): string {
  return globalThis.crypto?.randomUUID?.() ?? `m_${Date.now()}_${Math.random().toString(36).slice(2)}`;
}

function now(): string {
  return new Date().toISOString();
}

export function createUserMessage(content: string): UserMessage {
  return { type: 'user', uuid: uid(), timestamp: now(), message: { content } };
}

export function createAssistantMessage(content: string): AssistantMessage {
  return {
    type: 'assistant',
    uuid: uid(),
    timestamp: now(),
    message: { content: [{ type: 'text', text: content }] },
  };
}

export function createAssistantAPIErrorMessage(error: string): AssistantMessage {
  return {
    type: 'assistant',
    uuid: uid(),
    timestamp: now(),
    isError: true,
    message: { content: [{ type: 'text', text: `Error: ${error}` }] },
  };
}

export function extractTag(text: string, tag: string): string | null {
  const m = new RegExp(`<${tag}>([\\s\\S]*?)</${tag}>`).exec(text);
  return m ? m[1] : null;
}

export function getMessagesAfterCompactBoundary(messages: readonly Message[]): Message[] {
  const idx = messages.findLastIndex(
    (m) => m.type === 'system' && (m as SystemMessage).subtype === 'compact_boundary',
  );
  return idx === -1 ? [...messages] : messages.slice(idx + 1);
}

/**
 * Fold a stream event into the transcript: text chunks append to the trailing
 * assistant message (creating one if needed); start/end/cancel are metadata-only.
 */
export function handleMessageFromStream(event: StreamEvent, messages: Message[]): Message[] {
  if (event.type !== 'stream_chunk') return messages;
  const last = messages.at(-1);
  if (last?.type === 'assistant' && !last.isError) {
    const blocks = last.message.content as ContentBlock[];
    const tail = blocks.at(-1);
    if (tail?.type === 'text') tail.text += event.delta;
    else blocks.push({ type: 'text', text: event.delta });
    return [...messages.slice(0, -1), { ...last, message: { content: blocks } }];
  }
  return [...messages, createAssistantMessage(event.delta)];
}
