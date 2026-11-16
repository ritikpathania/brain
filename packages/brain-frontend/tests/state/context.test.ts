import { describe, expect, test } from 'bun:test';

import { BrainContextState } from '../../src/brain/context.js';

const context = {
  version: 1 as const,
  retrievalEpochId: 'epoch-1',
  retrievalCount: 1,
  retrievalChannels: ['semantic'] as const,
  contextTokenCount: 24,
  sourceRefs: ['opaque-source'],
};

describe('bounded Brain context state', () => {
  test('retains only active turns and the latest successful retrieval context per session', () => {
    const state = new BrainContextState();
    state.begin('session-a', 'generation-1');
    state.record('session-a', 'generation-1', context);
    state.settle('session-a', 'generation-1', 'completed');

    state.begin('session-a', 'generation-2');
    state.record('session-a', 'generation-2', { ...context, retrievalEpochId: 'epoch-2' });
    state.settle('session-a', 'generation-2', 'completed');

    expect(state.counts()).toEqual({ active: 0, latestCompleted: 1 });
    expect(state.latestForSession('session-a')?.retrievalEpochId).toBe('epoch-2');
  });

  test('clears a prior retrieval summary after a successful zero-retrieval turn', () => {
    const state = new BrainContextState();
    state.begin('session-a', 'generation-1');
    state.record('session-a', 'generation-1', context);
    state.settle('session-a', 'generation-1', 'completed');
    state.begin('session-a', 'generation-2');
    state.record('session-a', 'generation-2', {
      ...context,
      retrievalCount: 0,
      retrievalChannels: [],
      sourceRefs: [],
    });
    state.settle('session-a', 'generation-2', 'completed');

    expect(state.latestForSession('session-a')).toBeUndefined();
    expect(state.counts()).toEqual({ active: 0, latestCompleted: 0 });
  });

  test('preserves the last observed retrieval summary when a completed turn has no context snapshot', () => {
    const state = new BrainContextState();
    state.begin('session-a', 'generation-1');
    state.record('session-a', 'generation-1', context);
    state.settle('session-a', 'generation-1', 'completed');

    state.begin('session-a', 'generation-2');
    state.settle('session-a', 'generation-2', 'completed');

    expect(state.latestForSession('session-a')).toEqual(context);
    expect(state.counts()).toEqual({ active: 0, latestCompleted: 1 });
  });

  test('does not replace the prior successful context when a turn fails or is cancelled', () => {
    const state = new BrainContextState();
    state.begin('session-a', 'generation-1');
    state.record('session-a', 'generation-1', context);
    state.settle('session-a', 'generation-1', 'completed');
    state.begin('session-a', 'generation-2');
    state.record('session-a', 'generation-2', { ...context, retrievalEpochId: 'later' });
    state.settle('session-a', 'generation-2', 'cancelled');

    expect(state.latestForSession('session-a')?.retrievalEpochId).toBe('epoch-1');
    expect(state.counts()).toEqual({ active: 0, latestCompleted: 1 });
  });
});
