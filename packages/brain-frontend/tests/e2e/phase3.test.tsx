import { expect, test } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { renderToString } from 'ink';
import { BrainFrontendBootstrap } from '../../src/app/bootstrap.js';
import { BrainFrontendController } from '../../src/state/interactions.js';
import { BrainTerminalScreen } from '../../src/presentation/terminal/app.js';
import { defaultBrainTheme } from '../../src/presentation/theme.js';
import { DaemonHarness, requireDaemonBinary } from './support/daemonHarness.js';

async function until(predicate: () => boolean) {
  const deadline = Date.now() + 5_000;
  while (!predicate()) { if (Date.now() > deadline) throw new Error('Brain interaction did not reach expected state'); await Bun.sleep(10); }
}
async function setup() {
  const daemon = await DaemonHarness.start(requireDaemonBinary());
  const bootstrap = new BrainFrontendBootstrap({ socketPath: daemon.socketPath, frontendVersion: '0.1.0-test', frontendBuildId: 'owned-phase3-e2e' });
  const controller = BrainFrontendController.fromBootstrap(bootstrap);
  await controller.start(); await controller.createSession();
  return { daemon, bootstrap, controller };
}

test('real allow/deny round trips execute only in Brain and hydrate authoritative tool outcomes', async () => {
  const { daemon, bootstrap, controller } = await setup();
  try {
    const marker = join(dirname(daemon.socketPath), 'executions');
    const prompt = `run [brain-tool:bash|${JSON.stringify({ command: `printf executed >> '${marker}'; printf 'safe output'` })}] please`;
    controller.setDraft(prompt);
    const turn = controller.submit();
    await until(() => controller.snapshot().focus === 'permission');
    expect(existsSync(marker)).toBe(false);
    await Promise.all([controller.resolvePermission('allow'), controller.resolvePermission('allow')]);
    await turn;
    expect(readFileSync(marker, 'utf8')).toBe('executed');
    expect(controller.snapshot().transcript.filter(e => e.kind === 'tool')).toHaveLength(1);
    expect(controller.snapshot().transcript.find(e => e.kind === 'tool')).toMatchObject({ status: 'succeeded', detail: 'safe output' });
    expect(controller.snapshot().generating).toBe(false);
    controller.toggleToolDetail();
    const output = renderToString(<BrainTerminalScreen model={controller.snapshot()} theme={defaultBrainTheme} />);
    expect(output).toContain('safe output');
    expect(output).not.toContain('call_id');
    await controller.selectSession(controller.snapshot().session!.sessionId);
    expect(controller.snapshot().transcript.find(e => e.kind === 'tool')).toMatchObject({ status: 'succeeded', detail: 'safe output' });

    controller.setDraft(prompt);
    const denied = controller.submit();
    await until(() => controller.snapshot().focus === 'permission');
    await Promise.all([controller.resolvePermission('deny'), controller.resolvePermission('allow')]);
    await denied;
    expect(readFileSync(marker, 'utf8')).toBe('executed');
    expect(controller.snapshot().transcript.filter(e => e.kind === 'tool').at(-1)).toMatchObject({ status: 'failed', summary: 'Permission denied' });
    expect(controller.snapshot().focus).toBe('composer');
  } finally { await bootstrap.shutdown(); await daemon.close(); }
}, 15_000);

test('real tool failure and permission cancellation settle without a late success', async () => {
  const { daemon, bootstrap, controller } = await setup();
  try {
    controller.setDraft('run [brain-tool:bash|{"command":"exit 3"}] please');
    const failed = controller.submit();
    await until(() => controller.snapshot().focus === 'permission');
    await controller.resolvePermission('allow'); await failed;
    expect(controller.snapshot().transcript.find(e => e.kind === 'tool')).toMatchObject({ status: 'failed', detail: undefined });
    controller.setDraft('run [brain-tool:bash|{"command":"printf should-not-run"}] please');
    const cancelled = controller.submit();
    await until(() => controller.snapshot().focus === 'permission');
    expect(await controller.cancel()).toBe('requested');
    expect(await controller.cancel()).toBe('ignored');
    await cancelled;
    expect(controller.snapshot().permission).toBeUndefined();
    expect(controller.snapshot().transcript.filter(e => e.kind === 'tool').at(-1)).toMatchObject({ status: 'cancelled' });
    expect(controller.snapshot().transcript.filter(e => e.kind === 'assistant' && e.status === 'cancelled')).toHaveLength(1);
  } finally { await bootstrap.shutdown(); await daemon.close(); }
}, 15_000);

test('daemon loss while permission is pending reloads truth without tool/decision/prompt replay', async () => {
  const { daemon, bootstrap, controller } = await setup();
  try {
    const marker = join(dirname(daemon.socketPath), 'never-run');
    controller.setDraft(`run [brain-tool:bash|${JSON.stringify({ command: `touch '${marker}'` })}] please`);
    const turn = controller.submit();
    await until(() => controller.snapshot().focus === 'permission');
    await daemon.stop();
    await until(() => controller.snapshot().connection === 'reconnecting');
    expect(controller.snapshot().permission).toBeUndefined();
    await controller.resolvePermission('allow');
    await daemon.restart(); await turn;
    expect(controller.snapshot().connection).toBe('ready');
    expect(controller.snapshot().generating).toBe(false);
    expect(controller.snapshot().focus).toBe('composer');
    expect(controller.snapshot().contextSummary).toBeUndefined();
    expect(existsSync(marker)).toBe(false);
    const history = await bootstrap.sessions.load(controller.snapshot().session!.sessionId);
    expect(history.messages.filter(m => m.role === 'user')).toHaveLength(1);
    expect(history.messages.filter(m => m.role === 'tool')).toHaveLength(0);
    expect(bootstrap.recovery.snapshot().epoch).toBe(1);
  } finally { await bootstrap.shutdown(); await daemon.close(); }
}, 15_000);

test('real retrieval publishes only a quiet approved context summary, neutral after reload', async () => {
  const { daemon, bootstrap, controller } = await setup();
  try {
    await bootstrap.transport.request('v1/memory/store', { label: 'quartzarchive', content: 'quartzarchive project architecture uses Brain sessions and typed UDS', scope: 'workspace' });
    controller.setDraft('quartzarchive project architecture'); await controller.submit();
    expect(controller.snapshot().contextSummary?.retrievalCount).toBeGreaterThan(0);
    controller.setDraft('/context'); await controller.submit();
    const output = renderToString(<BrainTerminalScreen model={controller.snapshot()} theme={defaultBrainTheme} />);
    expect(output).toContain('retrieved source');
    expect(output).toContain('context tokens');
    expect(output).not.toContain('source_refs');
    expect(output).not.toContain('memory_provenance');
    const id = controller.snapshot().session!.sessionId;
    expect((await bootstrap.sessions.load(id)).messages.some(m => m.role === 'user' && m.content === '/context')).toBe(false);
    controller.closeContext(); await controller.selectSession(id);
    expect(controller.snapshot().contextSummary).toBeUndefined();
  } finally { await bootstrap.shutdown(); await daemon.close(); }
}, 15_000);
