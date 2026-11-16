import { expect, test } from 'bun:test';
import { accessSync, constants, existsSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { stripVTControlCharacters } from 'node:util';

import { BrainFrontendBootstrap } from '../../src/app/bootstrap.js';
import { DaemonHarness, requireDaemonBinary } from './support/daemonHarness.js';

function requireCliBinary(): string {
  const binary = resolve(process.env.BRAIN_CLI_BIN ?? resolve(import.meta.dir, '../../../../target/debug/brain'));
  try {
    accessSync(binary, constants.X_OK);
    if (!statSync(binary).isFile()) throw new Error('CLI must be a file');
  }
  catch { throw new Error('Build the Brain CLI with cargo build -p brain --locked, or supply an executable BRAIN_CLI_BIN for launcher E2E'); }
  return binary;
}

test('actual brain ui defaults to the owned frontend from an unrelated cwd and exits cleanly', async () => {
  const binary = requireCliBinary();
  let daemon: DaemonHarness | undefined;
  let home: string | undefined;
  let frontend: ReturnType<typeof Bun.spawn> | undefined;
  let descendants: number[] = [];
  try {
    home = mkdtempSync('/tmp/bfl-');
    daemon = await DaemonHarness.start(requireDaemonBinary());
    let output = '';
    const decoder = new TextDecoder();
    const environment: NodeJS.ProcessEnv = { ...process.env, HOME: home, XDG_CONFIG_HOME: home, BRAIN_CONFIG_DIR: home,
      BRAIN_SOCKET_PATH: daemon.socketPath, BRAIN_FRONTEND_RUNTIME: process.execPath,
      TERM: 'xterm-256color', CI: 'false', CONTINUOUS_INTEGRATION: 'false', FORCE_COLOR: '0' };
    delete environment.BRAIN_FRONTEND_ROOT;
    frontend = Bun.spawn([binary, 'ui'], {
      cwd: home, env: environment,
      terminal: { cols: 90, rows: 28, data: (_terminal, chunk) => { output += decoder.decode(chunk, { stream: true }); } },
    });
    const activeFrontend = frontend;
    const send = (text: string) => activeFrontend.terminal!.write(text);
    const waitFor = async (text: string, since = 0): Promise<void> => {
      const deadline = Date.now() + 5_000;
      while (!stripVTControlCharacters(output.slice(since)).includes(text)) {
        if (activeFrontend.exitCode !== null || Date.now() > deadline) {
          throw new Error(`brain ui did not show '${text}'; exit=${activeFrontend.exitCode}; output=${stripVTControlCharacters(output).slice(-2500)}`);
        }
        await Bun.sleep(10);
      }
    };
    // Session-picker readiness requires mandatory handshake through the real daemon.
    await waitFor('New conversation');
    descendants = await childrenOf(frontend.pid);
    send('\r'); await waitFor('Connected · New session');
    send('owned launcher smoke'); await waitFor('› owned launcher smoke');
    const submittedAt = output.length; send('\r');
    await waitFor('Mock response to: owned launcher smoke', submittedAt);
    await waitFor('Message Brain', submittedAt);

    const bootstrap = new BrainFrontendBootstrap({ socketPath: daemon.socketPath, frontendVersion: '0.1.0-test' });
    try {
      await bootstrap.start();
      const sessions = await bootstrap.sessions.list();
      expect(sessions).toHaveLength(1);
      const history = await bootstrap.sessions.load(sessions[0]!.sessionId);
      expect(history.messages.filter(message => message.role === 'user' && message.content === 'owned launcher smoke')).toHaveLength(1);
    } finally { await bootstrap.shutdown(); }

    send('\x03');
    expect(await boundedExit(frontend)).toBe(0);
    await waitUntilGone(descendants);
    expect(stripVTControlCharacters(output)).not.toContain('error:');
  } finally {
    try { if (frontend !== undefined) await cleanup(frontend, descendants); }
    finally {
      try { await daemon?.close(); }
      finally { if (home !== undefined) rmSync(home, { recursive: true, force: true }); }
    }
  }
}, 20_000);

test('brain ui propagates external termination without leaving frontend children', async () => {
  const binary = requireCliBinary();
  let daemon: DaemonHarness | undefined;
  let home: string | undefined;
  let frontend: ReturnType<typeof Bun.spawn> | undefined;
  let descendants: number[] = [];
  try {
    home = mkdtempSync('/tmp/bfs-');
    daemon = await DaemonHarness.start(requireDaemonBinary());
    let output = '';
    const environment: NodeJS.ProcessEnv = { ...process.env, HOME: home, XDG_CONFIG_HOME: home, BRAIN_CONFIG_DIR: home,
      BRAIN_SOCKET_PATH: daemon.socketPath, BRAIN_FRONTEND_RUNTIME: process.execPath,
      TERM: 'xterm-256color', CI: 'false', CONTINUOUS_INTEGRATION: 'false', FORCE_COLOR: '0' };
    delete environment.BRAIN_FRONTEND_ROOT;
    frontend = Bun.spawn([binary, 'ui'], {
      cwd: home, env: environment,
      terminal: { cols: 80, rows: 24, data: (_terminal, chunk) => { output += new TextDecoder().decode(chunk); } },
    });
    const deadline = Date.now() + 5_000;
    while (!stripVTControlCharacters(output).includes('New conversation')) {
      if (frontend.exitCode !== null || Date.now() > deadline) throw new Error(`brain ui readiness failed: ${output.slice(-2000)}`);
      await Bun.sleep(10);
    }
    descendants = await childrenOf(frontend.pid);
    frontend.kill('SIGTERM');
    expect(await boundedExit(frontend)).not.toBe(0);
    await waitUntilGone(descendants);
  } finally {
    try { if (frontend !== undefined) await cleanup(frontend, descendants); }
    finally {
      try { await daemon?.close(); }
      finally { if (home !== undefined) rmSync(home, { recursive: true, force: true }); }
    }
  }
}, 15_000);

async function childrenOf(parent: number): Promise<number[]> {
  const processList = Bun.spawn(['ps', '-axo', 'pid=,ppid='], { stdout: 'pipe' });
  const lines = (await new Response(processList.stdout).text()).trim().split('\n');
  if (await processList.exited !== 0) throw new Error('Cannot inspect frontend process cleanup with ps');
  const rows = lines.map(line => line.trim().split(/\s+/).map(Number));
  const descendants: number[] = [];
  const pending = [parent];
  while (pending.length > 0) {
    const next = pending.pop()!;
    for (const [pid, ppid] of rows) {
      if (ppid === next && pid !== undefined) { descendants.push(pid); pending.push(pid); }
    }
  }
  return descendants;
}

function alive(pid: number): boolean {
  try { process.kill(pid, 0); return true; }
  catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ESRCH') return false;
    throw error;
  }
}

async function waitUntilGone(pids: readonly number[]): Promise<void> {
  const deadline = Date.now() + 3_000;
  while (pids.some(alive)) {
    if (Date.now() > deadline) throw new Error(`Frontend children survived shutdown: ${pids.filter(alive).join(', ')}`);
    await Bun.sleep(10);
  }
}

async function boundedExit(frontend: ReturnType<typeof Bun.spawn>): Promise<number> {
  return Promise.race([frontend.exited, Bun.sleep(5_000).then(() => { throw new Error('brain ui did not exit'); })]);
}

async function cleanup(frontend: ReturnType<typeof Bun.spawn>, descendants: readonly number[]): Promise<void> {
  if (frontend.exitCode === null) frontend.kill('SIGTERM');
  await Promise.race([frontend.exited, Bun.sleep(500)]);
  if (frontend.exitCode === null) frontend.kill('SIGKILL');
  await frontend.exited;
  for (const pid of descendants.filter(alive)) process.kill(pid, 'SIGKILL');
  frontend.terminal!.close();
}

test('invalid CLI executable cannot leak a daemon when PTY startup fails', async () => {
  const fixture = mkdtempSync('/tmp/bfx-');
  const wrapper = resolve(fixture, 'daemon');
  const invalidExecutable = resolve(fixture, 'invalid-cli');
  const record = resolve(fixture, 'daemon.pid');
  writeFileSync(wrapper, '#!/bin/sh\nprintf "%s\\n%s\\n" "$$" "$BRAIN_CONFIG_DIR" > "$BRAIN_LAUNCH_FAILURE_PID"\nexec "$BRAIN_LAUNCH_REAL_DAEMON" "$@"\n', { mode: 0o755 });
  writeFileSync(invalidExecutable, '#!/nonexistent/brain-test-interpreter\n', { mode: 0o755 });
  let pid: number | undefined;
  let runtimeRoot: string | undefined;
  try {
    for (const invalidCli of ['/tmp', invalidExecutable]) {
      rmSync(record, { force: true });
      pid = undefined; runtimeRoot = undefined;
      const child = Bun.spawn([process.execPath, 'test', import.meta.filename, '--test-name-pattern', '^actual brain ui defaults'], {
        env: { ...process.env, BRAIN_CLI_BIN: invalidCli, BRAIN_DAEMON_BIN: wrapper,
          BRAIN_LAUNCH_FAILURE_PID: record, BRAIN_LAUNCH_REAL_DAEMON: resolve(requireDaemonBinary()) },
        stdout: 'pipe', stderr: 'pipe',
      });
      const [stdout, stderr, exit] = await Promise.all([
        new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited,
      ]);
      if (existsSync(record)) {
        const recorded = readFileSync(record, 'utf8').trim().split('\n');
        pid = Number(recorded[0]); runtimeRoot = recorded[1];
      }
      expect(exit).not.toBe(0);
      expect(stdout + stderr).toMatch(/executable|EACCES|ENOENT|No such file|not found/i);
      expect(pid === undefined || !alive(pid)).toBe(true);
      expect(runtimeRoot === undefined || !existsSync(runtimeRoot)).toBe(true);
      if (invalidCli === invalidExecutable) expect(pid).toBeDefined();
    }
  } finally {
    if (pid !== undefined && alive(pid)) {
      process.kill(pid, 'SIGKILL');
      await waitUntilGone([pid]);
    }
    if (runtimeRoot?.startsWith('/tmp/bfe-')) rmSync(runtimeRoot, { recursive: true, force: true });
    rmSync(fixture, { recursive: true, force: true });
  }
}, 15_000);
