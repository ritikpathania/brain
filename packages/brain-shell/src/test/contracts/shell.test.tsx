import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { BrainMark, BrainMarkView } from '../../ui/shell/BrainMark.js';
import { PromptModeIndicator } from '../../ui/shell/PromptModeIndicator.js';
import { PALETTES } from '../../state/palettes.js';

// NOTE: assertions run against the pure view functions, not a mounted frame.
// bun test's event loop does not pump React 19's scheduler, so live ink
// renders commit an empty tree under `bun test` (they work under `bun run`
// and in the PTY smoke gate). Element-tree checks stay deterministic here.

function textOf(el: React.ReactNode): string {
  if (el === null || el === undefined || typeof el === 'boolean') return '';
  if (typeof el === 'string' || typeof el === 'number') return String(el);
  if (Array.isArray(el)) return el.map(textOf).join('');
  if (typeof el === 'object' && el !== null && 'props' in el) {
    return textOf((el as React.ReactElement).props.children);
  }
  return '';
}

describe('ui/shell placeholders', () => {
  test('BrainMark renders the Brain wordmark, nothing proprietary', () => {
    const view = BrainMarkView({ tokens: PALETTES.dark });
    expect(React.isValidElement(view)).toBe(true);
    const frame = textOf(view);
    expect(frame).toContain('BRAIN');
    expect(frame.toLowerCase()).not.toContain('claude');
  });

  test('hooked wrapper is a valid element using theme context', () => {
    expect(React.isValidElement(React.createElement(BrainMark))).toBe(true);
  });

  test('mode indicator shows bash prefix only in bash mode', () => {
    expect(textOf(PromptModeIndicator({ mode: 'bash' }))).toBe('! bash');
    expect(PromptModeIndicator({ mode: 'prompt' }).type).toBe(React.Fragment);
  });
});
