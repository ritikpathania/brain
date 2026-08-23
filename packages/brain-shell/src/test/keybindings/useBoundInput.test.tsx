import { describe, it, expect } from 'bun:test';
import { DEFAULT_BINDINGS, resolveAction, strokeToKey } from '../../keybindings/resolve.js';

// useBoundInput is a thin composition of useInput + strokeToKey +
// resolveAction; per repo convention (pure functions unit-tested, hooked
// wrappers verified via PTY smoke) this pins the dispatch decision it makes:
// resolved → handler fires once; unresolved → ignored.
describe('useBoundInput dispatch rule', () => {
  const decide = (input: string, key: Parameters<typeof strokeToKey>[1]): string | null =>
    resolveAction(DEFAULT_BINDINGS, ['global'], strokeToKey(input, key));

  it('fires shell actions on their bound strokes', () => {
    expect(decide('c', { ctrl: true })).toBe('shell:exit');
    expect(decide('o', { ctrl: true })).toBe('shell:toggleTools');
  });

  it('ignores unbound strokes so they cannot double-handle', () => {
    expect(decide('x', { ctrl: true })).toBeNull();
    expect(decide('t', {})).toBeNull();
  });
});
