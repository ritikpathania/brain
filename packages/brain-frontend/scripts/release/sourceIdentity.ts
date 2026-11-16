import { createHash } from 'node:crypto';
import { lstatSync, readFileSync, readlinkSync, realpathSync } from 'node:fs';
import { dirname, isAbsolute, relative, resolve } from 'node:path';

export type SourceIdentity = Readonly<{
  sha: string;
  dirty: boolean;
  fingerprint_algorithm: 'brain-source-tree-v1';
  fingerprint_sha256: string;
  untracked_input_count: number;
}>;

const prefixes = ['.cargo/', '.github/workflows/', 'apps/', 'crates/', 'daemon/', 'xtask/',
  'generated/', 'sdks/', 'packages/brain-frontend/', 'scripts/release/'];
const exact = new Set(['Cargo.toml', 'Cargo.lock', 'LICENSE', '.gitignore', 'package.sh',
  'rust-toolchain', 'rust-toolchain.toml', 'scripts/package-daemon-runtime.sh',
  'scripts/verify-daemon-runtime.sh', 'scripts/package-brain-product.sh',
  'scripts/verify-frontend-artifact.sh', 'scripts/verify-brain-product.sh']);
const outputComponents = new Set(['.git', 'target', 'node_modules', 'dist', '.venv',
  '__pycache__', '.pytest_cache', '.ruff_cache', '.ty_cache', '.mypy_cache', '.code-review-graph']);
const decoder = new TextDecoder('utf-8', { fatal: true });
function generated(path: string): boolean {
  return path.split('/').some(part => outputComponents.has(part)) ||
    path.startsWith('artifacts/') || path.startsWith('release_assets/') ||
    (!path.includes('/') && /\.(tar\.gz|sha256)$/.test(path));
}
function contained(root: string, path: string): boolean {
  const rel = relative(root, path);
  return rel === '' || (!isAbsolute(rel) && rel !== '..' && !rel.startsWith('../'));
}
function git(root: string, ...args: string[]): Buffer {
  const result = Bun.spawnSync(['git', '-c', 'core.fsmonitor=false', ...args], { cwd: root });
  if (result.exitCode !== 0) throw new Error(`source identity Git failure: ${result.stderr.toString()}`);
  return result.stdout;
}
function paths(buffer: Buffer): string[] { return decoder.decode(buffer).split('\0').filter(Boolean); }
function validatePath(path: string): void {
  if (isAbsolute(path) || path.split('/').some(part => part === '..' || part === '') || path.includes('\n')) {
    throw new Error(`invalid source input path: ${JSON.stringify(path)}`);
  }
}
export function collectSourceIdentity(repositoryRoot: string): SourceIdentity {
  const root = realpathSync(repositoryRoot);
  if (realpathSync(decoder.decode(git(root, 'rev-parse', '--show-toplevel')).trim()) !== root) {
    throw new Error('source identity requires a repository root');
  }
  const sha = decoder.decode(git(root, 'rev-parse', '--verify', 'HEAD')).trim();
  if (!/^[0-9a-f]{40}$/.test(sha)) throw new Error('source identity requires a full SHA');
  const tracked = new Set([...paths(git(root, 'ls-tree', '-r', '-z', '--name-only', 'HEAD')),
    ...paths(git(root, 'ls-files', '--cached', '-z'))]);
  const untracked = new Set(paths(git(root, 'ls-files', '--others', '--exclude-standard', '-z')));
  // Status is not part of the content hash. Rename records carry a second path.
  const status = paths(git(root, 'status', '--porcelain=v1', '-z', '--untracked-files=all'));
  let dirty = false;
  for (let index = 0; index < status.length; index++) {
    const entry = status[index];
    if (!generated(entry.slice(3))) dirty = true;
    if (/[RC]/.test(entry.slice(0, 2))) {
      const oldPath = status[++index];
      if (oldPath !== undefined && !generated(oldPath)) dirty = true;
    }
  }
  const candidates = [...new Set([...tracked, ...untracked])].filter(path => {
    validatePath(path);
    if (generated(path)) {
      if (tracked.has(path)) throw new Error(`tracked generated input requires review: ${path}`);
      return false;
    }
    return exact.has(path) || prefixes.some(prefix => path.startsWith(prefix));
  }).sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  const hash = createHash('sha256').update('brain-source-tree-v1\n');
  let untrackedInputCount = 0;
  for (const path of candidates) {
    const absolute = resolve(root, path);
    let stat;
    try { stat = lstatSync(absolute); } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') continue;
      throw error;
    }
    if (!contained(root, realpathSync(dirname(absolute)))) throw new Error(`escaping source parent: ${path}`);
    let kind: 'file' | 'symlink'; let mode: string; let bytes: Buffer;
    if (stat.isSymbolicLink()) {
      kind = 'symlink'; mode = '120000'; bytes = readlinkSync(absolute, { encoding: 'buffer' });
      const target = decoder.decode(bytes);
      if (isAbsolute(target) || !contained(root, resolve(dirname(absolute), target))) {
        throw new Error(`escaping source symlink: ${path}`);
      }
      // Existing targets must not escape through another symlink, either.
      try {
        if (!contained(root, realpathSync(absolute))) throw new Error(`escaping source symlink: ${path}`);
      } catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error; }
    } else if (stat.isFile()) {
      kind = 'file'; mode = (stat.mode & 0o111) ? '100755' : '100644'; bytes = readFileSync(absolute);
    } else { throw new Error(`special node/submodule is not a source input: ${path}`); }
    const digest = createHash('sha256').update(bytes).digest('hex');
    hash.update(`${path}\0${kind}\0${mode}\0${digest}\n`);
    if (untracked.has(path) && !tracked.has(path)) untrackedInputCount++;
  }
  return Object.freeze({ sha, dirty, fingerprint_algorithm: 'brain-source-tree-v1',
    fingerprint_sha256: hash.digest('hex'), untracked_input_count: untrackedInputCount });
}
