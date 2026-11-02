import { afterAll, beforeAll, expect, test } from 'bun:test';
import { cpSync, existsSync, mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createServer, type Server, type Socket } from 'node:net';
import { buildStandalone, nativeTarget, validateBunVersion, assertSourceUnchanged, sanitizedBuildEnvironment } from '../../scripts/build.js';
import { collectSourceIdentity } from '../../scripts/release/sourceIdentity.js';
import { resolveFrontendConfiguration } from '../../src/cli/configuration.js';
import type { BuildIdentity } from '../../src/cli/buildIdentity.js';

const root = resolve(import.meta.dir, '../../../..');
const temporary = mkdtempSync(join(tmpdir(), 'brain-standalone-'));
const artifact = join(temporary, 'artifact');
let identity: BuildIdentity;
beforeAll(async () => {
  // Also expose an ambient neighboring module tree: an installed optional peer
  // must not turn the compiled release into a debugger merely via DEV=true.
  symlinkSync(join(root, 'packages/brain-frontend/node_modules'), join(temporary, 'node_modules'));
  identity = await buildStandalone({ repositoryRoot: root, outputDirectory: artifact, targetTriple: nativeTarget() });
}, 60_000);
afterAll(() => rmSync(temporary, { recursive: true, force: true }));

test('real compiled identity requires no socket/UI and cannot be spoofed by environment', () => {
  const env: NodeJS.ProcessEnv = { ...process.env, DEV: 'true', BRAIN_FRONTEND_VERSION: 'spoof', BRAIN_FRONTEND_BUILD_ID: 'spoof' };
  delete env.BRAIN_SOCKET_PATH;
  const run = Bun.spawnSync([join(artifact, 'brain-frontend'), '--build-identity'], { cwd: temporary, env });
  expect(run.exitCode).toBe(0);
  expect(run.stderr.toString()).toBe('');
  expect(JSON.parse(run.stdout.toString())).toEqual(identity);
  expect(identity.identity).toBe('brain-frontend');
  expect(identity.package).toBe('@brain/frontend');
  expect(identity.bun_version).toBe('1.4.4');
  expect(identity.runtime_model).toBe('bun-standalone');
  expect(identity.source.sha).toMatch(/^[a-f0-9]{40}$/);
  expect(readdirSync(artifact).sort()).toEqual(['brain-frontend', 'manifest.json']);
});

test('standalone ignores hostile dotenv, bunfig preload, package and tsconfig in CWD/HOME', () => {
  const cwd = join(temporary, 'hostile'); mkdirSync(cwd);
  const sentinel = join(temporary, 'preloaded');
  writeFileSync(join(cwd, 'preload.ts'), `await Bun.write(${JSON.stringify(sentinel)}, 'unsafe');`);
  writeFileSync(join(cwd, 'bunfig.toml'), 'preload = ["./preload.ts"]\n');
  writeFileSync(join(cwd, '.env'), 'BRAIN_SOCKET_PATH=/tmp/should-not-connect.sock\n');
  writeFileSync(join(cwd, 'package.json'), '{"scripts":{"start":"bun preload.ts"}}');
  writeFileSync(join(cwd, 'tsconfig.json'), '{"compilerOptions":{"jsxImportSource":"hostile"}}');
  const env: Record<string, string | undefined> = { ...process.env, HOME: cwd };
  for (const key of ['BRAIN_SOCKET_PATH', 'BUN_BE_BUN', 'BUN_OPTIONS', 'NODE_OPTIONS']) delete env[key];
  const run = Bun.spawnSync([join(artifact, 'brain-frontend')], { cwd, env });
  expect(run.exitCode).toBe(1);
  expect(run.stderr.toString()).toContain('provide --socket');
  expect(existsSync(sentinel)).toBe(false);
});

test('compiled configuration pins version/build identity but keeps explicit Brain socket', () => {
  expect(resolveFrontendConfiguration(['--frontend-version', 'spoof', '--frontend-build-id', 'spoof'],
    { BRAIN_SOCKET_PATH: '/tmp/owned.sock' }, identity)).toEqual({ socketPath: '/tmp/owned.sock',
      frontendVersion: identity.frontend_version, frontendBuildId: identity.source.fingerprint_sha256 });
});

test('wrong toolchain, missing inputs and changed source fail instead of publishing', async () => {
  expect(() => validateBunVersion('1.4.1')).toThrow('1.4.4');
  expect(() => validateBunVersion('1.4.2')).toThrow('1.4.4');
  expect(() => validateBunVersion('1.4.4')).not.toThrow();
  const source = collectSourceIdentity(root);
  expect(() => assertSourceUnchanged(source, { ...source, fingerprint_sha256: '0'.repeat(64) })).toThrow('changed');
  await expect(buildStandalone({ repositoryRoot: temporary, outputDirectory: join(temporary, 'bad'),
    targetTriple: nativeTarget() })).rejects.toThrow();
  expect(existsSync(join(temporary, 'bad'))).toBe(false);
  const foreign = join(temporary, 'foreign'); mkdirSync(foreign); writeFileSync(join(foreign, 'keep'), 'keep');
  await expect(buildStandalone({ repositoryRoot: root, outputDirectory: foreign,
    targetTriple: nativeTarget() })).rejects.toThrow('nonempty');
  expect(readFileSync(join(foreign, 'keep'), 'utf8')).toBe('keep');
});

test('sanitized build environment strips Bun and Node option variables', () => {
  const previous = {
    BUN_BE_BUN: process.env.BUN_BE_BUN,
    BUN_OPTIONS: process.env.BUN_OPTIONS,
    NODE_OPTIONS: process.env.NODE_OPTIONS,
  };
  try {
    process.env.BUN_BE_BUN = '1';
    process.env.BUN_OPTIONS = '--preload ./hostile.ts';
    process.env.NODE_OPTIONS = '--require ./hostile.js';
    const sanitized = sanitizedBuildEnvironment();
    expect(sanitized.BUN_BE_BUN).toBeUndefined();
    expect(sanitized.BUN_OPTIONS).toBeUndefined();
    expect(sanitized.NODE_OPTIONS).toBeUndefined();
  } finally {
    for (const [key, value] of Object.entries(previous)) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
  }
});

test('a real source edit during compilation prevents artifact publication', async () => {
  const fixture = join(temporary, 'source-mutation');
  const fixturePackage = join(fixture, 'packages/brain-frontend');
  mkdirSync(fixturePackage, { recursive: true });
  // Only first-party owned sources and declared locked toolchain inputs.
  for (const name of ['package.json', 'bun.lock', 'tsconfig.json', 'src', 'scripts']) {
    cpSync(join(root, 'packages/brain-frontend', name), join(fixturePackage, name), { recursive: true });
  }
  symlinkSync(join(root, 'packages/brain-frontend/node_modules'), join(fixturePackage, 'node_modules'));
  writeFileSync(join(fixture, 'Cargo.toml'), '[workspace]\n[workspace.package]\nversion="1.1.0"\n');
  writeFileSync(join(fixture, 'Cargo.lock'), '# independently authored fingerprint fixture\n');
  writeFileSync(join(fixture, 'LICENSE'), 'fixture input\n');
  writeFileSync(join(fixture, '.gitignore'), 'node_modules\ndist/\n');
  mkdirSync(join(fixture, 'apps')); writeFileSync(join(fixture, 'apps/input'), 'before');
  for (const args of [['init', '-q'], ['add', '.'], ['-c', 'user.name=Fixture', '-c',
    'user.email=fixture@example.invalid', 'commit', '-qm', 'fixture']]) {
    const command = Bun.spawnSync(['git', '-c', 'core.fsmonitor=false', ...args], { cwd: fixture });
    if (command.exitCode !== 0) throw new Error(command.stderr.toString());
  }
  const destination = join(fixture, 'output');
  // buildStandalone's first await occurs after identity capture and compiler
  // spawn: mutate only this disposable repository, without timing sleeps/hooks.
  const pending = buildStandalone({ repositoryRoot: fixture, outputDirectory: destination, targetTriple: nativeTarget() });
  writeFileSync(join(fixture, 'apps/input'), 'after');
  await expect(pending).rejects.toThrow('source inputs changed during build');
  expect(existsSync(destination)).toBe(false);
}, 60_000);

// A shipped build must never opt into optional DevTools from ambient DEV.
// This exercises the real compiled entrypoint and real OS sockets, not a mocked import.
for (const dev of [undefined, 'false', 'true'] as const) {
  test(`compiled startup does not activate DevTools with DEV=${dev ?? 'unset'}`, async () => {
    const runtime = mkdtempSync(join(tmpdir(), 'bfd-'));
    const socketPath = join(runtime, 'brain.sock');
    const sockets = new Set<Socket>();
    const servers: Server[] = [];
    const actions: string[] = [];
    let devtoolsConnections = 0;
    let output = '';
    let terminal: ReturnType<typeof Bun.spawn> | undefined;
    const track = (socket: Socket) => {
      sockets.add(socket);
      socket.on('close', () => sockets.delete(socket));
    };
    const listen = async (server: Server, options: Parameters<Server['listen']>[0]) => {
      servers.push(server);
      await new Promise<void>((accept, reject) => {
        server.once('error', reject);
        server.listen(options, () => { server.off('error', reject); accept(); });
      });
    };
    try {
      // Listen only on loopback. A connection is counted and closed, never upgraded
      // into an actual debugger service. Cover both localhost address families.
      for (const host of ['127.0.0.1', '::1']) {
        await listen(createServer(socket => {
          track(socket); devtoolsConnections++; socket.destroy();
        }), { port: 8097, host, ipv6Only: host === '::1' });
      }
      await listen(createServer(socket => {
        track(socket);
        let buffer = '';
        socket.on('data', bytes => {
          buffer += bytes.toString();
          if (!buffer.includes('\n')) return;
          const request = JSON.parse(buffer.slice(0, buffer.indexOf('\n')));
          actions.push(request.action);
          const body = request.action === 'v1/frontend/handshake' ? {
            selectedProtocolVersion: 1, backendName: 'brain-daemon', backendVersion: '1.1.0', daemonState: 'ready',
            supportedCapabilities: ['sessions.v1', 'generation.stream.v1', 'generation.cancel.v1'],
            acceptedOptionalCapabilities: [],
          } : { sessions: [] };
          socket.end(`${JSON.stringify({ version: '1.0', type: 'Response', id: request.id,
            status: 'success', body: JSON.stringify(body) })}\n`);
        });
      }), { path: socketPath });
      const env: NodeJS.ProcessEnv = { ...process.env, HOME: runtime, XDG_CONFIG_HOME: runtime,
        BRAIN_SOCKET_PATH: socketPath, TERM: 'xterm-256color', FORCE_COLOR: '0', CI: 'false',
        CONTINUOUS_INTEGRATION: 'false' };
      for (const key of ['DEV', 'BUN_BE_BUN', 'BUN_OPTIONS', 'NODE_OPTIONS']) delete env[key];
      if (dev !== undefined) env.DEV = dev;
      const decoder = new TextDecoder();
      terminal = Bun.spawn([join(artifact, 'brain-frontend')], {
        cwd: runtime, env,
        terminal: { cols: 90, rows: 28, data: (_terminal, chunk) => {
          output += decoder.decode(chunk, { stream: true });
        } },
      });
      const deadline = Date.now() + 8_000;
      while (!output.includes('New conversation')) {
        if (terminal.exitCode !== null || Date.now() >= deadline) {
          throw new Error(`compiled startup did not reach picker; exit=${terminal.exitCode}; output=${output.slice(-2000)}`);
        }
        await Bun.sleep(10);
      }
      const network = Bun.spawnSync(['lsof', '-nP', '-a', '-p', String(terminal.pid), '-i']);
      if (network.exitCode !== 0 && network.exitCode !== 1) throw new Error(`socket inspection failed: ${network.stderr.toString()}`);
      const extraOutput = /DevTools|react-devtools|8097/i.test(output);
      console.log(`[packaged DevTools probe] DEV=${dev ?? 'unset'} connections=${devtoolsConnections} ` +
        `networkSockets=${network.stdout.length === 0 ? 0 : 'present'} ready=true devtoolsOutput=${extraOutput}`);
      expect(actions).toEqual(['v1/frontend/handshake', 'v1/session/list']);
      expect(devtoolsConnections).toBe(0);
      expect(network.stdout.toString()).toBe('');
      expect(extraOutput).toBe(false);
      // Startup owns the session-picker overlay: dismiss it before the idle
      // quit key. A single Ctrl+C in that overlay intentionally only dismisses.
      const dismissedAt = output.length;
      terminal.terminal!.write('\x1b');
      const dismissalDeadline = Date.now() + 3_000;
      while (!output.slice(dismissedAt).includes('Message Brain')) {
        if (terminal.exitCode !== null || Date.now() >= dismissalDeadline) {
          throw new Error(`compiled picker did not restore composer: ${output.slice(-2000)}`);
        }
        await Bun.sleep(10);
      }
      await Bun.sleep(50);
      terminal.terminal!.write('\x03');
      await Promise.race([terminal.exited, Bun.sleep(3_000).then(() => { throw new Error('compiled frontend did not exit'); })]);
      expect(terminal.exitCode).toBe(0);
    } finally {
      if (terminal !== undefined) {
        if (terminal.exitCode === null) terminal.kill('SIGTERM');
        await Promise.race([terminal.exited, Bun.sleep(1_000)]);
        if (terminal.exitCode === null) terminal.kill('SIGKILL');
        await terminal.exited;
        terminal.terminal?.close();
      }
      for (const socket of sockets) socket.destroy();
      await Promise.all(servers.map(server => new Promise<void>(accept => server.close(() => accept()))));
      rmSync(runtime, { recursive: true, force: true });
    }
  }, 15_000);
}
