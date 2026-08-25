import { describe, it, expect, beforeEach } from 'bun:test';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import {
  addAllowRule,
  describeRule,
  describeRules,
  matchingRuleIndex,
  primaryInputString,
  readAllowRules,
  removeAllowRule,
  runPermissionsCommand,
} from '../../state/permissionRules.js';

let cfgPath: string;

beforeEach(() => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-inc17-rules-'));
  cfgPath = path.join(dir, 'config.json');
  process.env.BRAIN_CONFIG_PATH = cfgPath;
});

function seed(doc: unknown): void {
  fs.writeFileSync(cfgPath, JSON.stringify(doc));
}

const GIT_RULE = { tool: 'bash', inputPrefix: 'git ' };

describe('primaryInputString', () => {
  it('prefers canonical keys in declaration order', () => {
    expect(primaryInputString({ query: 'q', command: 'c' })).toBe('c');
    expect(primaryInputString({ file_path: '/a/b', path: '/z' })).toBe('/a/b');
  });

  it('falls back to the first non-empty string value', () => {
    expect(primaryInputString({ other: '  x  ', n: 3 })).toBe('x');
  });

  it('returns empty string when no string values exist', () => {
    expect(primaryInputString({})).toBe('');
    expect(primaryInputString({ depth: 2 })).toBe('');
  });
});

describe('matchingRuleIndex', () => {
  const rules = [GIT_RULE, { tool: 'read_file', inputPrefix: '' }];

  it('matches tool plus byte-exact case-sensitive prefix', () => {
    expect(matchingRuleIndex('bash', { command: 'git status' }, rules)).toBe(0);
    expect(matchingRuleIndex('bash', { command: 'Git status' }, rules)).toBe(-1);
    expect(matchingRuleIndex('bash', { command: 'rm -rf /' }, rules)).toBe(-1);
    expect(matchingRuleIndex('write_file', { command: 'git status' }, rules)).toBe(-1);
  });

  it('treats an empty prefix as any invocation of the tool', () => {
    expect(matchingRuleIndex('read_file', { path: '/etc/hosts' }, rules)).toBe(1);
    expect(matchingRuleIndex('read_file', {}, rules)).toBe(1);
  });
});

describe('store round-trips', () => {
  it('reads [] when the file or key is missing', () => {
    expect(readAllowRules()).toEqual([]);
    seed({ theme: 'dark' });
    expect(readAllowRules()).toEqual([]);
  });

  it('filters malformed entries but keeps valid ones', () => {
    seed({
      permissions: {
        allow: [
          GIT_RULE,
          { tool: '', inputPrefix: 'x' },
          { tool: 7, inputPrefix: 'y' },
          { tool: 'ok' },
          'junk',
        ],
      },
    });
    expect(readAllowRules()).toEqual([GIT_RULE]);
  });

  it('merge-writes a rule while preserving sibling keys', () => {
    seed({ theme: 'dark', other: { nested: true } });
    addAllowRule(GIT_RULE);
    const doc = JSON.parse(fs.readFileSync(cfgPath, 'utf8'));
    expect(doc.theme).toBe('dark');
    expect(doc.other).toEqual({ nested: true });
    expect(doc.permissions.allow).toEqual([GIT_RULE]);
  });

  it('dedupes identical rules instead of appending', () => {
    addAllowRule(GIT_RULE);
    addAllowRule(GIT_RULE);
    expect(readAllowRules()).toEqual([GIT_RULE]);
  });

  it('removes by index and reports out-of-range as false', () => {
    addAllowRule(GIT_RULE);
    addAllowRule({ tool: 'read_file', inputPrefix: '' });
    expect(removeAllowRule(0)).toBe(true);
    expect(readAllowRules()).toEqual([{ tool: 'read_file', inputPrefix: '' }]);
    expect(removeAllowRule(5)).toBe(false);
    expect(removeAllowRule(-1)).toBe(false);
  });
});

describe('formatters and command output', () => {
  it('describes prefixed and tool-wide rules through one formatter', () => {
    expect(describeRule(GIT_RULE)).toBe('bash — commands starting with "git "');
    expect(describeRule({ tool: 'read_file', inputPrefix: '' })).toBe(
      'read_file — any invocation',
    );
    expect(describeRules([GIT_RULE])).toEqual([' 1. bash — commands starting with "git "']);
  });

  it('runPermissionsCommand lists, removes, and rejects bad usage', () => {
    seed({ permissions: { allow: [GIT_RULE] } });
    const out = runPermissionsCommand([]);
    expect(out).toContain(`Always-allow rules (${cfgPath}):`);
    expect(out).toContain(' 1. bash — commands starting with "git "');
    expect(out).toContain('Remove with: /permissions remove <n>');

    expect(runPermissionsCommand(['remove', '1'])).toBe(
      'Removed rule 1 (bash — commands starting with "git ").',
    );
    expect(runPermissionsCommand([])).toBe('No always-allow rules saved.');
    expect(runPermissionsCommand(['remove', '9'])).toBe('No rule 9.');
    expect(runPermissionsCommand(['remove', 'x'])).toBe(
      'Usage: /permissions remove <rule number>',
    );
    expect(runPermissionsCommand(['frobnicate'])).toBe(
      'Usage: /permissions [remove <rule number>]',
    );
  });
});
