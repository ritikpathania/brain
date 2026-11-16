import React from 'react';
import { describe, expect, test } from 'bun:test';
import { renderToString } from 'ink';

import { Transcript } from '../../src/presentation/components/transcript.js';
import { defaultBrainTheme } from '../../src/presentation/theme.js';

describe('terminal transcript rendering', () => {
  test('renders hydrated history, a separate thinking state, and streaming answer text', () => {
    const output = renderToString(
      <Transcript
        theme={defaultBrainTheme}
        entries={[
          { kind: 'user', id: 'u1', text: 'Hello Brain' },
          { kind: 'assistant', id: 'a1', text: 'Persisted answer', status: 'complete' },
          { kind: 'thinking', id: 't1', text: 'Considering evidence', status: 'complete' },
          { kind: 'assistant', id: 'a2', text: 'Streaming answer', status: 'streaming' },
        ]}
      />,
    );

    expect(output).toContain('You');
    expect(output).toContain('Hello Brain');
    expect(output).toContain('Brain');
    expect(output).toContain('Persisted answer');
    expect(output).toContain('Thinking');
    expect(output).toContain('Considering evidence');
    expect(output).toContain('Streaming answer');
  });

  test('renders concise cancelled, error, and reconnect states without protocol internals', () => {
    const output = renderToString(
      <Transcript
        theme={defaultBrainTheme}
        entries={[
          { kind: 'assistant', id: 'cancelled', text: '', status: 'cancelled' },
          { kind: 'error', id: 'error', message: 'Daemon unavailable. Check the Brain daemon and retry.' },
          { kind: 'status', id: 'reconnect', message: 'Reconnecting to Brain…' },
        ]}
      />,
    );

    expect(output).toContain('Generation cancelled');
    expect(output).toContain('Daemon unavailable');
    expect(output).toContain('Reconnecting to Brain');
    expect(output).not.toContain('generation_id');
    expect(output).not.toContain('sequence');
  });

  test('preserves partial answers together with their authoritative terminal labels', () => {
    const output = renderToString(<Transcript theme={defaultBrainTheme} entries={[
      { kind: 'assistant', id: 'cancelled', text: 'Partial cancelled answer', status: 'cancelled' },
      { kind: 'assistant', id: 'failed', text: 'Partial failed answer', status: 'failed' },
    ]} />);
    expect(output).toContain('Partial cancelled answer');
    expect(output).toContain('Generation cancelled');
    expect(output).toContain('Partial failed answer');
    expect(output).toContain('Generation failed');
  });
});
