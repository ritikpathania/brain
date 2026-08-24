/** Replay a stored session's messages as frozen transcript rows. */
import type { BrainSession } from '../client/BrainBackendClient.js';
import type { TranscriptRow } from '../contracts/messages.js';

export function sessionToRows(session: BrainSession): TranscriptRow[] {
  return session.messages.flatMap((m, i) => {
    const text = (m.content ?? '').trim();
    if (text.length === 0) return [];
    const id = m.id && m.id.length > 0 ? m.id : `hist:${i}`;
    if (m.role === 'user') return [{ kind: 'user' as const, id, text }];
    if (m.role === 'assistant') return [{ kind: 'assistant' as const, id, markdown: text }];
    return [{ kind: 'system' as const, id, text }];
  });
}
