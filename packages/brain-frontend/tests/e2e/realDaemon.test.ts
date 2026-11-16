import { afterEach, describe, expect, test } from 'bun:test';

import { BrainFrontendBootstrap } from '../../src/app/bootstrap.js';
import { GenerationStreamGuard } from '../../src/brain/generation.js';
import { DaemonHarness, requireDaemonBinary } from './support/daemonHarness.js';

const daemons: DaemonHarness[] = [];

afterEach(async () => {
  await Promise.all(daemons.splice(0).map(daemon => daemon.close()));
});

describe('owned frontend real-daemon boundary', () => {
  test('handshakes, uses a daemon-issued session, and guards one deterministic stream in an isolated runtime', async () => {
    const daemon = await DaemonHarness.start(requireDaemonBinary());
    daemons.push(daemon);

    const frontend = new BrainFrontendBootstrap({
      socketPath: daemon.socketPath,
      frontendVersion: '0.1.0-test',
      frontendBuildId: 'owned-headless-e2e',
    });
    const capabilities = await frontend.start();
    expect(capabilities.backendName).toBe('brain-daemon');

    const created = await frontend.sessions.create({ title: 'Owned frontend E2E' });
    const initial = await frontend.sessions.load(created.sessionId);
    expect(initial.sessionId).toBe(created.sessionId);

    const generationId = crypto.randomUUID();
    const guard = new GenerationStreamGuard({ sessionId: created.sessionId, generationId });
    const terminal: string[] = [];
    for await (const frame of frontend.generations.stream({
      sessionId: created.sessionId,
      generationId,
      model: 'brain-default',
      messages: [{ role: 'user', content: 'owned frontend smoke' }],
    })) {
      const event = guard.accept(frame);
      if (event.terminal !== undefined) {
        terminal.push(event.terminal);
        break;
      }
    }

    expect(terminal).toEqual(['completed']);
    expect(guard.isTerminal()).toBe(true);
    const hydrated = await frontend.sessions.load(created.sessionId);
    expect(hydrated.messages.some(message => message.role === 'user')).toBe(true);
    expect(hydrated.messages.some(message => message.role === 'assistant')).toBe(true);
    await frontend.shutdown();
  });
});
