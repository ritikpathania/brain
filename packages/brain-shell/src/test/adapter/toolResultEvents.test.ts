import { describe, expect, test } from 'bun:test';
import { chunkToTurnEvent } from '../../adapter/chunkToTurnEvents.js';

describe('tool_result chunk mapping', () => {
  test('maps to the existing tool_result turn event', () => {
    const event = chunkToTurnEvent({
      type: 'tool_result',
      callId: 'call_tr',
      output: 'hi\n',
      isError: false,
    });
    expect(event).toEqual({
      type: 'tool_result',
      callId: 'call_tr',
      output: 'hi\n',
      isError: undefined,
    });
  });

  test('missing output maps to empty string, never null', () => {
    const event = chunkToTurnEvent({ type: 'tool_result', callId: 'c2' });
    expect(event).not.toBeNull();
    expect((event as { output: string }).output).toBe('');
  });

  test('missing callId is dropped (renderer never crashes on bad frames)', () => {
    expect(chunkToTurnEvent({ type: 'tool_result' })).toBeNull();
  });

  test('carries daemon-measured duration and exit code onto the event (Inc 10)', () => {
    const event = chunkToTurnEvent({
      type: 'tool_result',
      callId: 'call_tr',
      output: 'boom',
      isError: true,
      exitCode: 2,
      durationMs: 137,
    });
    expect(event).toEqual({
      type: 'tool_result',
      callId: 'call_tr',
      output: 'boom',
      isError: true,
      exitCode: 2,
      durationMs: 137,
    });
  });

  test('absent measurement fields stay undefined (old daemons keep working)', () => {
    const event = chunkToTurnEvent({ type: 'tool_result', callId: 'c3' }) as Record<string, unknown>;
    expect(event['exitCode']).toBeUndefined();
    expect(event['durationMs']).toBeUndefined();
  });
});
