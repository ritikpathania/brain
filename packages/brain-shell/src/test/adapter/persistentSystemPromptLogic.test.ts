import { describe, expect, it } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import type {
  BrainBackendClient,
  BrainGenerationRequest,
  BrainSession,
  BrainStreamChunk,
} from '../../client/BrainBackendClient.js';

class MockPersistentBackendClient implements Partial<BrainBackendClient> {
  public sessions = new Map<string, BrainSession>();
  public lastRequest?: BrainGenerationRequest;
  public persistedPrompts = new Map<string, string | null>();

  async createSession(req?: { title?: string }): Promise<any> {
    const sessionId = `sess-${Date.now()}`;
    const s: BrainSession = {
      id: sessionId,
      title: req?.title || 'Test Session',
      createdAtMs: Date.now(),
      updatedAtMs: Date.now(),
      pinned: false,
      archived: false,
      systemPrompt: null,
      messages: [],
    };
    this.sessions.set(sessionId, s);
    return { sessionId, title: s.title, createdAtMs: s.createdAtMs };
  }

  async loadSession(sessionId: string): Promise<{ session: BrainSession }> {
    const s = this.sessions.get(sessionId);
    if (!s) {
      throw new Error(`Session ${sessionId} not found`);
    }
    return { session: s };
  }

  async forkSession(req: { sourceSessionId: string; newTitle?: string }): Promise<any> {
    const src = this.sessions.get(req.sourceSessionId);
    const newId = `fork-${Date.now()}`;
    const forked: BrainSession = {
      id: newId,
      title: req.newTitle || `${src?.title || 'Session'} (fork)`,
      createdAtMs: Date.now(),
      updatedAtMs: Date.now(),
      pinned: false,
      archived: false,
      systemPrompt: src?.systemPrompt ?? null,
      messages: src ? [...src.messages] : [],
    };
    this.sessions.set(newId, forked);
    return { newSessionId: newId, sourceSessionId: req.sourceSessionId, clonedMessagesCount: forked.messages.length };
  }

  async setSessionSystemPrompt(sessionId: string, systemPrompt: string | null): Promise<void> {
    this.persistedPrompts.set(sessionId, systemPrompt);
    const s = this.sessions.get(sessionId);
    if (s) {
      s.systemPrompt = systemPrompt;
    }
  }

  async *streamText(request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
    this.lastRequest = request;
    yield {
      type: 'token',
      token: 'Response from mock',
      sequence: 1,
    };
    yield {
      type: 'finished',
      status: 'completed',
      sequence: 2,
    };
  }

  onChunk(): () => void {
    return () => {};
  }
}

describe('Increment 48: Persistent Per-Session System Prompt Logic', () => {
  it('restores custom system prompt when resuming a session that has one saved', async () => {
    const mock = new MockPersistentBackendClient();
    mock.sessions.set('sess-1', {
      id: 'sess-1',
      title: 'Rust Architecture Session',
      createdAtMs: 1000,
      updatedAtMs: 2000,
      pinned: false,
      archived: false,
      systemPrompt: 'You are an expert Rust systems architect.',
      messages: [{ id: 'm1', role: 'user', content: 'Design memory arena' }],
    });

    const controller = new SessionController(mock as unknown as BrainBackendClient);
    expect(controller.currentSystemPrompt).toBeNull();

    await controller.resumeSession('sess-1');

    expect(controller.activeSessionId).toBe('sess-1');
    expect(controller.currentSystemPrompt).toBe('You are an expert Rust systems architect.');
  });

  it('restores null system prompt when resuming a session with null system prompt', async () => {
    const mock = new MockPersistentBackendClient();
    mock.sessions.set('sess-2', {
      id: 'sess-2',
      title: 'Standard Session',
      createdAtMs: 1000,
      updatedAtMs: 2000,
      pinned: false,
      archived: false,
      systemPrompt: null,
      messages: [],
    });

    const controller = new SessionController(mock as unknown as BrainBackendClient);
    controller.setSystemPrompt('Temporary prompt in current state');
    expect(controller.currentSystemPrompt).toBe('Temporary prompt in current state');

    await controller.resumeSession('sess-2');

    expect(controller.activeSessionId).toBe('sess-2');
    expect(controller.currentSystemPrompt).toBeNull();
  });

  it('persists system prompt changes to daemon when active session exists', async () => {
    const mock = new MockPersistentBackendClient();
    mock.sessions.set('sess-3', {
      id: 'sess-3',
      title: 'Live Session',
      createdAtMs: 1000,
      updatedAtMs: 2000,
      pinned: false,
      archived: false,
      systemPrompt: null,
      messages: [],
    });

    const controller = new SessionController(mock as unknown as BrainBackendClient);
    await controller.resumeSession('sess-3');

    // Configure new prompt
    controller.setSystemPrompt('You are a Linux kernel developer.');
    expect(mock.persistedPrompts.get('sess-3')).toBe('You are a Linux kernel developer.');
    expect(mock.sessions.get('sess-3')?.systemPrompt).toBe('You are a Linux kernel developer.');

    // Clear prompt
    controller.setSystemPrompt('clear');
    expect(mock.persistedPrompts.get('sess-3')).toBeNull();
    expect(mock.sessions.get('sess-3')?.systemPrompt).toBeNull();
  });

  it('preserves system prompt across session forking', async () => {
    const mock = new MockPersistentBackendClient();
    mock.sessions.set('sess-parent', {
      id: 'sess-parent',
      title: 'Parent Session',
      createdAtMs: 1000,
      updatedAtMs: 2000,
      pinned: false,
      archived: false,
      systemPrompt: 'You are a compiler engineer specializing in LLVM.',
      messages: [{ id: 'm1', role: 'user', content: 'Generate IR' }],
    });

    const controller = new SessionController(mock as unknown as BrainBackendClient);
    await controller.resumeSession('sess-parent');
    expect(controller.currentSystemPrompt).toBe('You are a compiler engineer specializing in LLVM.');

    const forkedId = await controller.forkCurrentSession('Child Session');
    expect(forkedId).toBeDefined();
    expect(controller.activeSessionId).toBe(forkedId);
    expect(controller.currentSystemPrompt).toBe('You are a compiler engineer specializing in LLVM.');
  });

  it('submits turn with restored system prompt after session resume', async () => {
    const mock = new MockPersistentBackendClient();
    mock.sessions.set('sess-resumed', {
      id: 'sess-resumed',
      title: 'Resumed Turn Session',
      createdAtMs: 1000,
      updatedAtMs: 2000,
      pinned: false,
      archived: false,
      systemPrompt: 'Always output JSON.',
      messages: [],
    });

    const controller = new SessionController(mock as unknown as BrainBackendClient);
    await controller.resumeSession('sess-resumed');

    await controller.submit('List 3 fruits');

    expect(mock.lastRequest).toBeDefined();
    expect(mock.lastRequest?.sessionId).toBe('sess-resumed');
    expect(mock.lastRequest?.systemPrompt).toBe('Always output JSON.');
  });

  it('reflects restored prompt in session info report', async () => {
    const mock = new MockPersistentBackendClient();
    mock.sessions.set('sess-info', {
      id: 'sess-info',
      title: 'Info Report Session',
      createdAtMs: 1000,
      updatedAtMs: 2000,
      pinned: false,
      archived: false,
      systemPrompt: 'Senior Security Auditor instructions',
      messages: [{ id: 'm1', role: 'user', content: 'Check bounds' }],
    });

    const controller = new SessionController(mock as unknown as BrainBackendClient);
    await controller.resumeSession('sess-info');

    controller.reportSessionInfo();
    const rows = controller.getSnapshot().rows;
    const lastRow = rows[rows.length - 1];
    expect((lastRow as any)?.text).toContain('Senior Security Auditor instructions');
  });
});
