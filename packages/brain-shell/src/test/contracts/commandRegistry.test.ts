import { describe, expect, test } from 'bun:test';
import { getCommand, getCommands, registerCommand } from '../../commands/registry.js';

describe('contracts/commandRegistry', () => {
  test('registers and resolves by name and alias', () => {
    registerCommand({
      name: 'ping',
      description: 'responds pong',
      aliases: ['p'],
      handler: async () => ({ type: 'text', value: 'pong' }),
    });
    expect(getCommand('ping')?.description).toBe('responds pong');
    expect(getCommand('p')?.name).toBe('ping');
    expect(getCommands().map((c) => c.name)).toContain('ping');
  });

  test('getCommand returns undefined for unknown names', () => {
    expect(getCommand('definitely-not-registered-xyz')).toBeUndefined();
  });

  test('re-registering the same name replaces the entry, not duplicates', () => {
    registerCommand({
      name: 'dup',
      description: 'v1',
      handler: async () => ({ type: 'none' }),
    });
    registerCommand({
      name: 'dup',
      description: 'v2',
      handler: async () => ({ type: 'none' }),
    });
    expect(getCommands().filter((c) => c.name === 'dup')).toHaveLength(1);
    expect(getCommand('dup')?.description).toBe('v2');
  });
});
