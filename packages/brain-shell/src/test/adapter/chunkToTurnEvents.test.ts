import { describe, it, expect } from 'bun:test';
import { chunkToTurnEvent } from '../../adapter/chunkToTurnEvents.js';

describe('chunkToTurnEvent', () => {
  it('maps tokens to text deltas and drops empties', () => {
    expect(chunkToTurnEvent({ type: 'token', token: 'Hi' })).toEqual({ type: 'text_delta', delta: 'Hi' });
    expect(chunkToTurnEvent({ type: 'token', token: '' })).toBeNull();
    expect(chunkToTurnEvent({ type: 'token' })).toBeNull();
  });

  it('maps thinking and redacted thinking to thinking deltas', () => {
    expect(chunkToTurnEvent({ type: 'thinking', thinking: 'hm' })).toEqual({ type: 'thinking_delta', delta: 'hm' });
    expect(chunkToTurnEvent({ type: 'redacted_thinking' })).toEqual({
      type: 'thinking_delta',
      delta: '[redacted thinking]',
    });
  });

  it('maps tool_use preserving id/name/input', () => {
    expect(chunkToTurnEvent({ type: 'tool_use', toolUse: { id: 'call_9', name: 'search', input: { q: 'x' } } }))
      .toEqual({ type: 'tool_call_requested', callId: 'call_9', toolName: 'search', input: { q: 'x' } });
    expect(chunkToTurnEvent({ type: 'tool_use' })).toBeNull();
  });

  it('maps error and finished terminators', () => {
    expect(chunkToTurnEvent({ type: 'error', error: 'socket lost' })).toEqual({ type: 'turn_error', error: 'socket lost' });
    expect(chunkToTurnEvent({ type: 'finished', status: 'completed' })).toEqual({
      type: 'turn_complete',
      stopReason: 'completed',
    });
    expect(chunkToTurnEvent({ type: 'finished' })).toEqual({ type: 'turn_complete', stopReason: undefined });
  });
});
