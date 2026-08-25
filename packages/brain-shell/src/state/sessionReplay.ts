/** Replay a stored session's messages as frozen transcript rows. */
import type { BrainSession } from '../client/BrainBackendClient.js';
import type { TranscriptRow, ToolCardData } from '../contracts/messages.js';

/** Wire shape of the daemon's persisted tool_event envelope (Inc 8). */
interface ToolEventEnvelope {
  type?: unknown;
  v?: unknown;
  call_id?: unknown;
  name?: unknown;
  input?: unknown;
  outcome?: unknown;
  is_error?: unknown;
  exit_code?: unknown;
  output?: unknown;
  duration_ms?: unknown;
}

/**
 * Parse a persisted Inc 8 tool_event envelope into frozen card data.
 * Returns undefined when the content isn't a v1 envelope — the caller then
 * falls back to a plain system row so malformed history stays visible.
 */
function toolCardFromContent(content: string): ToolCardData | undefined {
  let env: ToolEventEnvelope;
  try {
    const parsed: unknown = JSON.parse(content);
    if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) return undefined;
    env = parsed as ToolEventEnvelope;
  } catch {
    return undefined;
  }
  const shaped =
    env.type === 'tool_event' &&
    env.v === 1 &&
    typeof env.call_id === 'string' &&
    typeof env.name === 'string' &&
    (env.outcome === 'executed' || env.outcome === 'denied');
  if (!shaped) return undefined;

  const status: ToolCardData['status'] =
    env.outcome === 'denied'
      ? 'denied'
      : env.is_error === true
        ? 'failed'
        : 'completed';
  const card: ToolCardData = {
    callId: env.call_id as string,
    toolName: env.name as string,
    input:
      typeof env.input === 'object' && env.input !== null && !Array.isArray(env.input)
        ? (env.input as Record<string, unknown>)
        : {},
    status,
  };
  if (status !== 'denied') {
    if (typeof env.output === 'string') card.output = env.output;
    if (typeof env.is_error === 'boolean') card.isError = env.is_error;
    if (typeof env.duration_ms === 'number') card.durationMs = env.duration_ms;
    if (typeof env.exit_code === 'number') card.exitCode = env.exit_code;
  }
  return card;
}

/** Wire shape of the persisted Inc 19 thinking_block envelope. */
interface ThinkingEnvelope {
  type?: unknown;
  v?: unknown;
  text?: unknown;
  duration_ms?: unknown;
}

/**
 * Parse a persisted thinking_block envelope. Returns undefined when the
 * content isn't a v1 envelope — the caller then falls back to a plain
 * system row so malformed history stays visible.
 */
function thinkingFromContent(content: string): { text: string; durationMs?: number } | undefined {
  let env: ThinkingEnvelope;
  try {
    const parsed: unknown = JSON.parse(content);
    if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) return undefined;
    env = parsed as ThinkingEnvelope;
  } catch {
    return undefined;
  }
  if (!(env.type === 'thinking_block' && env.v === 1 && typeof env.text === 'string'))
    return undefined;
  return typeof env.duration_ms === 'number'
    ? { text: env.text, durationMs: env.duration_ms }
    : { text: env.text };
}

export function sessionToRows(session: BrainSession): TranscriptRow[] {
  return session.messages.flatMap<TranscriptRow>((m, i) => {
    const text = (m.content ?? '').trim();
    if (text.length === 0) return [];
    const id = m.id && m.id.length > 0 ? m.id : `hist:${i}`;
    if (m.role === 'user') return [{ kind: 'user' as const, id, text }];
    if (m.role === 'assistant') return [{ kind: 'assistant' as const, id, markdown: text }];
    if (m.role === 'tool') {
      const card = toolCardFromContent(text);
      if (card !== undefined) return [{ kind: 'tool' as const, id, tool: card }];
    }
    if (m.role === 'thinking') {
      const th = thinkingFromContent(text);
      if (th !== undefined) {
        const row: Extract<TranscriptRow, { kind: 'thinking' }> = {
          kind: 'thinking',
          id,
          text: th.text,
          collapsed: true,
        };
        if (th.durationMs !== undefined) row.durationMs = th.durationMs;
        return [row];
      }
    }
    return [{ kind: 'system' as const, id, text }];
  });
}
