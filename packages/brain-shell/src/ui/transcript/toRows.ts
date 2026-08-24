import type { BrainTurnViewModel, ToolExecutionView } from '../../adapter/BrainViewModels.js';
import type { TranscriptRow, ToolCardData } from '../../contracts/messages.js';

function toolCard(t: ToolExecutionView): ToolCardData {
  return {
    callId: t.callId,
    toolName: t.toolName,
    input: t.input ?? {},
    status: t.status === 'permission_required' ? 'pending' : t.status,
    durationMs: t.durationMs,
    exitCode: t.exitCode,
    output: t.output,
    isError: t.isError,
  };
}

/** Frozen-row projection of a completed turn. Memory provenance deferred (Inc 3). */
export function turnToRows(turn: BrainTurnViewModel): TranscriptRow[] {
  const rows: TranscriptRow[] = [];
  let i = 0;
  const id = (kind: string) => `${turn.id}:${kind}:${i++}`;
  if (turn.thinking && turn.thinking.text.trim().length > 0) {
    rows.push({
      kind: 'thinking',
      id: id('thinking'),
      text: turn.thinking.text,
      durationMs: turn.thinking.durationMs,
    });
  }
  for (const t of turn.tools ?? []) {
    rows.push({ kind: 'tool', id: id('tool'), tool: toolCard(t) });
  }
  if (turn.content.trim().length > 0) {
    rows.push({ kind: 'assistant', id: id('assistant'), markdown: turn.content });
  }
  if (turn.error) {
    rows.push({ kind: 'error', id: id('error'), text: turn.error });
  }
  return rows;
}
