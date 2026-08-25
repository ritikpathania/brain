import { describe, it, expect } from 'bun:test';
import {
  COMMANDS,
  parseCommandQuery,
  fuzzyMatchCommands,
} from '../../commands/matcher.js';

describe('parseCommandQuery', () => {
  it('accepts a bare slash token and rejects everything else', () => {
    expect(parseCommandQuery('/')).toBe('');
    expect(parseCommandQuery('/he')).toBe('he');
    expect(parseCommandQuery('/clear')).toBe('clear');
    expect(parseCommandQuery('help')).toBeNull();      // no slash
    expect(parseCommandQuery('/he rest')).toBeNull();  // args started → menu closed
    expect(parseCommandQuery('/HE')).toBeNull();       // queries are lowercase tokens
    expect(parseCommandQuery('x/he')).toBeNull();      // slash must lead
  });
});

describe('fuzzyMatchCommands', () => {
  const cmds = [
    { name: 'help', description: 'List available slash commands' },
    { name: 'clear', description: 'Clear the transcript' },
    { name: 'quit', description: 'Exit Brain shell', aliases: ['q'] },
  ];

  it('lists everything alphabetically on empty query', () => {
    const names = fuzzyMatchCommands('', cmds).map((m) => m.command.name);
    expect(names).toEqual(['clear', 'help', 'quit']);
  });

  it('ranks prefixes above subsequences and breaks ties by name', () => {
    const extra = [...cmds, { name: 'clone', description: 'zzz' }];
    const names = fuzzyMatchCommands('c', extra).map((m) => m.command.name);
    // 'c' prefixes both clear and clone (tie → alphabetical); never quit.
    expect(names.indexOf('clone')).toBeGreaterThan(names.indexOf('clear'));
    expect(names[0]).toBe('clear');
    expect(names).not.toContain('quit'); // 'c' is not inside 'quit'
  });

  it('matches aliases and rejects misses deterministically', () => {
    expect(fuzzyMatchCommands('q', cmds)[0]!.command.name).toBe('quit');   // alias exact
    expect(fuzzyMatchCommands('hlp', cmds)[0]!.command.name).toBe('help'); // subsequence
    expect(fuzzyMatchCommands('zzz', cmds)).toEqual([]);                   // miss → []
    // description-word match still surfaces, below any name match
    const descOnly = [{ name: 'xyzzy', description: 'Transcript tool' }];
    expect(fuzzyMatchCommands('tran', descOnly)[0]!.command.name).toBe('xyzzy');
  });

  it('ships the Inc 2 command set plus the Inc 3 additions', () => {
    expect(COMMANDS.map((c) => c.name).sort()).toEqual([
      'clear',
      'help',
      'permissions',
      'quit',
      'resume',
      'theme',
    ]);
    expect(COMMANDS.find((c) => c.name === 'quit')!.aliases).toEqual(['q']);
  });
});

describe('executor lookup contract', () => {
  it('finds commands by name or alias; prefix resolves only when unique', () => {
    const find = (token: string) =>
      COMMANDS.find((c) => c.name === token || (c.aliases ?? []).includes(token));
    expect(find('help')?.name).toBe('help');
    expect(find('q')?.name).toBe('quit');     // alias-exact
    expect(find('zzz')).toBeUndefined();      // unknown → notice path
    const prefixHits = COMMANDS.filter((c) => c.name.startsWith('he'));
    expect(prefixHits.map((c) => c.name)).toEqual(['help']); // unique prefix
  });
});
