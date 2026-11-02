import { createHash } from 'node:crypto';
import { existsSync, mkdtempSync, mkdirSync, readFileSync, readdirSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import type { BuildIdentity } from '../src/cli/buildIdentity.js';
import { collectSourceIdentity, type SourceIdentity } from './release/sourceIdentity.js';

const packageRoot = resolve(import.meta.dir, '..');
const targets: Readonly<Record<string, string>> = {
  'aarch64-apple-darwin': 'bun-darwin-arm64',
  'x86_64-unknown-linux-gnu': 'bun-linux-x64',
};
export function validateBunVersion(version: string): void {
  if (version !== '1.4.4') throw new Error(`standalone build requires Bun 1.4.4, got ${version}`);
}
function readBrainVersion(root: string): string {
  if (existsSync(join(root, 'Cargo.toml'))) {
    const cargo = Bun.TOML.parse(readFileSync(join(root, 'Cargo.toml'), 'utf8')) as { workspace?: { package?: { version?: string } } };
    if (cargo.workspace?.package?.version) return cargo.workspace.package.version;
  }
  for (const path of ['apps/brain/Cargo.toml', 'daemon/Cargo.toml']) {
    if (existsSync(join(root, path))) {
      const cargo = Bun.TOML.parse(readFileSync(join(root, path), 'utf8')) as { package?: { version?: string } };
      if (cargo.package?.version) return cargo.package.version;
    }
  }
  throw new Error('cannot resolve brain_version from Cargo.toml');
}
export function nativeTarget(): string {
  if (process.platform === 'darwin' && process.arch === 'arm64') return 'aarch64-apple-darwin';
  if (process.platform === 'linux' && process.arch === 'x64') return 'x86_64-unknown-linux-gnu';
  throw new Error('unsupported native standalone target');
}
export function assertSourceUnchanged(before: SourceIdentity, after: SourceIdentity): void {
  if (JSON.stringify(before) !== JSON.stringify(after)) throw new Error('source inputs changed during build; rebuild from one snapshot');
}
export function sanitizedBuildEnvironment(): NodeJS.ProcessEnv {
  const env = { ...process.env };
  for (const key of ['BUN_BE_BUN', 'BUN_OPTIONS', 'NODE_OPTIONS']) delete env[key];
  return env;
}
export function standaloneArguments(entrypoint: string, output: string, target: string, identity: BuildIdentity): string[] {
  return ['build', entrypoint, '--no-env-file', '--compile', '--production', '--minify', '--sourcemap=none',
    `--target=${targets[target]}`, `--outfile=${output}`, '--no-compile-autoload-dotenv',
    '--no-compile-autoload-bunfig', '--no-compile-autoload-tsconfig', '--no-compile-autoload-package-json',
    '--reject-unresolved', '--define', `__BRAIN_COMPILED_IDENTITY__=${JSON.stringify(identity)}`];
}
function digest(path: string): string { return createHash('sha256').update(readFileSync(path)).digest('hex'); }
function validatePreviousOutput(output: string): void {
  if (!existsSync(output)) return;
  const entries = readdirSync(output).sort();
  if (entries.length === 0) return;
  if (JSON.stringify(entries) !== JSON.stringify(['brain-frontend', 'manifest.json'])) {
    throw new Error('refusing nonempty output that is not an owned frontend artifact');
  }
  const manifest = JSON.parse(readFileSync(join(output, 'manifest.json'), 'utf8'));
  if (manifest.schema_version !== 1 || manifest.identity !== 'brain-frontend' || manifest.package !== '@brain/frontend' ||
    manifest.entrypoint !== 'brain-frontend' || manifest.executable_sha256 !== digest(join(output, 'brain-frontend'))) {
    throw new Error('refusing replacement of an invalid frontend artifact');
  }
}
export async function buildStandalone(options: {
  repositoryRoot: string; outputDirectory: string; targetTriple: string;
}): Promise<BuildIdentity> {
  validateBunVersion(Bun.version);
  if (options.targetTriple !== nativeTarget()) throw new Error('standalone builds must use the supported native target');
  const host = Bun.spawnSync(['rustc', '-vV']);
  if (host.exitCode !== 0 || !host.stdout.toString().includes(`host: ${options.targetTriple}\n`)) {
    throw new Error('Rust and Bun native targets disagree');
  }
  const root = resolve(options.repositoryRoot);
  const packageDirectory = join(root, 'packages/brain-frontend');
  for (const path of ['Cargo.toml', 'Cargo.lock', 'LICENSE', 'packages/brain-frontend/package.json',
    'packages/brain-frontend/bun.lock', 'packages/brain-frontend/src/cli/main.ts']) {
    if (!existsSync(join(root, path))) throw new Error(`missing mandatory owned build input: ${path}`);
    const ignored = Bun.spawnSync(['git', '-c', 'core.fsmonitor=false', 'check-ignore', '--no-index', '-q', path], { cwd: root });
    const tracked = Bun.spawnSync(['git', '-c', 'core.fsmonitor=false', 'ls-files', '--error-unmatch', path], { cwd: root });
    if (ignored.exitCode === 0 && tracked.exitCode !== 0) throw new Error(`ignored mandatory build input: ${path}`);
  }
  const before = collectSourceIdentity(root);
  const pkg = JSON.parse(readFileSync(join(packageDirectory, 'package.json'), 'utf8'));
  const identity: BuildIdentity = Object.freeze({ schema_version: 1, identity: 'brain-frontend', package: '@brain/frontend',
    frontend_version: pkg.version, brain_version: readBrainVersion(root), source: before,
    target_triple: options.targetTriple, runtime_model: 'bun-standalone', bun_version: '1.4.4',
    lockfile_sha256: digest(join(packageDirectory, 'bun.lock')) });
  const output = resolve(options.outputDirectory);
  validatePreviousOutput(output);
  mkdirSync(dirname(output), { recursive: true });
  const staging = mkdtempSync(join(dirname(output), '.brain-frontend-build-'));
  try {
    const built = Bun.spawn([process.execPath, ...standaloneArguments('src/cli/main.ts', join(staging, 'brain-frontend'),
      options.targetTriple, identity)], { cwd: packageDirectory, env: sanitizedBuildEnvironment(), stdout: 'pipe', stderr: 'pipe' });
    const [stdout, stderr, exit] = await Promise.all([new Response(built.stdout).text(), new Response(built.stderr).text(), built.exited]);
    if (exit !== 0) throw new Error(`standalone compilation failed (${exit}):\n${stdout}\n${stderr}`);
    if (!(statSync(join(staging, 'brain-frontend')).mode & 0o111)) throw new Error('compiled artifact is not executable');
    const probe = Bun.spawnSync([join(staging, 'brain-frontend'), '--build-identity'], { env: sanitizedBuildEnvironment() });
    if (probe.exitCode !== 0 || JSON.stringify(JSON.parse(probe.stdout.toString())) !== JSON.stringify(identity)) {
      throw new Error(`compiled identity verification failed: ${probe.stderr.toString()}`);
    }
    const manifest = { ...identity, entrypoint: 'brain-frontend', executable_sha256: digest(join(staging, 'brain-frontend')) };
    writeFileSync(join(staging, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
    assertSourceUnchanged(before, collectSourceIdentity(root));
    // Only replace the previously validated fixed pair; never delete an unrelated directory.
    if (existsSync(output)) {
      const old = `${staging}-previous`;
      renameSync(output, old);
      try { renameSync(staging, output); } catch (error) { renameSync(old, output); throw error; }
      rmSync(old, { recursive: true });
    } else { renameSync(staging, output); }
    return identity;
  } finally { if (existsSync(staging)) rmSync(staging, { recursive: true }); }
}

if (import.meta.main) {
  const args = Bun.argv.slice(2);
  const value = (name: string, fallback: string): string => {
    const index = args.indexOf(name);
    if (index === -1) return fallback;
    const result = args[index + 1];
    if (!result || result.startsWith('--')) throw new Error(`${name} requires a value`);
    return result;
  };
  await buildStandalone({ repositoryRoot: resolve(packageRoot, '../..'),
    outputDirectory: resolve(value('--output', join(packageRoot, 'dist/brain-frontend'))),
    targetTriple: value('--target', nativeTarget()) });
  console.log('Brain standalone frontend built and identity verified.');
}
