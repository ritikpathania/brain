import { describe, test, expect } from 'bun:test';
import { query } from '../../vendor/claude/query.js';
import { productionDeps, type QueryDeps } from '../../vendor/claude/query/deps.js';
import { createUserMessage, handleMessageFromStream } from '../../vendor/claude/utils/messages.js';
import type { ToolUseContext } from '../../vendor/claude/Tool.js';
import type { Message, AssistantMessage } from '../../vendor/claude/types/message.js';
import { MockBrainBackendClient, type BrainGenerationRequest } from '../client/BrainBackendClient.js';
import { createBrainCallModel } from '../adapter/brainCallModel.js';

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

describe('Phase 5.2: Brain Text Streaming Adapter', () => {
  test('Single-turn text stream: routes Brain tokens through query() into Claude AssistantMessage', async () => {
    const brainClient = new MockBrainBackendClient([
      'Brain',
      ' relational',
      ' memory',
      ' text',
      ' stream',
      ' operational.',
    ]);
    const brainCallModel = createBrainCallModel(brainClient);

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: brainCallModel,
    };

    const queryStream = query({
      messages: [createUserMessage({ content: 'Hello Brain' })],
      systemPrompt: 'You are Brain Claude shell.' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    const receivedTokens: string[] = [];
    let finalAssistantMsg: AssistantMessage | null = null;

    for await (const event of queryStream) {
      if ((event as any).type === 'assistant') {
        finalAssistantMsg = event as AssistantMessage;
      }
      if (
        (event as any).type === 'stream_event' &&
        (event as any).event?.type === 'content_block_delta' &&
        (event as any).event.delta?.type === 'text_delta'
      ) {
        receivedTokens.push((event as any).event.delta.text);
      }
    }

    expect(receivedTokens.join('')).toBe('Brain relational memory text stream operational.');
    expect(finalAssistantMsg).not.toBeNull();
    expect(finalAssistantMsg!.message.content[0]).toEqual({
      type: 'text',
      text: 'Brain relational memory text stream operational.',
    });
  });

  test('Multi-turn conversation history: normalizes Claude Message[] history for Brain backend client', async () => {
    let capturedRequest: BrainGenerationRequest | null = null;

    const brainClient = {
      async *streamText(request: BrainGenerationRequest) {
        capturedRequest = request;
        yield { type: 'token' as const, token: 'Acknowledged turn 2.' };
        yield { type: 'finished' as const };
      },
    };
    const brainCallModel = createBrainCallModel(brainClient);

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: brainCallModel,
    };

    // Prepare 2-turn history
    const messagesHistory: Message[] = [
      createUserMessage({ content: 'Turn 1 User Prompt' }),
      {
        type: 'assistant',
        uuid: 'turn_1_asst' as any,
        message: {
          id: 'msg_1',
          type: 'message',
          role: 'assistant',
          content: [{ type: 'text', text: 'Turn 1 Assistant Response' }],
          model: 'brain-engine-v1',
          stop_reason: 'end_turn',
          stop_sequence: null,
          usage: { input_tokens: 5, output_tokens: 5 } as any,
        },
      } as AssistantMessage,
      createUserMessage({ content: 'Turn 2 User Follow-up' }),
    ];

    for await (const _ of query({
      messages: messagesHistory,
      systemPrompt: 'Custom System Prompt' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    })) {}

    expect(capturedRequest).not.toBeNull();
    expect(capturedRequest!.messages.length).toBe(3);
    expect(capturedRequest!.messages[0]).toEqual({ role: 'user', content: 'Turn 1 User Prompt' });
    expect(capturedRequest!.messages[1]).toEqual({ role: 'assistant', content: 'Turn 1 Assistant Response' });
    expect(capturedRequest!.messages[2]).toEqual({ role: 'user', content: 'Turn 2 User Follow-up' });
    expect(capturedRequest!.systemPrompt).toBe('Custom System Prompt');
  });

  test('Stream cancellation: propagates AbortSignal to BrainBackendClient and terminates generator cleanly', async () => {
    const abortController = new AbortController();
    let abortedAtClient = false;
    let tokensEmitted = 0;

    const brainClient = {
      async *streamText(request: BrainGenerationRequest) {
        request.signal?.addEventListener('abort', () => {
          abortedAtClient = true;
        });

        for (let i = 0; i < 50; i++) {
          if (request.signal?.aborted) {
            break;
          }
          tokensEmitted++;
          yield { type: 'token' as const, token: `chunk_${i} ` };
          if (i === 2) {
            abortController.abort();
          }
        }
      },
    };

    const brainCallModel = createBrainCallModel(brainClient);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: brainCallModel,
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Abort test' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(abortController),
      querySource: 'repl',
      deps,
    });

    for await (const _ of stream) {
      if (abortController.signal.aborted) {
        break;
      }
    }

    expect(abortedAtClient).toBe(true);
    expect(tokensEmitted).toBeLessThanOrEqual(4);
  });

  test('Error normalization: transforms Brain backend error into Claude createAssistantAPIErrorMessage', async () => {
    const brainClient = new MockBrainBackendClient([], 'Brain daemon socket disconnected (ECONNREFUSED)');
    const brainCallModel = createBrainCallModel(brainClient);

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: brainCallModel,
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Trigger backend error' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    let receivedError: AssistantMessage | null = null;
    for await (const event of stream) {
      if ((event as any).type === 'assistant' && (event as any).isApiErrorMessage) {
        receivedError = event as AssistantMessage;
      }
    }

    expect(receivedError).not.toBeNull();
    expect(receivedError!.isApiErrorMessage).toBe(true);
    expect(receivedError!.message.content[0].text).toBe('Brain daemon socket disconnected (ECONNREFUSED)');
  });

  test('Downstream rendering integration: streams Brain tokens into Claude handleMessageFromStream reducer', async () => {
    let streamedContent: string | null = null;
    let finalMessageDelivered: Message | null = null;

    const onMessage = (msg: Message) => {
      finalMessageDelivered = msg;
    };
    const onUpdateLength = () => {};
    const onSetStreamMode = () => {};
    const onStreamingToolUses = () => {};
    const onStreamingText = (updater: any) => {
      streamedContent = typeof updater === 'function' ? updater(streamedContent) : updater;
    };

    const brainClient = new MockBrainBackendClient(['Hello', ' ', 'Brain!']);
    const brainCallModel = createBrainCallModel(brainClient);

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: brainCallModel,
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Test prompt' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    for await (const event of stream) {
      handleMessageFromStream(
        event as any,
        onMessage,
        onUpdateLength,
        onSetStreamMode,
        onStreamingToolUses,
        undefined,
        undefined,
        undefined,
        onStreamingText
      );
    }

    expect(finalMessageDelivered).not.toBeNull();
    expect((finalMessageDelivered as any).message.content[0].text).toBe('Hello Brain!');
    expect(streamedContent).toBeNull(); // Reset upon turn finalization
  });
});
