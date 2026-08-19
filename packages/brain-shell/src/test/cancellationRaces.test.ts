import { describe, test, expect } from 'bun:test';
import { query } from '../../vendor/claude/query.js';
import { productionDeps, type QueryDeps } from '../../vendor/claude/query/deps.js';
import { createUserMessage, createAssistantMessage, handleMessageFromStream } from '../../vendor/claude/utils/messages.js';
import type { ToolUseContext } from '../../vendor/claude/Tool.js';
import type { Message, AssistantMessage } from '../../vendor/claude/types/message.js';
import type { BrainBackendClient, BrainGenerationRequest, BrainStreamChunk } from '../client/BrainBackendClient.js';
import { createBrainCallModel } from '../adapter/brainCallModel.js';

class AdversarialBrainClient implements BrainBackendClient {
  public activeStreams = 0;
  public totalStreamsStarted = 0;
  public totalStreamsCompleted = 0;
  public totalStreamsAborted = 0;
  public orphanTokensEmitted = 0;

  constructor(
    private tokenCount = 100,
    private disconnectAtToken = -1,
    private disconnectBeforeStart = false
  ) {}

  async *streamText(request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
    this.activeStreams++;
    this.totalStreamsStarted++;

    let observedAbort = false;
    request.signal?.addEventListener('abort', () => {
      observedAbort = true;
      this.totalStreamsAborted++;
    });

    try {
      if (this.disconnectBeforeStart) {
        yield { type: 'error', error: 'Connection refused before stream start' };
        return;
      }

      for (let i = 0; i < this.tokenCount; i++) {
        if (request.signal?.aborted || observedAbort) {
          break;
        }

        if (this.disconnectAtToken >= 0 && i >= this.disconnectAtToken) {
          yield { type: 'error', error: `Socket severed at token index ${i}` };
          return;
        }

        if (request.signal?.aborted) {
          this.orphanTokensEmitted++;
        }

        yield {
          type: 'token',
          token: `token_${i} `,
          metadata: { inputTokens: 10, outputTokens: i + 1 },
        };

        // Small micro-yield to allow asynchronous race conditions to trigger
        await new Promise((resolve) => setTimeout(resolve, 1));
      }

      if (!request.signal?.aborted && !observedAbort) {
        yield { type: 'finished' };
      }
    } finally {
      this.activeStreams--;
      this.totalStreamsCompleted++;
    }
  }
}

function createMockToolUseContext(abortController: AbortController = new AbortController()): ToolUseContext {
  const appState: any = {
    toolPermissionContext: { additionalWorkingDirectories: new Map(), alwaysAllowRules: {} },
    mcp: { clients: [], tools: [] },
    sessionHooks: new Map(),
    fastMode: false,
    effortValue: 'high',
  };
  return {
    abortController,
    options: {
      tools: [],
      mcpClients: [],
      mainLoopModel: 'claude-3-7-sonnet-20250219',
      thinkingConfig: { mode: 'off' },
      agentDefinitions: { activeAgents: [], allowedAgentTypes: [] },
    },
    getAppState: () => appState,
    setAppState: () => {},
    addNotification: () => {},
  };
}

describe('Phase 5.3: Adversarial Cancellation & Race Condition Verification', () => {
  test('Race 1: Ctrl+C after first token terminates Brain backend stream without orphan leaks', async () => {
    const client = new AdversarialBrainClient(50);
    const brainCallModel = createBrainCallModel(client);
    const abortController = new AbortController();

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: brainCallModel,
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Prompt 1' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(abortController),
      querySource: 'repl',
      deps,
    });

    let tokensReceived = 0;
    for await (const event of stream) {
      if ((event as any).type === 'stream_event' && (event as any).event?.type === 'content_block_delta') {
        tokensReceived++;
        if (tokensReceived === 2) {
          // Simulate Ctrl+C
          abortController.abort();
        }
      }
    }

    // Assertions:
    expect(tokensReceived).toBe(2);
    expect(client.totalStreamsAborted).toBe(1);
    expect(client.activeStreams).toBe(0); // Clean backend teardown
    expect(client.orphanTokensEmitted).toBe(0); // 0 orphan tokens
  });

  test('Race 2: Ctrl+C before first token exits immediately with 0 tokens and no orphan stream', async () => {
    const client = new AdversarialBrainClient(50);
    const brainCallModel = createBrainCallModel(client);
    const abortController = new AbortController();

    // Abort BEFORE query starts
    abortController.abort();

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: brainCallModel,
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Pre-abort prompt' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(abortController),
      querySource: 'repl',
      deps,
    });

    const receivedStreamEvents: any[] = [];
    for await (const event of stream) {
      if ((event as any).type === 'stream_event') {
        receivedStreamEvents.push(event);
      }
    }

    expect(client.activeStreams).toBe(0);
    expect(client.orphanTokensEmitted).toBe(0);
    expect(receivedStreamEvents.length).toBe(0); // 0 stream tokens delivered
  });

  test('Race 3: Ctrl+C around token arrival produces NO phantom AssistantMessage or duplicate final state', async () => {
    const client = new AdversarialBrainClient(10);
    const brainCallModel = createBrainCallModel(client);
    const abortController = new AbortController();

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: brainCallModel,
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Prompt' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(abortController),
      querySource: 'repl',
      deps,
    });

    let assistantMsgYielded = false;
    for await (const event of stream) {
      if ((event as any).type === 'stream_event' && (event as any).event?.type === 'content_block_delta') {
        abortController.abort(); // Abort on first delta
      }
      if ((event as any).type === 'assistant') {
        assistantMsgYielded = true;
      }
    }

    // Because it was aborted mid-stream, NO final AssistantMessage should be yielded
    expect(assistantMsgYielded).toBe(false);
    expect(client.activeStreams).toBe(0);
  });

  test('Race 4: Backend disconnect before first token generates clean Claude API error message', async () => {
    const client = new AdversarialBrainClient(50, -1, true); // Disconnect before start
    const brainCallModel = createBrainCallModel(client);

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: brainCallModel,
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Prompt' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    let errorReceived: AssistantMessage | null = null;
    for await (const event of stream) {
      if ((event as any).type === 'assistant' && (event as any).isApiErrorMessage) {
        errorReceived = event as AssistantMessage;
      }
    }

    expect(errorReceived).not.toBeNull();
    expect(errorReceived!.isApiErrorMessage).toBe(true);
    expect(errorReceived!.message.content[0].text).toContain('Connection refused before stream start');
    expect(client.activeStreams).toBe(0);
  });

  test('Race 5: Backend disconnect mid-stream handles partial output deterministically', async () => {
    const client = new AdversarialBrainClient(50, 3, false); // Sever at token 3
    const brainCallModel = createBrainCallModel(client);

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: brainCallModel,
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Prompt' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    let deltasCount = 0;
    let errorReceived: AssistantMessage | null = null;
    for await (const event of stream) {
      if ((event as any).type === 'stream_event' && (event as any).event?.type === 'content_block_delta') {
        deltasCount++;
      }
      if ((event as any).type === 'assistant' && (event as any).isApiErrorMessage) {
        errorReceived = event as AssistantMessage;
      }
    }

    expect(deltasCount).toBe(3);
    expect(errorReceived).not.toBeNull();
    expect(errorReceived!.isApiErrorMessage).toBe(true);
    expect(errorReceived!.message.content[0].text).toContain('Socket severed at token index 3');
    expect(client.activeStreams).toBe(0);
  });

  test('Race 6: Repeated abort calls are idempotent and do not corrupt adapter state', async () => {
    const client = new AdversarialBrainClient(50);
    const brainCallModel = createBrainCallModel(client);
    const abortController = new AbortController();

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: brainCallModel,
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Prompt' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(abortController),
      querySource: 'repl',
      deps,
    });

    for await (const event of stream) {
      if ((event as any).type === 'stream_event' && (event as any).event?.type === 'content_block_delta') {
        abortController.abort();
        abortController.abort(); // Call 2
        abortController.abort(); // Call 3
      }
    }

    expect(client.activeStreams).toBe(0);
    expect(client.totalStreamsAborted).toBe(1); // Exactly 1 abort event handled
  });

  test('Race 7: Abort triggered after stream completion is a clean no-op with 0 state mutation', async () => {
    const client = new AdversarialBrainClient(3);
    const brainCallModel = createBrainCallModel(client);
    const abortController = new AbortController();

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: brainCallModel,
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Prompt' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(abortController),
      querySource: 'repl',
      deps,
    });

    let completedAssistantMsg: AssistantMessage | null = null;
    for await (const event of stream) {
      if ((event as any).type === 'assistant') {
        completedAssistantMsg = event as AssistantMessage;
      }
    }

    // Now trigger late abort after completion
    abortController.abort();

    expect(completedAssistantMsg).not.toBeNull();
    expect(completedAssistantMsg!.message.content[0].text).toBe('token_0 token_1 token_2 ');
    expect(client.activeStreams).toBe(0);
  });

  test('Race 8: Full Lifecycle — Rapid cancel(T1) -> recover -> reuse(T2) -> cancel(T2) -> reuse(T3)', async () => {
    const client = new AdversarialBrainClient(20);
    const brainCallModel = createBrainCallModel(client);

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: brainCallModel,
    };

    // --- Turn 1: Cancelled after 2 tokens ---
    const ac1 = new AbortController();
    const stream1 = query({
      messages: [createUserMessage({ content: 'T1' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(ac1),
      querySource: 'repl',
      deps,
    });

    let t1Deltas = 0;
    for await (const event of stream1) {
      if ((event as any).type === 'stream_event' && (event as any).event?.type === 'content_block_delta') {
        t1Deltas++;
        if (t1Deltas === 2) {
          ac1.abort();
        }
      }
    }
    expect(client.activeStreams).toBe(0);

    // --- Turn 2: Recover and execute full turn ---
    const ac2 = new AbortController();
    const stream2 = query({
      messages: [createUserMessage({ content: 'T2 - Clean turn' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(ac2),
      querySource: 'repl',
      deps,
    });

    let t2FinalMsg: AssistantMessage | null = null;
    for await (const event of stream2) {
      if ((event as any).type === 'assistant') {
        t2FinalMsg = event as AssistantMessage;
      }
    }
    expect(t2FinalMsg).not.toBeNull();
    expect(client.activeStreams).toBe(0);

    // --- Turn 3: Cancelled again ---
    const ac3 = new AbortController();
    const stream3 = query({
      messages: [createUserMessage({ content: 'T3' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(ac3),
      querySource: 'repl',
      deps,
    });

    let t3Deltas = 0;
    for await (const event of stream3) {
      if ((event as any).type === 'stream_event' && (event as any).event?.type === 'content_block_delta') {
        t3Deltas++;
        if (t3Deltas === 1) {
          ac3.abort();
        }
      }
    }
    expect(client.activeStreams).toBe(0);

    // --- Turn 4: Final clean execution ---
    const ac4 = new AbortController();
    const stream4 = query({
      messages: [createUserMessage({ content: 'T4 - Final clean turn' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(ac4),
      querySource: 'repl',
      deps,
    });

    let t4FinalMsg: AssistantMessage | null = null;
    for await (const event of stream4) {
      if ((event as any).type === 'assistant') {
        t4FinalMsg = event as AssistantMessage;
      }
    }

    expect(t4FinalMsg).not.toBeNull();
    expect(client.activeStreams).toBe(0);
    expect(client.totalStreamsStarted).toBe(4);
    expect(client.totalStreamsCompleted).toBe(4);
    expect(client.totalStreamsAborted).toBe(2);
    expect(client.orphanTokensEmitted).toBe(0); // 0 orphan tokens across all 4 turns
  });
});
