import { expect, test } from 'bun:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { stripVTControlCharacters } from 'node:util';
import { BrainFrontendBootstrap } from '../../src/app/bootstrap.js';
import { DaemonHarness, requireDaemonBinary } from './support/daemonHarness.js';

test('native terminal allows and denies Brain tools, restores focus and opens local context', async () => {
  const daemon = await DaemonHarness.start(requireDaemonBinary(), 20);
  const home = mkdtempSync('/tmp/bfp-');
  let output = '';
  const decoder = new TextDecoder();
  const frontend = Bun.spawn([process.execPath, resolve('src/cli/main.ts'), '--socket', daemon.socketPath], {
    env: { ...process.env, HOME: home, XDG_CONFIG_HOME: home, TERM: 'xterm-256color', CI: 'false', CONTINUOUS_INTEGRATION: 'false', FORCE_COLOR: '0' },
    terminal: { cols: 90, rows: 28, data: (_terminal, chunk) => { output += decoder.decode(chunk, { stream: true }); } },
  });
  const send = (text: string) => frontend.terminal!.write(text);
  const waitFor = async (text: string, since = 0) => {
    const deadline = Date.now() + 5_000;
    while (!stripVTControlCharacters(output.slice(since)).includes(text)) {
      if (frontend.exitCode !== null || Date.now() > deadline) throw new Error(`PTY did not show '${text}': ${stripVTControlCharacters(output).slice(-2500)}`);
      await Bun.sleep(10);
    }
  };
  try {
    await waitFor('New conversation'); send('\r'); await waitFor('Connected · New session');
    send('run [brain-tool:bash|{"command":"printf permission-proof"}] please');
    await waitFor('› run [brain-tool:bash'); send('\r');
    await waitFor('y allow');
    // Enter/no-leak precedence is asserted deterministically in state/input tests.
    // Send the decision as its own terminal key event, not a coalesced paste.
    const allowedAt = output.length; send('y');
    await waitFor('Tool succeeded: bash', allowedAt);
    await waitFor('Message Brain', allowedAt);
    const detailAt = output.length; send('\x14'); await waitFor('permission-proof', detailAt);
    const contextAt = output.length; send('/context'); await waitFor('› /context', contextAt); send('\r');
    await waitFor('No retrieved context for this session', contextAt);
    const closeAt = output.length; send('\x1b'); await waitFor('Message Brain', closeAt);

    const deniedAt = output.length;
    send('run [brain-tool:bash|{"command":"printf forbidden-output"}] please');
    await waitFor('› run [brain-tool:bash', deniedAt); send('\r');
    await waitFor('y allow', deniedAt);
    const decisionAt = output.length; send('n');
    await waitFor('Permission denied', decisionAt); await waitFor('Message Brain', decisionAt);
    const draftAt = output.length; send('draft after permission'); await waitFor('› draft after permission', draftAt);

    const bootstrap = new BrainFrontendBootstrap({ socketPath: daemon.socketPath, frontendVersion: '0.1.0-test' });
    await bootstrap.start();
    const sessions = await bootstrap.sessions.list();
    const history = await bootstrap.sessions.load(sessions[0]!.sessionId);
    expect(history.messages.filter(m => m.role === 'user')).toHaveLength(2);
    expect(history.messages.some(m => m.content === '/context')).toBe(false);
    const tools = history.messages.filter(m => m.role === 'tool').map(m => JSON.parse(m.content) as { outcome: string });
    expect(tools.map(t => t.outcome)).toEqual(['success', 'denied']);
    await bootstrap.shutdown();
    send('\x03');
    expect(await Promise.race([frontend.exited, Bun.sleep(5_000).then(() => { throw new Error('PTY failed to exit'); })])).toBe(0);
  } finally {
    if (frontend.exitCode === null) frontend.kill('SIGTERM');
    await Promise.race([frontend.exited, Bun.sleep(500)]);
    if (frontend.exitCode === null) frontend.kill('SIGKILL');
    await frontend.exited;
    frontend.terminal!.close();
    await daemon.close();
    rmSync(home, { recursive: true, force: true });
  }
}, 20_000);
