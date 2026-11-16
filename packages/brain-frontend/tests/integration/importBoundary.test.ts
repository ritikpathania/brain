import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const sourceRoot = join(import.meta.dir, '../../src');
const forbiddenImportPatterns = [
  /from\s+['"][^'"]*(?:claude-code|claude-analysis|anthropic)[^'"]*['"]/i,
  /import\s*\(\s*['"][^'"]*(?:claude-code|claude-analysis|anthropic)[^'"]*['"]\s*\)/i,
  /\/Users\/ritikpathania\/(?:Developer\/claude-code|Desktop\/claude-analysis)/,
  /BrainToClaude/i,
];

describe('owned frontend source boundary', () => {
  test('contains no imports from private Claude or reverse-engineering reference trees', () => {
    const violations = filesUnder(sourceRoot).flatMap(path => {
      const source = readFileSync(path, 'utf8');
      return forbiddenImportPatterns
        .filter(pattern => pattern.test(source))
        .map(pattern => `${path}: ${pattern}`);
    });

    expect(violations).toEqual([]);
  });

  test('keeps frontend-neutral state independent from presentation modules', () => {
    const stateRoot = join(sourceRoot, 'state');
    const violations = filesUnder(stateRoot).flatMap(path => {
      const source = readFileSync(path, 'utf8');
      return /from\s+['"][^'"]*presentation\//.test(source) ? [path] : [];
    });

    expect(violations).toEqual([]);
  });

  test('terminal presentation never imports wire decoders or backend clients', () => {
    const violations = filesUnder(join(sourceRoot, 'presentation')).filter(path =>
      /(?:from\s*|import\s*\()\s*['"][^'"]*(?:\/brain\/|\/protocol\/|\/transport\/)[^'"]*['"]/.test(readFileSync(path, 'utf8')));
    expect(violations).toEqual([]);
  });
});

function filesUnder(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return filesUnder(path);
    return entry.isFile() && (path.endsWith('.ts') || path.endsWith('.tsx')) ? [path] : [];
  });
}
