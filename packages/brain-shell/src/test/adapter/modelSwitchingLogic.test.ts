/**
 * Inc 38: Unit tests for SessionController model discovery, resolution, and switching.
 */
import { describe, it, expect, mock } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import type { BrainBackendClient, BrainModelDescriptor } from '../../client/BrainBackendClient.js';

describe('SessionController model discovery and switching', () => {
  const MOCK_MODELS: BrainModelDescriptor[] = [
    {
      id: 'brain-default',
      name: 'Brain Default',
      provider: 'local',
      contextWindow: 128000,
      maxOutputTokens: 8192,
      supportsThinking: true,
      supportsTools: true,
      isDefault: true,
    },
    {
      id: 'claude-3-5-sonnet-20241022',
      name: 'Claude 3.5 Sonnet',
      provider: 'anthropic',
      contextWindow: 200000,
      maxOutputTokens: 8192,
      supportsThinking: true,
      supportsTools: true,
      isDefault: false,
    },
    {
      id: 'gpt-4o-2024-08-06',
      name: 'GPT-4o',
      provider: 'openai',
      contextWindow: 128000,
      maxOutputTokens: 4096,
      supportsThinking: false,
      supportsTools: true,
      isDefault: false,
    },
  ];

  function makeMockClient(overrides: Partial<BrainBackendClient> = {}): BrainBackendClient {
    return {
      streamText: mock((_req) => (async function* () {})()),
      retrieveContext: mock(async () => ({ memories: [] })),
      createSession: mock(async () => ({ sessionId: 'sess-1', title: 'New', createdAtMs: Date.now() })),
      loadSession: mock(async (sid) => ({ session: { id: sid, title: 'Loaded', createdAtMs: 1, updatedAtMs: 1, pinned: false, archived: false, messages: [] } })),
      forkSession: mock(async () => ({ session: { id: 'sess-2', title: 'Forked', createdAtMs: 2, updatedAtMs: 2, pinned: false, archived: false, messages: [] } })),
      deleteSession: mock(async (sid) => ({ success: true, sessionId: sid, deletedTurnsCount: 0 })),
      archiveSession: mock(async () => {}),
      restoreSession: mock(async () => {}),
      renameSession: mock(async () => {}),
      pinSession: mock(async () => {}),
      listSessions: mock(async () => []),
      listAuthoritativeSessions: mock(async () => ({ sessions: [], total: 0 })),
      loadAuthoritativeSession: mock(async (sid) => ({ session: { id: sid, title: 'Auth', createdAtMs: 1, updatedAtMs: 1, messages: [] } })),
      triggerConsolidation: mock(async () => ({ actionsApplied: 0, promoted: 0, merged: 0, archived: 0, pruned: 0, durationMs: 0, errors: [] })),
      isAvailable: mock(async () => true),
      startSession: mock(async () => ({ sessionId: 's1' })),
      appendTurn: mock(async () => ({})),
      completeTurn: mock(async () => ({})),
      searchMemory: mock(async () => ({ results: [] })),
      storeMemory: mock(async () => ({ success: true, nodeId: 'n1' })),
      deleteMemory: mock(async () => ({ success: true })),
      searchTranscripts: mock(async () => ({ matches: [], totalMatches: 0, query: '' })),
      sendToolFeedback: mock(async () => ({ success: true })),
      listModels: mock(async () => MOCK_MODELS),
      resolveModel: mock(async (query?: string) => {
        if (!query || query === 'default') return MOCK_MODELS[0]!;
        const hit = MOCK_MODELS.find(
          (m) =>
            m.id.toLowerCase() === query.toLowerCase() ||
            m.name.toLowerCase().includes(query.toLowerCase()) ||
            m.provider.toLowerCase() === query.toLowerCase(),
        );
        if (!hit) throw new Error(`Model '${query}' not found`);
        return hit;
      }),
      ...overrides,
    };
  }

  it('initializes with undefined activeModel and fallback activeModelName', () => {
    const client = makeMockClient();
    const controller = new SessionController(client);
    expect(controller.activeModel).toBeUndefined();
    expect(controller.activeModelName).toBe('brain-default');
    const snapshot = controller.getSnapshot();
    expect(snapshot.activeModel).toBeUndefined();
    expect(snapshot.activeModelName).toBe('brain-default');
  });

  it('lists available models from authoritative client', async () => {
    const client = makeMockClient();
    const controller = new SessionController(client);
    const models = await controller.listAvailableModels();
    expect(models).toHaveLength(3);
    expect(models[0]!.id).toBe('brain-default');
    expect(models[1]!.id).toBe('claude-3-5-sonnet-20241022');
  });

  it('switches active model by query, updates snapshot, and emits confirmation notice', async () => {
    const client = makeMockClient();
    const controller = new SessionController(client);
    const resolved = await controller.switchModel('sonnet');
    expect(resolved).toBeDefined();
    expect(resolved!.id).toBe('claude-3-5-sonnet-20241022');
    expect(resolved!.name).toBe('Claude 3.5 Sonnet');

    expect(controller.activeModel).toEqual(resolved);
    expect(controller.activeModelName).toBe('Claude 3.5 Sonnet');

    const snapshot = controller.getSnapshot();
    expect(snapshot.activeModel).toEqual(resolved);
    expect(snapshot.activeModelName).toBe('Claude 3.5 Sonnet');

    // Verify confirmation notice in rows
    const noticeRow = snapshot.rows.find((r) =>
      r.kind === 'system' && r.text.includes('Switched active model to “Claude 3.5 Sonnet”'),
    );
    expect(noticeRow).toBeDefined();
  });

  it('leaves active model unchanged when resolution fails and records error notice', async () => {
    const client = makeMockClient();
    const controller = new SessionController(client);

    // Set an initial model
    await controller.switchModel('sonnet');
    expect(controller.activeModel?.id).toBe('claude-3-5-sonnet-20241022');

    // Attempt invalid model
    const failed = await controller.switchModel('nonexistent-model-xyz');
    expect(failed).toBeUndefined();

    // Verify previous model remains intact
    expect(controller.activeModel?.id).toBe('claude-3-5-sonnet-20241022');
    expect(controller.activeModelName).toBe('Claude 3.5 Sonnet');

    // Verify error notice
    const snapshot = controller.getSnapshot();
    const errRow = snapshot.rows.find((r) =>
      r.kind === 'system' && r.text.includes('Could not resolve model: Model \'nonexistent-model-xyz\' not found'),
    );
    expect(errRow).toBeDefined();
  });

  it('forwards active model id in BrainGenerationRequest during submit', async () => {
    let capturedReq: any = null;
    const client = makeMockClient({
      streamText: mock((req) => {
        capturedReq = req;
        return (async function* () {})() as any;
      }),
    });
    const controller = new SessionController(client);

    // Switch to GPT-4o
    await controller.switchModel('openai');
    expect(controller.activeModel?.id).toBe('gpt-4o-2024-08-06');

    // Submit prompt
    await controller.submit('Explain quantum topological states');

    expect(capturedReq).toBeDefined();
    expect(capturedReq.model).toBe('gpt-4o-2024-08-06');
  });

  it('forwards undefined model in BrainGenerationRequest when no active model set', async () => {
    let capturedReq: any = null;
    const client = makeMockClient({
      streamText: mock((req) => {
        capturedReq = req;
        return (async function* () {})() as any;
      }),
    });
    const controller = new SessionController(client);

    // Submit without switching
    await controller.submit('Default prompt');

    expect(capturedReq).toBeDefined();
    expect(capturedReq.model).toBeUndefined();
  });
});
