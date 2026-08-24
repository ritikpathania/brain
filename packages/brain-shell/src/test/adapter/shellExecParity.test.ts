import { describe, it, expect } from 'bun:test';
import { BrainTurnTransformer } from '../../adapter/BrainTurnTransformer.js';
import type { BrainTurnEvent } from '../../adapter/BrainTurnEvents.js';
import { turnToRows } from '../../ui/transcript/toRows.js';
import { sessionToRows } from '../../state/sessionReplay.js';
import type { BrainSession } from '../../client/BrainBackendClient.js';

/**
 * Inc 11: the card runShellCommand projects locally must equal the frozen
 * card sessionToRows replays from the persisted envelope — the same
 * guarantee Inc 10 proved for agentic cards, extended to user-initiated
 * `!` turns.
 */
function envelope(command: string, exitCode: number, durationMs: number, output: string): string {
  return JSON.stringify({
    type: 'tool_event',
    v: 1,
    call_id: 'shell-parity',
    name: 'bash',
    input: { command },
    outcome: 'executed',
    is_error: exitCode !== 0,
    exit_code: exitCode,
    output,
    duration_ms: durationMs,
  });
}

function localCard(events: BrainTurnEvent[]) {
  const rows = turnToRows(BrainTurnTransformer.transform(events));
  const card = rows.find((r) => r.kind === 'tool');
  if (card?.kind !== 'tool') throw new Error('local side produced no tool row');
  return card.tool;
}

function replayedCard(command: string, exitCode: number, durationMs: number, output: string) {
  const session: BrainSession = {
    id: 's',
    title: 't',
    createdAtMs: 0,
    updatedAtMs: 0,
    pinned: false,
    archived: false,
    messages: [
      { id: 'm0', role: 'user', content: `! ${command}` },
      { id: 'm1', role: 'tool', content: envelope(command, exitCode, durationMs, output) },
    ],
  };
  const card = sessionToRows(session).find((r) => r.kind === 'tool');
  if (card?.kind !== 'tool') throw new Error('replay produced no tool row');
  return card.tool;
}

describe('shell-exec live/replay parity', () => {
  it('successful command: local projection deep-equals replayed envelope card', () => {
    const events: BrainTurnEvent[] = [
      { type: 'tool_call_requested', callId: 'shell-parity', toolName: 'bash', input: { command: 'echo bang' } },
      { type: 'tool_result', callId: 'shell-parity', output: 'bang\n', isError: false, exitCode: 0, durationMs: 88 },
    ];
    expect(localCard(events)).toEqual(replayedCard('echo bang', 0, 88, 'bang\n'));
  });

  it('failed command: both sides agree on failed status and exit code', () => {
    const events: BrainTurnEvent[] = [
      { type: 'tool_call_requested', callId: 'shell-parity', toolName: 'bash', input: { command: 'false' } },
      { type: 'tool_result', callId: 'shell-parity', output: '', isError: true, exitCode: 1, durationMs: 15 },
    ];
    expect(localCard(events)).toEqual(replayedCard('false', 1, 15, ''));
  });
});
