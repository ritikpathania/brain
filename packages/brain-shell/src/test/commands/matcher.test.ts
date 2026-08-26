import { describe, expect, test } from 'bun:test';
import {
  parseCommandQuery,
  fuzzyMatchCommands,
  type BrainCommand,
} from '../../commands/matcher.js';
import { getCommands } from '../../commands/registry.js';
import '../../commands/builtin.js'; // side-effect registration

const FIXTURE: readonly BrainCommand[] = [
  { name: 'help', description: 'List available slash commands' },
  { name: 'clear', description: 'Clear the transcript' },
  { name: 'quit', description: 'Exit Brain shell', aliases: ['q'] },
];

describe('parseCommandQuery', () => {
  test('open iff whole buffer is a bare slash token', () => {
    expect(parseCommandQuery('/')).toBe('');
    expect(parseCommandQuery('/c')).toBe('c');
    expect(parseCommandQuery('/clear')).toBe('clear');
    expect(parseCommandQuery('/clear now')).toBeNull(); // args started
    expect(parseCommandQuery('x/y')).toBeNull();
    expect(parseCommandQuery('/HE')).toBeNull(); // queries are lowercase tokens
    expect(parseCommandQuery('clear')).toBeNull();
  });
});

describe('fuzzyMatchCommands', () => {
  test('exact name > alias exact > prefix > subsequence > description', () => {
    const hits = fuzzyMatchCommands('q', FIXTURE);
    expect(hits[0]!.command.name).toBe('quit'); // alias exact 85 beats subsequence
    const pre = fuzzyMatchCommands('cl', FIXTURE);
    expect(pre[0]!.command.name).toBe('clear');
    const desc = fuzzyMatchCommands('xq', [
      { name: 'omega', description: 'List available slash commands' },
    ]);
    expect(desc).toHaveLength(0); // 'xq' misses the name AND every description word
    const descOnly = [{ name: 'xyzzy', description: 'Transcript tool' }];
    expect(fuzzyMatchCommands('tran', descOnly)[0]!.command.name).toBe('xyzzy');
  });

  test('empty query lists everything at tier 10, ties break by name', () => {
    const hits = fuzzyMatchCommands('', FIXTURE);
    expect(hits.map((h) => h.command.name)).toEqual(['clear', 'help', 'quit']);
  });

  test('no matches yields empty array', () => {
    expect(fuzzyMatchCommands('zzzz', FIXTURE)).toHaveLength(0);
  });
});

describe('palette over the canonical registry', () => {
  test('registry catalog satisfies the palette contract', () => {
    const hits = fuzzyMatchCommands('', getCommands());
    expect(hits.length).toBeGreaterThanOrEqual(6);
    expect(hits.map((h) => h.command.name)).toContain('help');
    const narrow = fuzzyMatchCommands('res', getCommands());
    expect(narrow[0]!.command.name).toBe('resume');
  });

  test('executor lookup contract over the registry', () => {
    const find = (token: string) =>
      getCommands().find((c) => c.name === token || (c.aliases ?? []).includes(token));
    expect(find('help')?.name).toBe('help');
    expect(find('q')?.name).toBe('quit'); // alias-exact
    expect(find('zzz')).toBeUndefined();  // unknown → notice path
    const prefixHits = getCommands().filter((c) => c.name.startsWith('he'));
    expect(prefixHits.map((c) => c.name)).toEqual(['help']); // unique prefix
  });
});
