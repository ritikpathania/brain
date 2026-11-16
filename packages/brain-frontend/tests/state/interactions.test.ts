import { describe, expect, test } from 'bun:test';

import { BrainFrontendController, type BrainFrontendRuntime } from '../../src/state/interactions.js';
import type { BrainSession } from '../../src/brain/sessions.js';
import { BrainContextState } from '../../src/brain/context.js';

const sessionId = '01ARZ3NDEKTSV4RRFFQ69G5FAV';

describe('terminal interaction controller', () => {
  test('projects startup failures as a visible failed connection state instead of abandoning the terminal', async () => {
    const runtime = runtimeWithFrames([]);
    const controller = new BrainFrontendController({
      ...runtime,
      start: async () => { throw new Error('unsupported_protocol_version: daemon is incompatible'); },
    });

    await expect(controller.start()).resolves.toBe('failed');
    expect(controller.snapshot().connection).toBe('failed');
    expect(controller.snapshot().transcript).toEqual([{
      kind: 'error', id: expect.any(String), message: 'Brain daemon is incompatible with this frontend.',
    }]);
  });

  test('submits a multiline draft once and projects separate thinking and text states', async () => {
    const runtime = runtimeWithFrames([
      frame(0, 'stream_start'),
      frame(1, 'thinking_start'),
      frame(2, 'thinking_delta', { thinking: 'Checking context' }),
      frame(3, 'thinking_end'),
      frame(4, 'token', { token: 'Answer' }),
      frame(5, 'finished'),
    ]);
    const controller = new BrainFrontendController(runtime);
    await controller.start();
    await controller.createSession();
    controller.setDraft('first line');
    controller.insertNewline();
    controller.appendDraft('second line');

    expect(await controller.submit()).toBe('submitted');
    expect(await controller.submit()).toBe('ignored');

    const snapshot = controller.snapshot();
    expect(runtime.streamRequests).toHaveLength(1);
    expect(runtime.streamRequests[0]?.messages).toEqual([{ role: 'user', content: 'first line\nsecond line' }]);
    expect(snapshot.transcript.some(entry => entry.kind === 'thinking' && entry.text === 'Checking context')).toBe(true);
    expect(snapshot.transcript.some(entry => entry.kind === 'assistant' && entry.text === 'Answer')).toBe(true);
    expect(snapshot.generating).toBe(false);
    expect(snapshot.composer.draft).toBe('');
  });

  test('deduplicates cancellation and waits for Brain terminal settlement', async () => {
    let releaseTerminal: (() => void) | undefined;
    const terminalGate = new Promise<void>(resolve => { releaseTerminal = resolve; });
    const runtime = runtimeWithGenerator(async function* () {
      yield frame(0, 'stream_start');
      await terminalGate;
      yield frame(1, 'cancelled');
    });
    const controller = new BrainFrontendController(runtime);
    await controller.start();
    await controller.createSession();
    controller.setDraft('cancel me');

    const submission = controller.submit();
    await waitFor(() => controller.snapshot().generating);
    expect(await controller.cancel()).toBe('requested');
    expect(await controller.cancel()).toBe('ignored');
    expect(controller.snapshot().cancelling).toBe(true);
    releaseTerminal?.();
    await submission;

    expect(runtime.cancelCalls).toBe(1);
    expect(controller.snapshot().generating).toBe(false);
    expect(controller.snapshot().transcript.some(entry => entry.kind === 'assistant' && entry.status === 'cancelled')).toBe(true);
  });

  test('holds one early cancellation intent until Brain accepts the same generation', async () => {
    let accept!: () => void;
    let finish!: () => void;
    const acceptance = new Promise<void>(resolve => { accept = resolve; });
    const terminal = new Promise<void>(resolve => { finish = resolve; });
    let registered = false;
    let cancelled = false;
    const runtime = runtimeWithGenerator(async function* () {
      await acceptance;
      registered = true;
      yield frame(0, 'stream_start');
      await terminal;
      yield frame(1, cancelled ? 'cancelled' : 'finished');
    });
    const controller = new BrainFrontendController({
      ...runtime,
      generations: { ...runtime.generations, cancel: async correlation => {
        runtime.cancelCalls += 1;
        expect(correlation).toEqual({ sessionId, generationId: runtime.streamRequests[0]!.generationId });
        // The real daemon acknowledges even when the generation is not yet registered.
        cancelled = registered;
      } },
    });
    await controller.start();
    await controller.createSession();
    controller.setDraft('cancel before acceptance');
    const submission = controller.submit();
    const cancellation = controller.cancel();
    try {
      await new Promise<void>(resolve => setImmediate(resolve));
      expect(controller.snapshot().cancelling).toBe(true);
      expect(runtime.cancelCalls).toBe(0);
      expect(await controller.cancel()).toBe('ignored');
      accept();
      expect(await cancellation).toBe('requested');
      expect(runtime.cancelCalls).toBe(1);
      finish();
      await submission;
      expect(controller.snapshot().transcript.filter(entry => entry.kind === 'assistant')).toEqual([
        expect.objectContaining({ status: 'cancelled' }),
      ]);
      expect(controller.snapshot().generating).toBe(false);
    } finally {
      accept();
      finish();
      await Promise.all([submission, cancellation]);
    }
  });

  test('abandons an early cancellation intent if stream acceptance fails', async () => {
    let fail!: () => void;
    const failure = new Promise<void>(resolve => { fail = resolve; });
    const runtime = runtimeWithGenerator(async function* () {
      await failure;
      throw new Error('unsupported_protocol_version: refused before stream acceptance');
    });
    const controller = new BrainFrontendController(runtime);
    await controller.start();
    await controller.createSession();
    controller.setDraft('preserve failed draft');
    const submission = controller.submit();
    const cancellation = controller.cancel();
    fail();
    await submission;
    expect(await cancellation).toBe('ignored');
    expect(runtime.cancelCalls).toBe(0);
    expect(controller.snapshot().cancelling).toBe(false);
    expect(controller.snapshot().composer.draft).toBe('preserve failed draft');
  });

  test('ends pending tool presentation when Brain cancels or fails a generation', async () => {
    for (const status of ['cancelled', 'failed']) {
      const runtime = runtimeWithFrames([
        frame(0, 'stream_start'),
        frame(1, 'tool_use', { toolUse: { id: 'call-1', name: 'bash', input: { command: 'printf ok' } } }),
        frame(2, 'finished', { status }),
      ]);
      const controller = new BrainFrontendController(runtime);
      await controller.start();
      await controller.createSession();
      controller.setDraft('terminal tool');
      await controller.submit();
      expect(controller.snapshot().transcript).toEqual(expect.arrayContaining([
        expect.objectContaining({ kind: 'tool', label: 'bash', status }),
      ]));
      expect(controller.snapshot().generating).toBe(false);
    }
  });

  test('ignores an old cancellation failure after a later generation owns cancellation state', async () => {
    let finishFirst!: () => void;
    let finishSecond!: () => void;
    let rejectFirstCancel!: (error: Error) => void;
    const firstTerminal = new Promise<void>(resolve => { finishFirst = resolve; });
    const secondTerminal = new Promise<void>(resolve => { finishSecond = resolve; });
    const firstCancel = new Promise<void>((_resolve, reject) => { rejectFirstCancel = reject; });
    let turn = 0;
    let cancellations = 0;
    const runtime = runtimeWithGenerator(async function* () {
      const terminal = turn++ === 0 ? firstTerminal : secondTerminal;
      yield frame(0, 'stream_start');
      await terminal;
      yield frame(1, 'finished');
    });
    const controller = new BrainFrontendController({
      ...runtime,
      generations: { ...runtime.generations, cancel: async () => { if (cancellations++ === 0) await firstCancel; } },
    });
    await controller.start();
    await controller.createSession();
    controller.setDraft('first');
    const firstSubmission = controller.submit();
    const oldCancellation = controller.cancel();
    finishFirst();
    await firstSubmission;
    controller.setDraft('second');
    const secondSubmission = controller.submit();
    await controller.cancel();
    rejectFirstCancel(new Error('connection_unavailable: old cancel response'));
    await oldCancellation;
    expect(controller.snapshot().cancelling).toBe(true);
    expect(controller.snapshot().transcript.some(entry => entry.kind === 'error')).toBe(false);
    expect(cancellations).toBe(2);
    finishSecond();
    await secondSubmission;
  });

  test('projects correlated tool and permission events as noninteractive placeholders', async () => {
    const runtime = runtimeWithFrames([
      frame(0, 'stream_start'),
      frame(1, 'tool_use', { toolUse: { id: 'call-1', name: 'bash', input: { command: 'printf ok' } } }),
      frame(2, 'tool_permission_requested', {
        call_id: 'call-1',
        permission_request_id: 'permission-1',
        tool_name: 'bash',
        input: { command: 'printf ok' },
      }),
      frame(3, 'tool_denied', { call_id: 'call-1', permission_request_id: 'permission-1', outcome: 'denied' }),
      frame(4, 'finished'),
    ]);
    const controller = new BrainFrontendController(runtime);
    let permissionShown = false;
    controller.subscribe(state => {
      if (state.transcript.some(entry => entry.kind === 'permission' && entry.label === 'bash')) permissionShown = true;
    });
    await controller.start();
    await controller.createSession();
    controller.setDraft('run a tool');

    await controller.submit();

    expect(controller.snapshot().transcript).toEqual(expect.arrayContaining([
      expect.objectContaining({ kind: 'tool', label: 'bash', status: 'failed' }),
    ]));
    expect(permissionShown).toBe(true);
    expect(controller.snapshot().transcript.some(entry => entry.kind === 'permission')).toBe(false);
  });

  test('keeps composer focus and an upward scroll lock while stream updates continue', async () => {
    let releaseStream: (() => void) | undefined;
    const streamGate = new Promise<void>(resolve => { releaseStream = resolve; });
    const runtime = runtimeWithGenerator(async function* () {
      yield frame(0, 'stream_start');
      await streamGate;
      yield frame(1, 'token', { token: 'new output' });
      yield frame(2, 'finished');
    });
    const controller = new BrainFrontendController(runtime);
    await controller.start();
    await controller.createSession();
    controller.setDraft('preserve focus');

    const submission = controller.submit();
    await waitFor(() => controller.snapshot().generating && controller.snapshot().transcript.length === 1);
    controller.scrollUp();
    const before = controller.snapshot().scrollOffset;
    expect(controller.snapshot().focus).toBe('composer');
    expect(controller.snapshot().followTranscript).toBe(false);
    releaseStream?.();
    await submission;

    expect(controller.snapshot().focus).toBe('composer');
    expect(controller.snapshot().followTranscript).toBe(false);
    expect(controller.snapshot().scrollOffset).toBeGreaterThan(before);
  });

  test('preserves a locked transcript anchor when terminal settlement appends a row without answer text', async () => {
    let finish!: () => void;
    const gate = new Promise<void>(resolve => { finish = resolve; });
    const runtime = runtimeWithGenerator(async function* () {
      yield frame(0, 'stream_start');
      await gate;
      yield frame(1, 'cancelled');
    }, { load: async id => session(id, Array.from({ length: 12 }, (_, index) => ({ role: 'user', content: `history-${index}` }))) });
    const controller = new BrainFrontendController(runtime);
    await controller.start();
    await controller.selectSession(sessionId);
    controller.setDraft('cancel before text');
    const submission = controller.submit();
    await waitFor(() => controller.snapshot().transcript.length === 13);
    controller.scrollUp();
    const before = controller.snapshot();
    const anchorBefore = before.transcript.length - before.scrollOffset;
    finish();
    await submission;
    const after = controller.snapshot();
    expect(after.transcript.length - after.scrollOffset).toBe(anchorBefore);
    expect(after.followTranscript).toBe(false);
  });

  test('restores the composer focus and draft after picker close or recoverable transport failure without replay', async () => {
    const runtime = runtimeWithGenerator(async function* () {
      throw new Error('connection_unavailable: socket closed');
    });
    const controller = new BrainFrontendController(runtime);
    await controller.start();
    await controller.createSession();
    controller.setDraft('keep this');
    await controller.openSessionPicker();
    expect(controller.snapshot().focus).toBe('session-picker');
    controller.closeSessionPicker();
    expect(controller.snapshot().focus).toBe('composer');

    expect(await controller.submit()).toBe('submitted');
    expect(controller.snapshot().composer.draft).toBe('keep this');
    expect(runtime.streamRequests).toHaveLength(1);
    expect(runtime.recoverCalls).toBe(1);
  });

  test('hydrates exact Brain history on session selection and keeps scroll ownership explicit', async () => {
    const runtime = runtimeWithFrames([], {
      list: async () => [{
        sessionId: '01ARZ3NDEKTSV4RRFFQ69G5FAV', title: 'Existing', messageCount: 1, pinned: false, archived: false,
      }],
      load: async (id: string) => session(id, [{ role: 'assistant', content: 'authoritative history' }]),
    });
    const controller = new BrainFrontendController(runtime);
    await controller.start();

    await controller.openSessionPicker();
    await controller.selectSession(sessionId);
    controller.scrollUp();
    expect(controller.snapshot().followTranscript).toBe(false);
    controller.scrollToBottom();

    const snapshot = controller.snapshot();
    expect(snapshot.session?.sessionId).toBe(sessionId);
    expect(snapshot.transcript).toEqual([{ kind: 'assistant', id: 'history-0', text: 'authoritative history', status: 'complete' }]);
    expect(snapshot.followTranscript).toBe(true);
    expect(snapshot.focus).toBe('composer');
  });

  test('rejects a stale epoch frame before it can repaint the current transcript', async () => {
    const runtime = runtimeWithFrames([]);
    const controller = new BrainFrontendController(runtime);
    await controller.start();
    await controller.createSession();

    expect(() => controller.ingestGenerationFrame('old-generation', frame(0, 'stream_start'), 99)).toThrow('stale_generation');
    expect(controller.snapshot().transcript).toEqual([]);
  });

  test('settles an interrupted stream and preserves its draft without replay', async () => {
    const runtime = runtimeWithFrames([frame(0, 'stream_start'), frame(1, 'token', { token: 'partial' })]);
    const controller = new BrainFrontendController(runtime);
    await controller.start();
    await controller.createSession();
    controller.setDraft('interrupted request');

    await controller.submit();

    expect(controller.snapshot().generating).toBe(false);
    expect(controller.snapshot().composer.draft).toBe('interrupted request');
    expect(runtime.streamRequests).toHaveLength(1);
    expect(runtime.recoverCalls).toBe(1);
  });

  test('does not replace the active session or create another session during generation', async () => {
    let releaseStream!: () => void;
    const gate = new Promise<void>(resolve => { releaseStream = resolve; });
    let creations = 0;
    const runtime = runtimeWithGenerator(async function* () {
      yield frame(0, 'stream_start');
      await gate;
      yield frame(1, 'token', { token: 'same session' });
      yield frame(2, 'finished');
    }, { create: async () => { creations += 1; return { sessionId }; } });
    const controller = new BrainFrontendController(runtime);
    await controller.start();
    await controller.createSession();
    controller.setDraft('active');
    const submission = controller.submit();
    await waitFor(() => controller.snapshot().generating);

    await controller.selectSession('01ARZ3NDEKTSV4RRFFQ69G5FAW');
    await controller.createSession();
    await controller.openSessionPicker();
    expect(controller.snapshot().session?.sessionId).toBe(sessionId);
    expect(controller.snapshot().focus).toBe('composer');
    expect(creations).toBe(1);
    releaseStream();
    await submission;
  });

  test('settles partial presentation rows and permissions when the stream guard rejects a frame', async () => {
    for (const frames of [
      [frame(0, 'stream_start'), frame(1, 'thinking_start'), frame(2, 'thinking_delta', { thinking: 'partial thought' }), frame(4, 'thinking_end')],
      [
        frame(0, 'stream_start'), frame(1, 'token', { token: 'partial answer' }),
        frame(2, 'tool_use', { toolUse: { id: 'call-1', name: 'bash', input: { command: 'printf ok' } } }),
        frame(3, 'tool_permission_requested', { call_id: 'call-1', permission_request_id: 'permission-1', tool_name: 'bash', input: { command: 'printf ok' } }),
        frame(5, 'finished'),
      ],
    ]) {
      const runtime = runtimeWithFrames(frames);
      const controller = new BrainFrontendController(runtime);
      await controller.start();
      await controller.createSession();
      controller.setDraft('retry only explicitly');
      await controller.submit();

      const snapshot = controller.snapshot();
      expect(snapshot.generating).toBe(false);
      expect(snapshot.composer.draft).toBe('retry only explicitly');
      expect(snapshot.transcript.some(entry => entry.kind === 'thinking' && entry.status === 'streaming')).toBe(false);
      expect(snapshot.transcript.some(entry => entry.kind === 'assistant' && entry.status === 'streaming')).toBe(false);
      expect(snapshot.transcript.some(entry => entry.kind === 'tool' && entry.status === 'pending')).toBe(false);
      expect(snapshot.transcript.some(entry => entry.kind === 'permission')).toBe(false);
      expect(snapshot.transcript.some(entry => entry.kind === 'error')).toBe(true);
      expect(runtime.streamRequests).toHaveLength(1);
    }
  });

  test('presents session errors and suppresses duplicate create while an operation is pending', async () => {
    let releaseCreate!: () => void;
    const gate = new Promise<void>(resolve => { releaseCreate = resolve; });
    let creations = 0;
    const runtime = runtimeWithFrames([], {
      create: async () => { creations += 1; await gate; return { sessionId }; },
      load: async () => { throw new Error('session_not_found: missing'); },
    });
    const controller = new BrainFrontendController(runtime);
    await controller.start();
    const creation = controller.createSession();
    await controller.createSession();
    releaseCreate();
    await creation;
    expect(creations).toBe(1);
    await expect(controller.selectSession(sessionId)).resolves.toBeUndefined();
    expect(controller.snapshot().transcript.some(entry => entry.kind === 'error' && entry.message.includes('no longer exists'))).toBe(true);
  });

  test('refreshes an idle disconnected session without changing the draft or focus', async () => {
    const runtime = runtimeWithFrames([]);
    const controller = new BrainFrontendController({
      ...runtime,
      checkConnection: async () => { throw new Error('connection_unavailable: stopped'); },
    });
    await controller.start();
    await controller.createSession();
    controller.setDraft('unsent draft');
    await controller.refreshConnection();
    expect(controller.snapshot().connection).toBe('ready');
    expect(controller.snapshot().composer.draft).toBe('unsent draft');
    expect(controller.snapshot().focus).toBe('composer');
    expect(runtime.streamRequests).toHaveLength(0);
    expect(runtime.recoverCalls).toBe(1);
  });

  test('keeps short history visible when scrolling and excludes stored tool payloads from presentation', async () => {
    const runtime = runtimeWithFrames([], {
      load: async id => session(id, [
        { role: 'user', content: 'earliest message' },
        { role: 'tool', content: '{"call_id":"private-id","raw":"internal tool details"}' },
      ]),
    });
    const controller = new BrainFrontendController(runtime);
    await controller.start();
    await controller.selectSession(sessionId);
    controller.scrollUp();
    expect(controller.snapshot().scrollOffset).toBe(1);
    expect(controller.snapshot().transcript[1]).toEqual({ kind: 'tool', id: 'history-1', label: 'Stored tool outcome', status: 'interrupted', summary: 'Outcome unavailable' });
  });

  test('hydrates tool outcomes stored before the typed terminal statuses', async () => {
    const runtime = runtimeWithFrames([], {
      load: async id => session(id, [false, true].map(isError => ({
        role: 'tool',
        content: JSON.stringify({ type: 'tool_event', v: 1, name: 'bash', outcome: 'executed', is_error: isError, output: 'stored output' }),
      }))),
    });
    const controller = new BrainFrontendController(runtime);
    await controller.start();
    await controller.selectSession(sessionId);
    expect(controller.snapshot().transcript).toMatchObject([
      { kind: 'tool', status: 'succeeded', detail: 'stored output' },
      { kind: 'tool', status: 'failed', detail: undefined },
    ]);
  });

  test('releases context state at terminal settlement and interrupted recovery', async () => {
    const contexts = new BrainContextState();
    const runtime = runtimeWithFrames([frame(0, 'stream_start'), frame(1, 'finished')]);
    const controller = new BrainFrontendController(runtime, contexts);
    await controller.start();
    await controller.createSession();
    for (let turn = 0; turn < 100; turn += 1) {
      controller.setDraft(`turn ${turn}`);
      await controller.submit();
    }
    expect(contexts.counts()).toEqual({ active: 0, latestCompleted: 0 });
    const interrupted = new BrainFrontendController(runtimeWithFrames([frame(0, 'stream_start')]), contexts);
    await interrupted.start();
    await interrupted.createSession();
    interrupted.setDraft('interrupt');
    await interrupted.submit();
    expect(contexts.counts()).toEqual({ active: 0, latestCompleted: 0 });
  });

  test('ignores a late health failure belonging to a session that was already replaced', async () => {
    let failProbe!: (error: Error) => void;
    const probe = new Promise<void>((_resolve, reject) => { failProbe = reject; });
    const runtime = runtimeWithFrames([]);
    const controller = new BrainFrontendController({ ...runtime, checkConnection: () => probe });
    await controller.start();
    await controller.createSession();
    const refresh = controller.refreshConnection();
    const nextSessionId = '01ARZ3NDEKTSV4RRFFQ69G5FAW';
    await controller.selectSession(nextSessionId);
    failProbe(new Error('connection_unavailable: old probe'));
    await refresh;
    expect(controller.snapshot().session?.sessionId).toBe(nextSessionId);
    expect(runtime.recoverCalls).toBe(0);
  });
});

function frame(sequence: number, type: string, extra: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    type,
    sequence,
    session_id: sessionId,
    generation_id: 'generated-by-controller',
    status: type === 'finished' ? 'completed' : 'in_progress',
    ...extra,
  };
}

function session(id: string, messages: BrainSession['messages'] = []): BrainSession {
  return { sessionId: id, title: 'Session', messages };
}

function runtimeWithFrames(
  frames: readonly Record<string, unknown>[],
  sessions: Partial<TestRuntime['sessions']> = {},
): TestRuntime {
  return runtimeWithGenerator(async function* () { yield* frames; }, sessions);
}

function runtimeWithGenerator(
  generator: () => AsyncGenerator<Record<string, unknown>, void, void>,
  sessionOverrides: Partial<TestRuntime['sessions']> = {},
): TestRuntime {
  const runtime: TestRuntime = {
    streamRequests: [],
    cancelCalls: 0,
    recoverCalls: 0,
    start: async () => undefined,
    sessions: {
      create: async () => ({ sessionId }),
      list: async () => [],
      load: async id => session(id),
      ...sessionOverrides,
    },
    generations: {
      stream: request => {
        runtime.streamRequests.push(request);
        return correlatedFrames(generator(), request);
      },
      cancel: async () => { runtime.cancelCalls += 1; },
    },
    connection: {
      epoch: () => 0,
      markDisconnected: () => undefined,
      recover: async () => { runtime.recoverCalls += 1; },
    },
  };
  return runtime;
}

async function* correlatedFrames(
  frames: AsyncGenerator<Record<string, unknown>, void, void>,
  request: Readonly<{ sessionId: string; generationId: string }>,
): AsyncGenerator<Record<string, unknown>, void, void> {
  for await (const raw of frames) {
    yield { ...raw, session_id: request.sessionId, generation_id: request.generationId };
  }
}

type TestRuntime = BrainFrontendRuntime & {
  streamRequests: Array<Readonly<{ sessionId: string; generationId: string; messages: readonly Readonly<{ role: string; content: string }>[] }>>;
  cancelCalls: number;
  recoverCalls: number;
};

async function waitFor(predicate: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 20; attempt += 1) {
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 0));
  }
  throw new Error('timed out waiting for controller state');
}
