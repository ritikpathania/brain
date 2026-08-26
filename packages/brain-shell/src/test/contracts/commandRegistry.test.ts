import { describe, expect, test } from 'bun:test';
import {
  getCommand,
  getCommands,
  registerCommand,
  type Command,
} from '../../commands/registry.js';
import '../../commands/builtin.js'; // side-effect: registers the built-in catalog

const NAMES = ['help', 'clear', 'resume', 'theme', 'permissions', 'quit'];

describe('contracts/commandRegistry (Inc 21)', () => {
  test('builtin catalog registers all six launch commands', () => {
    for (const n of NAMES) expect(getCommand(n)).toBeDefined();
  });

  test('alias q resolves to quit', () => {
    expect(getCommand('q')?.name).toBe('quit');
  });

  test('catalog is name-sorted and duplicate-free', () => {
    const all = getCommands().map((c) => c.name);
    expect(new Set(all).size).toBe(all.length);
    expect([...all].sort((a, b) => a.localeCompare(b))).toEqual(all);
  });

  test('help run returns text naming every catalog entry', () => {
    const out = getCommand('help')!.run({ args: [] });
    expect(out.type).toBe('text');
    if (out.type !== 'text') return;
    for (const n of getCommands().map((c) => c.name)) {
      expect(out.value).toContain(`/${n}`);
    }
  });

  test('clear/resume/theme/quit return declarative actions', () => {
    expect(getCommand('clear')!.run({ args: [] })).toEqual({ type: 'action', action: 'clear' });
    expect(getCommand('resume')!.run({ args: [] })).toEqual({ type: 'action', action: 'resume' });
    expect(getCommand('theme')!.run({ args: [] })).toEqual({ type: 'action', action: 'theme' });
    expect(getCommand('quit')!.run({ args: [] })).toEqual({ type: 'action', action: 'quit' });
  });

  test('permissions passes args to the rules engine and returns text', () => {
    const out = getCommand('permissions')!.run({ args: ['list'] });
    expect(out.type).toBe('text');
  });

  test('registerCommand replaces by name and resolves aliases', () => {
    const a: Command = { name: 'ping', description: 'v1', run: () => ({ type: 'none' }) };
    registerCommand(a);
    registerCommand({
      name: 'ping',
      description: 'v2',
      aliases: ['p'],
      run: () => ({ type: 'text', value: 'pong' }),
    });
    expect(getCommand('ping')?.description).toBe('v2');
    expect(getCommand('p')?.name).toBe('ping');
    expect(getCommands().filter((c) => c.name === 'ping')).toHaveLength(1);
    expect(getCommand('definitely-not-registered-xyz')).toBeUndefined();
  });
});
