import { afterEach, expect, test } from 'bun:test';
import { mkdtempSync, mkdirSync, writeFileSync, chmodSync, symlinkSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { collectSourceIdentity } from '../../scripts/release/sourceIdentity.js';

const roots: string[] = [];
function git(root: string, ...args: string[]): void {
  const result = Bun.spawnSync(['git', '-c', 'core.fsmonitor=false', ...args], { cwd: root });
  if (result.exitCode !== 0) throw new Error(result.stderr.toString());
}
function repository(): string {
  const root = mkdtempSync(join(tmpdir(), 'brain-source-'));
  roots.push(root);
  mkdirSync(join(root, 'apps'), { recursive: true });
  writeFileSync(join(root, 'Cargo.toml'), '[workspace]\n');
  writeFileSync(join(root, 'apps', 'entry.ts'), 'original\n');
  writeFileSync(join(root, '.gitignore'), 'dist/\nnode_modules/\n.superpowers/\n');
  git(root, 'init', '-q');
  git(root, 'add', '.');
  git(root, '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'fixture');
  return root;
}
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });

test('fingerprint_is_independent_of_staging and repository absolute root', () => {
  const a = repository(); const b = repository();
  for (const root of [a, b]) writeFileSync(join(root, 'apps/entry.ts'), 'changed\n');
  git(a, 'add', 'apps/entry.ts');
  expect(collectSourceIdentity(a).fingerprint_sha256).toBe(collectSourceIdentity(b).fingerprint_sha256);
  expect(collectSourceIdentity(a).dirty).toBe(true);
  writeFileSync(join(b, 'apps/entry.ts'), 'different\n');
  expect(collectSourceIdentity(a).fingerprint_sha256).not.toBe(collectSourceIdentity(b).fingerprint_sha256);
});

test('documentation_results_do_not_feed_artifact_identity; ignored outputs remain excluded', () => {
  const root = repository(); const before = collectSourceIdentity(root);
  mkdirSync(join(root, 'docs')); writeFileSync(join(root, 'docs/evidence.md'), 'archive hash\n');
  mkdirSync(join(root, 'apps/dist')); writeFileSync(join(root, 'apps/dist/artifact'), 'generated');
  const after = collectSourceIdentity(root);
  expect(after.fingerprint_sha256).toBe(before.fingerprint_sha256);
  expect(after.dirty).toBe(true);
  expect(after.untracked_input_count).toBe(0);
});

test('untracked source, Unicode bytes, executable mode, deletion and link target affect identity', () => {
  const root = repository(); let previous = collectSourceIdentity(root).fingerprint_sha256;
  writeFileSync(join(root, 'apps/a space é.ts'), 'source\r\n');
  expect(collectSourceIdentity(root).untracked_input_count).toBe(1);
  let current = collectSourceIdentity(root).fingerprint_sha256;
  expect(current).not.toBe(previous); previous = current;
  chmodSync(join(root, 'apps/entry.ts'), 0o755);
  current = collectSourceIdentity(root).fingerprint_sha256;
  expect(current).not.toBe(previous); previous = current;
  symlinkSync('entry.ts', join(root, 'apps/link'));
  current = collectSourceIdentity(root).fingerprint_sha256;
  expect(current).not.toBe(previous); previous = current;
  rmSync(join(root, 'apps/link')); rmSync(join(root, 'apps/entry.ts'));
  expect(collectSourceIdentity(root).fingerprint_sha256).not.toBe(previous);
});

test('tracked generated files and escaping build-input symlinks are rejected', () => {
  const root = repository();
  mkdirSync(join(root, 'apps/dist')); writeFileSync(join(root, 'apps/dist/bad'), 'bad');
  git(root, 'add', '-f', 'apps/dist/bad');
  expect(() => collectSourceIdentity(root)).toThrow('tracked generated');
  git(root, 'reset', '-q', 'apps/dist/bad');
  symlinkSync('/etc/hosts', join(root, 'apps/escape'));
  expect(() => collectSourceIdentity(root)).toThrow('escaping');
});
