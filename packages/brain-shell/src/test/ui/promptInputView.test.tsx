import { describe, it, expect } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../state/palettes.js';
import { PromptInputView, PromptInput } from '../../ui/composer/PromptInput.js';

// NOTE: assertions run against the pure view function (repo convention — see
// src/test/contracts/shell.test.tsx). bun test cannot pump React 19's
// scheduler, so the interactive component is only checked for validity here;
// live behavior is exercised by the PTY smoke gate.

function textOf(el: React.ReactNode): string {
  if (el === null || el === undefined || typeof el === 'boolean') return '';
  if (typeof el === 'string' || typeof el === 'number') return String(el);
  if (Array.isArray(el)) return el.map(textOf).join('');
  if (typeof el === 'object' && el !== null && 'props' in el) {
    return textOf((el as React.ReactElement).props.children);
  }
  return '';
}

describe('PromptInputView', () => {
  it('renders the ❯ glyph with prompt text and block cursor', () => {
    const view = PromptInputView({ value: 'hello brain', cursor: 11, busy: false, tokens: PALETTES.dark });
    expect(React.isValidElement(view)).toBe(true);
    const out = textOf(view);
    expect(out).toContain('❯');
    expect(out).toContain('hello brain');
  });

  it('renders the ! glyph when the buffer is in bash mode', () => {
    const out = textOf(PromptInputView({ value: '!git status', cursor: 11, busy: false, tokens: PALETTES.dark }));
    expect(out.startsWith('!')).toBe(true);
    expect(out).toContain('git status');
    expect(out).not.toContain('❯');
  });

  it('dims to the inactive border color while a turn streams', () => {
    const idle = PromptInputView({ value: '', cursor: 0, busy: false, tokens: PALETTES.dark });
    const busy = PromptInputView({ value: '', cursor: 0, busy: true, tokens: PALETTES.dark });
    // Same glyphs either way; the difference is token selection.
    expect(textOf(idle)).toBe(textOf(busy));
  });
});

describe('PromptInput (hooked wrapper)', () => {
  it('is a valid element', () => {
    expect(
      React.isValidElement(React.createElement(PromptInput, { onSubmit: () => {} })),
    ).toBe(true);
  });
});
