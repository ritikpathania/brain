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

type UserContentBlock = TextBlock | ToolResultBlock;
type AssistantContentBlock =
  | TextBlock
  | ThinkingBlock
  | RedactedThinkingBlock
  | ToolUseBlock;

interface Envelope<C> { content: string | C[] }

function textBlock(text: string): TextBlock {
  return { type: 'text', text };
}

/** Token accounting carried on assistant turns; backend may add fields. */
export interface AssistantUsage {
  input_tokens?: number;
  output_tokens?: number;
  [key: string]: unknown;
}

export interface UserMessage {
  type: 'user';
  uuid: string;
  timestamp: string;
  message: Envelope<UserContentBlock>;
}
export interface AssistantMessage {
  type: 'assistant';
  uuid: string;
  timestamp: string;
  /** True when this turn failed (kept alongside isApiErrorMessage). */
  isError?: boolean;
  /** Marker set by createAssistantAPIErrorMessage; stream folding stops here. */
  isApiErrorMessage?: boolean;
  apiError?: string;
  error?: string;
  errorDetails?: string;
  usage?: AssistantUsage;
  /** Synthetic turns (e.g. replayed history) that never hit the backend. */
  isVirtual?: true;
  message: Envelope<AssistantContentBlock>;
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

/** Placeholder rendered for assistant turns that produced no text at all. */
const NO_CONTENT_MESSAGE = '(no content)';

function asBlocks<B extends AssistantContentBlock>(content: string | B[]): B[] {
  if (typeof content !== 'string') return content;
  return [textBlock(content === '' ? NO_CONTENT_MESSAGE : content) as B];
}

export function createAssistantMessage({
  content,
  usage,
  isVirtual,
}: {
  content: string | AssistantContentBlock[];
  usage?: AssistantUsage;
  isVirtual?: true;
}): AssistantMessage {
  return {
    type: 'assistant',
    uuid: uid(),
    timestamp: now(),
    ...(usage ? { usage } : {}),
    ...(isVirtual ? { isVirtual } : {}),
    // Block arrays pass through untouched so thinking/tool_use ordering
    // survives; strings become a single text block ('' → placeholder).
    message: { content: asBlocks(content) },
  };
}

export function createAssistantAPIErrorMessage({
  content,
  apiError,
  error,
  errorDetails,
}: {
  content: string;
  apiError?: string;
  error?: string;
  errorDetails?: string;
}): AssistantMessage {
  return {
    type: 'assistant',
    uuid: uid(),
    timestamp: now(),
    isError: true,
    isApiErrorMessage: true,
    ...(apiError !== undefined ? { apiError } : {}),
    ...(error !== undefined ? { error } : {}),
    ...(errorDetails !== undefined ? { errorDetails } : {}),
    message: { content: asBlocks(content) },
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
  if (last?.type === 'assistant' && !last.isError && !last.isApiErrorMessage) {
    const blocks = last.message.content as AssistantContentBlock[];
    const tail = blocks.at(-1);
    if (tail?.type === 'text') tail.text += event.delta;
    else blocks.push(textBlock(event.delta));
    return [...messages.slice(0, -1), { ...last, message: { content: blocks } }];
  }
  return [...messages, createAssistantMessage({ content: event.delta })];
}

// ── Transcript rows (Inc 1) ────────────────────────────────────────────────
// Presentation taxonomy derived from adapter view models. Kept dependency-
// free: ToolCardData mirrors adapter/BrainViewModels.ToolExecutionView
// structurally (assignment works both ways without an import).

export interface ToolCardData {
  callId: string;
  toolName: string;
  input: Record<string, unknown>;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'denied' | 'cancelled';
  durationMs?: number;
}

export type TranscriptRow =
  | { kind: 'user'; id: string; text: string }
  | { kind: 'assistant'; id: string; markdown: string }
  | { kind: 'thinking'; id: string; text: string; durationMs?: number }
  | { kind: 'tool'; id: string; tool: ToolCardData }
  | { kind: 'error'; id: string; text: string }
  | { kind: 'system'; id: string; text: string };
