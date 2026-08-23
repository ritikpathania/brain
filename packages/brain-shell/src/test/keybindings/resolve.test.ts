import { describe, it, expect } from 'bun:test';
import {
  DEFAULT_BINDINGS,
  resolveAction,
  strokeToKey,
} from '../../keybindings/resolve.js';
import type { KeyInfo } from '../../ui/composer/translateKey.js';

const key = (patch: Partial<KeyInfo>): KeyInfo => ({ ...patch });

describe('strokeToKey', () => {
  it('normalizes ink events into canonical key ids', () => {
    expect(strokeToKey('c', key({ ctrl: true }))).toBe('ctrl+c');
    expect(strokeToKey('o', key({ ctrl: true }))).toBe('ctrl+o');
    expect(strokeToKey('\r', key({ return: true }))).toBe('return');
    expect(strokeToKey('', key({ escape: true }))).toBe('escape');
    expect(strokeToKey('', key({ tab: true }))).toBe('tab');
    expect(strokeToKey('', key({ upArrow: true }))).toBe('up');
    expect(strokeToKey('', key({ downArrow: true }))).toBe('down');
    expect(strokeToKey('', key({ backspace: true }))).toBe('backspace');
    expect(strokeToKey('?', key({}))).toBe('?');
  });
});

describe('resolveAction', () => {
  it('resolves defaults with context precedence: specific beats global', () => {
    // ctrl+c is bound global-only; resolvable from any context list.
    expect(resolveAction(DEFAULT_BINDINGS, [], 'ctrl+c')).toBe('shell:exit');
    expect(resolveAction(DEFAULT_BINDINGS, ['palette'], 'ctrl+c')).toBe('shell:exit');
    // Unknown stroke → null.
    expect(resolveAction(DEFAULT_BINDINGS, ['composer'], '?')).toBeNull();
  });

  it('earlier (more specific) contexts win over later ones and global', () => {
    const bindings = [
      { action: 'composer:submit', context: 'composer' as const, key: 'return' },
      { action: 'palette:complete', context: 'palette' as const, key: 'tab' },
      { action: 'shell:exit', context: 'global' as const, key: 'ctrl+c' },
    ];
    expect(resolveAction(bindings, ['palette', 'composer'], 'tab')).toBe('palette:complete');
    expect(resolveAction(bindings, ['composer', 'palette'], 'return')).toBe('composer:submit');
    expect(resolveAction(bindings, ['palette'], 'ctrl+c')).toBe('shell:exit');
    expect(resolveAction(bindings, [], 'tab')).toBeNull();
  });
});
