import { expect, test } from 'bun:test';
import { BrainFrontendController, type BrainFrontendRuntime } from '../../src/state/interactions.js';
import { PermissionController } from '../../src/brain/permissions.js';
import { BrainContextState } from '../../src/brain/context.js';

const sessionId = '01ARZ3NDEKTSV4RRFFQ69G5FAV';
const context = { version: 1, retrieval_epoch_id: 'epoch', retrieval_count: 1, retrieval_channels: ['semantic'], context_token_count: 24, source_refs: ['private-opaque-ref'] };
async function until(predicate: () => boolean) {
  const limit = Date.now() + 1_000;
  while (!predicate()) { if (Date.now() > limit) throw new Error('state did not settle'); await Bun.sleep(1); }
}
function harness(response: () => Promise<unknown> = async () => ({ type: 'resolved', status: 'ok' }), afterTool: Promise<void> = Promise.resolve()) {
  const requests: { action: string; payload: unknown }[] = [];
  let end!: (status: string) => void;
  const gate = new Promise<string>(resolve => { end = resolve; });
  let epoch = 0;
  let streams = 0;
  const permissions = new PermissionController({ requestSingleFlight: async (action, payload) => { requests.push({ action, payload }); return response(); } });
  const runtime: BrainFrontendRuntime = {
    start: async () => {}, permissions,
    sessions: { create: async () => ({ sessionId }), list: async () => [], load: async id => ({ sessionId: id, title: 'Session', messages: [] }) },
    generations: { cancel: async () => {}, stream: async function* (request) {
      streams++;
      const f = (sequence: number, type: string, extra: Record<string, unknown> = {}) => ({ sequence, type, session_id: sessionId, generation_id: request.generationId, ...extra });
      yield f(0, 'stream_start');
      yield f(1, 'tool_use', { toolUse: { id: 'call', name: 'bash', input: { command: 'printf hello' } } });
      yield f(2, 'tool_permission_requested', { call_id: 'call', permission_request_id: 'p', tool_name: 'bash' });
      const status = await gate;
      if (status === 'completed') yield f(3, 'tool_result', { call_id: 'call', permission_request_id: 'p', outcome: 'success', output: 'hello' });
      await afterTool;
      yield f(status === 'completed' ? 4 : 3, 'finished', { status });
    } },
    connection: { epoch: () => epoch, markDisconnected: () => { epoch++; }, recover: async () => {} },
  };
  const controller = new BrainFrontendController(runtime);
  const begin = async () => {
    await controller.start(); await controller.createSession(); controller.setDraft('run tool');
    const submission = controller.submit(); await until(() => controller.snapshot().focus === 'permission');
    return { submission };
  };
  return { controller, begin, requests, end, permissions, streams: () => streams };
}

for (const decision of ['allow', 'deny'] as const) test(`${decision} dispatches one fully correlated permission decision and restores composer`, async () => {
  const h = harness(); const { submission } = await h.begin();
  h.controller.setDraft('preserved draft');
  expect(h.controller.snapshot().permission?.summary).toBe('printf hello');
  await Promise.all([h.controller.resolvePermission(decision), h.controller.resolvePermission(decision)]);
  expect(h.requests).toHaveLength(1);
  expect(h.requests[0]).toEqual({ action: 'v1/tool/resolve', payload: { session_id: sessionId, generation_id: expect.any(String), call_id: 'call', permission_request_id: 'p', granted: decision === 'allow' } });
  expect(h.controller.snapshot().focus).toBe('composer');
  expect(h.controller.snapshot().composer.draft).toBe('preserved draft');
  const tool = h.controller.snapshot().transcript.find(e => e.kind === 'tool');
  expect(tool?.kind === 'tool' ? tool.status : undefined).toBe(decision === 'allow' ? 'running' : 'pending');
  h.end('cancelled'); await submission;
  expect(h.controller.snapshot().transcript.find(e => e.kind === 'tool')).toMatchObject({ status: 'cancelled' });
  expect(h.permissions.get('p')).toBeUndefined();
});

test('terminal settlement closes unresolved permissions and rejects late frames', async () => {
  const h = harness(); const { submission } = await h.begin();
  const generationId = h.controller.snapshot().transcript[0]!.id.split(':')[0]!;
  h.end('failed'); await submission;
  await h.controller.resolvePermission('allow');
  expect(h.requests).toHaveLength(0);
  expect(h.controller.snapshot().permission).toBeUndefined();
  expect(h.controller.snapshot().focus).toBe('composer');
  expect(() => h.controller.ingestGenerationFrame(generationId, {}, 0)).toThrow('stale_generation');
});

test('cancelling disables pending permission input until Brain settles the turn', async () => {
  const h = harness(); const { submission } = await h.begin();
  await h.controller.cancel();
  expect(await h.controller.resolvePermission('allow')).toBe('ignored');
  expect(h.requests).toHaveLength(0);
  expect(h.controller.snapshot().permission).toBeUndefined();
  h.end('cancelled'); await submission;
});

test('lost decision dispatch recovers authority without replay or stale ACK repaint', async () => {
  const h = harness(async () => { throw new Error('connection_unavailable: /Users/private/socket p'); });
  const { submission } = await h.begin();
  await h.controller.resolvePermission('allow');
  expect(h.requests).toHaveLength(1);
  expect(h.streams()).toBe(1);
  expect(h.controller.snapshot()).toMatchObject({ connection: 'ready', generating: false, focus: 'composer' });
  expect(h.controller.snapshot().permission).toBeUndefined();
  h.end('completed'); await submission;
  expect(h.controller.snapshot().transcript).toEqual([]);
});

test('late permission ACK cannot repaint an authoritative completed tool', async () => {
  let ack!: (value: unknown) => void;
  const h = harness(() => new Promise(resolve => { ack = resolve; }));
  const { submission } = await h.begin();
  const decision = h.controller.resolvePermission('allow');
  h.end('completed'); await submission;
  ack({ type: 'resolved', status: 'ok' }); await decision;
  expect(h.controller.snapshot().transcript.find(e => e.kind === 'tool')).toMatchObject({ status: 'succeeded', detail: 'hello' });
  expect(h.controller.snapshot().generating).toBe(false);
});

for (const failure of ['connection_unavailable: lost ACK', 'request already settled']) test(`late ${failure} cannot interrupt a healthy stream after tool settlement`, async () => {
  let rejectAck!: (error: Error) => void;
  let finish!: () => void;
  const afterTool = new Promise<void>(resolve => { finish = resolve; });
  const h = harness(() => new Promise((_resolve, reject) => { rejectAck = reject; }), afterTool);
  const { submission } = await h.begin();
  const decision = h.controller.resolvePermission('allow');
  h.end('completed');
  await until(() => h.controller.snapshot().transcript.some(e => e.kind === 'tool' && e.status === 'succeeded'));
  rejectAck(new Error(failure)); await decision;
  expect(h.controller.snapshot().generating).toBe(true);
  expect(h.controller.snapshot().transcript.some(e => e.kind === 'error')).toBe(false);
  expect(h.controller.snapshot().composer.draft).toBe('');
  finish(); await submission;
  expect(h.controller.snapshot().transcript.find(e => e.kind === 'tool')).toMatchObject({ status: 'succeeded' });
});

test('daemon rejection shows a concise error, never redispatches or leaks details', async () => {
  const h = harness(async () => { throw new Error('request expired /Users/private p stacktrace'); });
  const { submission } = await h.begin();
  await h.controller.resolvePermission('deny'); await h.controller.resolvePermission('allow');
  expect(h.requests).toHaveLength(1);
  const errors = h.controller.snapshot().transcript.filter(e => e.kind === 'error');
  expect(errors).toHaveLength(1);
  expect(JSON.stringify(errors)).not.toContain('/Users/');
  expect(h.controller.snapshot().focus).toBe('composer');
  h.end('failed'); await submission;
});

test('context command is count-only, session-scoped and never a submitted prompt', async () => {
  let retrieval = 1;
  let submissions = 0;
  const contexts = new BrainContextState();
  const runtime: BrainFrontendRuntime = {
    start: async () => {},
    sessions: { create: async () => ({ sessionId }), list: async () => [], load: async id => ({ sessionId: id, title: 'History', messages: [{ role: 'assistant', content: 'history' }] }) },
    generations: { cancel: async () => {}, stream: async function* (request) {
      submissions++;
      const f = (sequence: number, type: string, extra: object = {}) => ({ sequence, type, session_id: request.sessionId, generation_id: request.generationId, ...extra });
      yield f(0, 'stream_start', { metadata: { brain_context: { ...context, retrieval_count: retrieval, source_refs: retrieval ? context.source_refs : [] } } });
      yield f(1, 'finished', { status: 'completed' });
    } },
    connection: { epoch: () => 0, markDisconnected: () => {}, recover: async () => {} },
  };
  const controller = new BrainFrontendController(runtime, contexts);
  await controller.start(); await controller.createSession();
  controller.setDraft('answer'); await controller.submit();
  controller.setDraft('/context'); await controller.submit();
  expect(submissions).toBe(1);
  expect(controller.snapshot()).toMatchObject({ focus: 'context', contextSummary: { retrievalCount: 1, contextTokenCount: 24 } });
  expect(JSON.stringify(controller.snapshot())).not.toContain('private-opaque-ref');
  controller.closeContext();
  retrieval = 0; controller.setDraft('no retrieval'); await controller.submit();
  expect(controller.snapshot().contextSummary).toBeUndefined();
  expect(contexts.counts()).toEqual({ active: 0, latestCompleted: 0 });
  retrieval = 1; controller.setDraft('again'); await controller.submit();
  await controller.selectSession('01ARZ3NDEKTSV4RRFFQ69G5FAW');
  expect(controller.snapshot().contextSummary).toBeUndefined();
  await controller.selectSession(sessionId);
  expect(controller.snapshot().contextSummary).toBeUndefined();
});

test('unresolved permission cannot promote a successful-looking context snapshot', async () => {
  const h = harness();
  const { submission } = await h.begin();
  const activeId = h.controller.snapshot().transcript[0]!.id.split(':')[0]!;
  // Inject an authoritative end while a permission is still unresolved.
  // It closes UI state but is not a retrieval-bearing successful response.
  h.controller.ingestGenerationFrame(activeId, { sequence: 3, type: 'stream_end', session_id: sessionId, generation_id: activeId, metadata: { brain_context: context } }, 0);
  h.controller.ingestGenerationFrame(activeId, { sequence: 4, type: 'finished', status: 'completed', session_id: sessionId, generation_id: activeId }, 0);
  expect(h.controller.snapshot().contextSummary).toBeUndefined();
  expect(h.controller.snapshot().transcript.find(e => e.kind === 'tool')).toMatchObject({ status: 'interrupted' });
  h.end('failed'); await submission;
});

test('tool updates and expansion retain the locked transcript position', async () => {
  const h = harness(); const { submission } = await h.begin();
  h.controller.scrollUp();
  const anchor = h.controller.snapshot().scrollOffset;
  await h.controller.resolvePermission('allow');
  expect(h.controller.snapshot().followTranscript).toBe(false);
  expect(h.controller.snapshot().scrollOffset).toBe(anchor);
  h.end('completed'); await submission;
  h.controller.toggleToolDetail();
  expect(h.controller.snapshot().followTranscript).toBe(false);
  expect(h.controller.snapshot().focus).toBe('composer');
  expect(h.controller.snapshot().transcript.find(e => e.kind === 'tool')).toMatchObject({ action: 'printf hello', expanded: true });
});
