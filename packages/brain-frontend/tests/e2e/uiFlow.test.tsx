import { afterEach, describe, expect, test } from 'bun:test';
import { renderToString } from 'ink';

import { BrainFrontendBootstrap } from '../../src/app/bootstrap.js';
import { BrainTerminalScreen } from '../../src/presentation/terminal/app.js';
import { defaultBrainTheme } from '../../src/presentation/theme.js';
import { BrainFrontendController } from '../../src/state/interactions.js';
import { DaemonHarness, requireDaemonBinary } from './support/daemonHarness.js';

const daemons: DaemonHarness[] = [];

afterEach(async () => {
  await Promise.all(daemons.splice(0).map(daemon => daemon.close()));
});

describe('first-party terminal UI flow against a real daemon', () => {
  test('creates a session, renders Brain-backed stream state, and hydrates authoritative history', async () => {
    const daemon = await DaemonHarness.start(requireDaemonBinary());
    daemons.push(daemon);
    const bootstrap = new BrainFrontendBootstrap({
      socketPath: daemon.socketPath,
      frontendVersion: '0.1.0-test',
      frontendBuildId: 'owned-terminal-ui-e2e',
    });
    const controller = BrainFrontendController.fromBootstrap(bootstrap);

    expect(await controller.start()).toBe('ready');
    await controller.createSession();
    const sessionId = controller.snapshot().session?.sessionId;
    expect(sessionId).toBeDefined();
    controller.setDraft('terminal UI flow');
    expect(await controller.submit()).toBe('submitted');
    expect(controller.snapshot().generating).toBe(false);
    expect(controller.snapshot().transcript.some(entry => entry.kind === 'user' && entry.text === 'terminal UI flow')).toBe(true);
    expect(controller.snapshot().transcript.some(entry => entry.kind === 'assistant' && entry.status === 'complete')).toBe(true);

    await controller.selectSession(sessionId!);
    const output = renderToString(<BrainTerminalScreen model={controller.snapshot()} theme={defaultBrainTheme} />);
    expect(output).toContain('terminal UI flow');
    expect(output).toContain('Brain');

    controller.setDraft('preserved through restart');
    await daemon.stop();
    const recovery = controller.refreshConnection();
    await waitFor(() => controller.snapshot().connection === 'reconnecting');
    await daemon.restart();
    await recovery;
    expect(controller.snapshot().connection).toBe('ready');
    expect(controller.snapshot().session?.sessionId).toBe(sessionId);
    expect(controller.snapshot().composer.draft).toBe('preserved through restart');
    expect(controller.snapshot().session?.messages.filter(message => message.role === 'user')).toHaveLength(1);
    expect(bootstrap.recovery.snapshot().epoch).toBe(1);
    await bootstrap.shutdown();
  });

  test('cancels an active thinking stream and returns the composer to ready after Brain settlement', async () => {
    const daemon = await DaemonHarness.start(requireDaemonBinary(), 100);
    daemons.push(daemon);
    const bootstrap = new BrainFrontendBootstrap({ socketPath: daemon.socketPath, frontendVersion: '0.1.0-test' });
    const controller = BrainFrontendController.fromBootstrap(bootstrap);
    await controller.start();
    await controller.createSession();
    controller.setDraft('cancel thinking');
    const submission = controller.submit();
    await waitFor(() => controller.snapshot().transcript.some(entry => entry.kind === 'thinking' && entry.text.length > 0));
    expect(await controller.cancel()).toBe('requested');
    expect(await controller.cancel()).toBe('ignored');
    await submission;
    expect(controller.snapshot().generating).toBe(false);
    expect(controller.snapshot().transcript.filter(entry => entry.kind === 'assistant' && entry.status === 'cancelled')).toHaveLength(1);
    await bootstrap.shutdown();
  });
});

async function waitFor(predicate: () => boolean): Promise<void> {
  const deadline = Date.now() + 5_000;
  while (!predicate()) {
    if (Date.now() > deadline) throw new Error('Timed out waiting for frontend state');
    await Bun.sleep(10);
  }
}
