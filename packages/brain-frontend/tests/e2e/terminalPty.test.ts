import { expect, test } from 'bun:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { BrainFrontendBootstrap } from '../../src/app/bootstrap.js';
import { DaemonHarness, requireDaemonBinary } from './support/daemonHarness.js';
import { GenerationTrace } from './support/generationTrace.js';

test('real terminal accepts a draft, submits once, cancels without quitting, and restores picker focus', async () => {
  const daemon = await DaemonHarness.start(requireDaemonBinary(), 200);
  const home = mkdtempSync('/tmp/bft-');
  const trace = await GenerationTrace.start(join(home, 'trace.sock'), daemon.socketPath);
  const command = [process.execPath, resolve('src/cli/main.ts'), '--socket', trace.socketPath];
  let output = '';
  const decoder = new TextDecoder();
  const terminal = Bun.spawn(command, {
    env: { ...process.env, HOME: home, XDG_CONFIG_HOME: home, TERM: 'xterm-256color', CI: 'false', CONTINUOUS_INTEGRATION: 'false', FORCE_COLOR: '0' },
    terminal: { cols: 90, rows: 28, data: (_terminal, chunk) => { output += decoder.decode(chunk, { stream: true }); } },
  });
  const waitForOutput = async (text: string, since = 0): Promise<void> => {
    const deadline = Date.now() + 5_000;
    while (!plain(output.slice(since)).includes(text)) {
      if (terminal.exitCode !== null || Date.now() > deadline) {
        throw new Error(`Terminal did not show '${text}'; exit=${terminal.exitCode}; output=${plain(output).slice(-3000)}`);
      }
      await Bun.sleep(10);
    }
  };
  const send = (input: string) => { terminal.terminal!.write(input); };
  try {
    await waitForOutput('New conversation');
    send('\r');
    await waitForOutput('Connected · New session');
    send('pty turn');
    await waitForOutput('› pty turn');
    send('\r');
    await waitForOutput('Mock response to: pty turn');
    const completedAt = output.length;
    await waitForOutput('Message Brain', completedAt);

    send('cancel this');
    await waitForOutput('› cancel this');
    const cancellationAt = output.length;
    // Exercise the real early-cancel race without sleeps or changing mock timing:
    // UI intent exists while the generation request has not reached Brain yet.
    trace.holdNextGeneration();
    trace.mark('submit-key');
    send('\r');
    await waitForOutput('Generating', cancellationAt);
    trace.mark('cancel-key');
    send('\x03');
    await waitForOutput('Cancelling', cancellationAt);
    expect(trace.hasHeldGeneration()).toBe(true);
    expect(trace.observations.filter(event => event.direction === 'request' && event.action === 'v1/generation/cancel')).toHaveLength(0);
    trace.releaseGeneration();
    await waitForOutput('Generation cancelled');
    trace.mark('ui:cancelled');
    const cancelledRequest = trace.observations.find(event => event.direction === 'request' && event.action === 'v1/generation/cancel')!;
    expect(cancelledRequest).toBeDefined();
    const generationEvents = trace.observations.filter(event => event.generationId === cancelledRequest.generationId);
    const acceptedIndex = generationEvents.findIndex(event => event.type === 'stream_start');
    const cancelIndex = generationEvents.findIndex(event => event.direction === 'request' && event.action === 'v1/generation/cancel');
    expect(acceptedIndex).toBeGreaterThan(-1);
    expect(cancelIndex).toBeGreaterThan(acceptedIndex);
    expect(generationEvents.filter(event => event.type === 'finished')).toEqual([
      expect.objectContaining({ status: 'cancelled', sessionId: cancelledRequest.sessionId }),
    ]);
    expect(generationEvents.some(event => event.status === 'completed')).toBe(false);
    expect(terminal.exitCode).toBeNull();

    const pickerAt = output.length;
    send('\x0f');
    await waitForOutput('Sessions', pickerAt);
    const dismissedAt = output.length;
    send('\x1b');
    await waitForOutput('Message Brain', dismissedAt);
    const draftAt = output.length;
    send('draft after picker');
    await waitForOutput('draft after picker', draftAt);
    const resizedAt = output.length;
    terminal.terminal!.resize(55, 18);
    await waitForOutput('draft after picker', resizedAt);
    send('\n');
    send('second draft line');
    await waitForOutput('second draft line');

    const bootstrap = new BrainFrontendBootstrap({ socketPath: daemon.socketPath, frontendVersion: '0.1.0-test' });
    await bootstrap.start();
    const sessions = await bootstrap.sessions.list();
    expect(sessions).toHaveLength(1);
    const history = await bootstrap.sessions.load(sessions[0]!.sessionId);
    expect(history.messages.filter(message => message.role === 'user' && message.content === 'pty turn')).toHaveLength(1);
    expect(history.messages.filter(message => message.role === 'user' && message.content === 'cancel this')).toHaveLength(1);
    expect(history.messages.some(message => message.content === 'draft after picker')).toBe(false);
    await bootstrap.shutdown();
    send('\x03');
    const exitCode = await Promise.race([terminal.exited, Bun.sleep(5_000).then(() => { throw new Error('Terminal did not exit'); })]);
    expect(exitCode).toBe(0);
    expect(plain(output)).not.toContain('error:');
  } finally {
    if (terminal.exitCode === null) terminal.kill('SIGTERM');
    await Promise.race([terminal.exited, Bun.sleep(500)]);
    if (terminal.exitCode === null) terminal.kill('SIGKILL');
    await terminal.exited;
    terminal.terminal!.close();
    await trace.close();
    await daemon.close();
    rmSync(home, { recursive: true, force: true });
  }
}, 20_000);

function plain(text: string): string {
  return text.replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, '').replace(/\r/g, '');
}
