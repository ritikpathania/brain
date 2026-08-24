/**
 * Inc 10: live/resumed card parity.
 *
 * The live pipeline (UDS chunk → turn event → transformer → frozen rows) and
 * the replayed pipeline (persisted tool_event envelope → sessionToRows) must
 * produce the SAME tool card for the same execution. The daemon owns one
 * clock: duration_ms measured in handlers.rs feeds both the tool_result wire
 * frame and the persisted envelope (Inc 8), so neither view approximates.
 */
import { describe, expect, test } from 'bun:test';
import { chunkToTurnEvent } from '../../adapter/chunkToTurnEvents.js';
import { BrainTurnTransformer } from '../../adapter/BrainTurnTransformer.js';
import type { BrainTurnEvent } from '../../adapter/BrainTurnEvents.js';
import { turnToRows } from '../../ui/transcript/toRows.js';
import { sessionToRows } from '../../state/sessionReplay.js';
import type { BrainSession } from '../../client/BrainBackendClient.js';

const INPUT = { command: 'false' };
/** Mirrors the daemon's handlers.rs envelope field-for-field (Inc 8). */
const ENVELOPE = {
  call_id: 'call_p',
  name: 'bash',
  input: INPUT,
  outcome: 'executed',
  is_error: true,
  exit_code: 2,
  output: 'boom',
  duration_ms: 137,
};

function liveToolCard(): Record<string, unknown> | undefined {
  let vm = BrainTurnTransformer.createInitial('turn_live');
  const events: BrainTurnEvent[] = [
    { type: 'tool_call_requested', callId: ENVELOPE.call_id, toolName: ENVELOPE.name, input: INPUT },
    {
      type: chunkToTurnEvent({
        type: 'tool_result',
        callId: ENVELOPE.call_id,
        output: ENVELOPE.output,
        isError: ENVELOPE.is_error,
        exitCode: ENVELOPE.exit_code,
        durationMs: ENVELOPE.duration_ms,
      })!.type,
      callId: ENVELOPE.call_id,
      output: ENVELOPE.output,
      isError: ENVELOPE.is_error,
      exitCode: ENVELOPE.exit_code,
      durationMs: ENVELOPE.duration_ms,
    },
  ];
  for (const e of events) vm = BrainTurnTransformer.reduce(vm, e);
  const rows = turnToRows(vm).filter((r) => r.kind === 'tool');
  const card = rows[0] && rows[0].kind === 'tool' ? rows[0].tool : undefined;
  return card ? (JSON.parse(JSON.stringify(card)) as Record<string, unknown>) : undefined;
}

function resumedToolCard(): Record<string, unknown> | undefined {
  const session: BrainSession = {
    id: 'sess-p',
    title: 't',
    createdAtMs: 0,
    updatedAtMs: 0,
    pinned: false,
    archived: false,
    messages: [
      {
        id: 'm1',
        role: 'tool',
        content: JSON.stringify({ type: 'tool_event', v: 1, ...ENVELOPE }),
      },
    ],
  };
  const rows = sessionToRows(session).filter((r) => r.kind === 'tool');
  const card = rows[0] && rows[0].kind === 'tool' ? rows[0].tool : undefined;
  return card ? (JSON.parse(JSON.stringify(card)) as Record<string, unknown>) : undefined;
}

describe('live/resumed card parity (Inc 10)', () => {
  test('failed execution: live pipeline card equals its own replayed card', () => {
    expect(liveToolCard()).toEqual(resumedToolCard());
    // And both carry what the status line needs — no approximation either way.
    expect(liveToolCard()).toMatchObject({
      status: 'failed',
      durationMs: 137,
      exitCode: 2,
    });
  });

  test('successful execution stays in parity too', () => {
    const envelope = { ...ENVELOPE, is_error: false, exit_code: 0, output: 'hi\n' };
    const session: BrainSession = {
      id: 's2',
      title: 't',
      createdAtMs: 0,
      updatedAtMs: 0,
      pinned: false,
      archived: false,
      messages: [
        { id: 'm1', role: 'tool', content: JSON.stringify({ type: 'tool_event', v: 1, ...envelope }) },
      ],
    };
    const resumedRows = sessionToRows(session).filter((r) => r.kind === 'tool');
    const resumed = resumedRows[0] && resumedRows[0].kind === 'tool' ? resumedRows[0].tool : undefined;
    let vm = BrainTurnTransformer.createInitial('turn_live');
    vm = BrainTurnTransformer.reduce(vm, {
      type: 'tool_call_requested',
      callId: envelope.call_id,
      toolName: envelope.name,
      input: INPUT,
    });
    vm = BrainTurnTransformer.reduce(vm, {
      type: 'tool_result',
      callId: envelope.call_id,
      output: envelope.output,
      isError: envelope.is_error,
      exitCode: envelope.exit_code,
      durationMs: envelope.duration_ms,
    });
    const liveRows = turnToRows(vm).filter((r) => r.kind === 'tool');
    const live = liveRows[0] && liveRows[0].kind === 'tool' ? liveRows[0].tool : undefined;
    expect(JSON.parse(JSON.stringify(live))).toEqual(JSON.parse(JSON.stringify(resumed)));
  });
});
