import { describe, expect, test } from 'bun:test';

import {
  GenerationClient,
  GenerationStreamError,
  GenerationStreamGuard,
} from '../../src/brain/generation.js';

const sessionId = '01ARZ3NDEKTSV4RRFFQ69G5FAV';
const generationId = 'generation-1';

function frame(sequence: number, type: string, extra: Record<string, unknown> = {}) {
  return {
    type,
    sequence,
    session_id: sessionId,
    generation_id: generationId,
    status: type === 'finished' ? 'completed' : 'in_progress',
    ...extra,
  };
}

const safeContext = {
  version: 1,
  retrieval_epoch_id: 'epoch-1',
  retrieval_count: 1,
  retrieval_channels: ['semantic'],
  context_token_count: 24,
  source_refs: ['opaque-ref-1'],
};

describe('generation stream guard', () => {
  test('accepts ordered thinking-to-text frames and one terminal outcome', () => {
    const guard = new GenerationStreamGuard({ sessionId, generationId });

    expect(guard.accept(frame(0, 'stream_start', { metadata: { brain_context: safeContext } })).type)
      .toBe('stream_start');
    expect(guard.accept(frame(1, 'thinking_start')).type).toBe('thinking_start');
    expect(guard.accept(frame(2, 'thinking_delta', { thinking: 'reasoning' })).type)
      .toBe('thinking_delta');
    expect(guard.accept(frame(3, 'thinking_end')).type).toBe('thinking_end');
    expect(guard.accept(frame(4, 'token', { token: 'answer' })).type).toBe('token');
    expect(guard.accept(frame(5, 'finished')).terminal).toBe('completed');
  });

  test('rejects gaps, duplicate terminals, and post-terminal content', () => {
    const guard = new GenerationStreamGuard({ sessionId, generationId });
    guard.accept(frame(0, 'stream_start', { metadata: { brain_context: safeContext } }));

    expect(() => guard.accept(frame(2, 'token', { token: 'gap' }))).toThrow(GenerationStreamError);

    guard.accept(frame(1, 'finished'));
    expect(() => guard.accept(frame(2, 'finished'))).toThrow(GenerationStreamError);
    expect(() => guard.accept(frame(2, 'token', { token: 'late' }))).toThrow(GenerationStreamError);
  });

  test('requires tool and permission frames to stay correlated to the active call', () => {
    const guard = new GenerationStreamGuard({ sessionId, generationId });
    guard.accept(frame(0, 'stream_start', { metadata: { brain_context: safeContext } }));
    guard.accept(frame(1, 'tool_use', {
      toolUse: { id: 'call-1', name: 'bash', input: { command: 'printf ok' } },
    }));
    guard.accept(frame(2, 'tool_permission_requested', {
      permission_request_id: 'permission-1',
      call_id: 'call-1',
      tool_name: 'bash',
      input: { command: 'printf ok' },
    }));

    expect(() => guard.accept(frame(3, 'tool_result', {
      permission_request_id: 'permission-1',
      call_id: 'foreign-call',
      tool_name: 'bash',
      output: 'wrong',
      outcome: 'success',
    }))).toThrow(GenerationStreamError);
  });

  test('rejects unsafe metadata instead of retaining raw provenance or telemetry', () => {
    const guard = new GenerationStreamGuard({ sessionId, generationId });

    expect(() => guard.accept(frame(0, 'stream_start', {
      metadata: {
        brain_context: safeContext,
        memory_provenance: { min_score: 0.1 },
      },
    }))).toThrow(GenerationStreamError);
  });

  test('sends cancellation only through the canonical generation action', async () => {
    const requests: Array<{ action: string; body: unknown }> = [];
    const client = new GenerationClient({
      stream: async function* () {},
      requestSingleFlight: async (action, body) => {
        requests.push({ action, body });
        return { type: 'cancelled', status: 'ok' };
      },
    });

    await client.cancel({ sessionId, generationId });
    expect(requests).toEqual([{
      action: 'v1/generation/cancel',
      body: { session_id: sessionId, generation_id: generationId },
    }]);
  });

  test('rejects a stale connection epoch before a recovered stream can mutate state', () => {
    const guard = new GenerationStreamGuard({ sessionId, generationId, connectionEpoch: 2 });

    expect(() => guard.accept(frame(0, 'stream_start', { metadata: { brain_context: safeContext } }), 1))
      .toThrow(GenerationStreamError);
  });

  test('rejects a terminal context snapshot that disagrees with stream-start metadata', () => {
    const guard = new GenerationStreamGuard({ sessionId, generationId });
    guard.accept(frame(0, 'stream_start', { metadata: { brain_context: safeContext } }));

    expect(() => guard.accept(frame(1, 'stream_end', {
      metadata: { brain_context: { ...safeContext, retrieval_epoch_id: 'other-epoch' } },
    }))).toThrow(GenerationStreamError);
  });

  test('requires stream_start before any terminal settlement', () => {
    const guard = new GenerationStreamGuard({ sessionId, generationId });

    expect(() => guard.accept(frame(0, 'finished'))).toThrow(GenerationStreamError);
  });

  test('does not allow a terminal outcome while thinking remains open', () => {
    const guard = new GenerationStreamGuard({ sessionId, generationId });
    guard.accept(frame(0, 'stream_start', { metadata: { brain_context: safeContext } }));
    guard.accept(frame(1, 'thinking_start'));

    expect(() => guard.accept(frame(2, 'finished'))).toThrow(GenerationStreamError);
  });

  test('accepts Brain cancellation as authoritative closure of an active thinking block', () => {
    const guard = new GenerationStreamGuard({ sessionId, generationId });
    guard.accept(frame(0, 'stream_start'));
    guard.accept(frame(1, 'thinking_start'));
    guard.accept(frame(2, 'thinking_delta', { thinking: 'partial thinking' }));
    expect(guard.accept(frame(3, 'finished', { status: 'cancelled' })).terminal).toBe('cancelled');
    expect(() => guard.accept(frame(4, 'thinking_delta', { thinking: 'late' }))).toThrow(GenerationStreamError);
  });
});
