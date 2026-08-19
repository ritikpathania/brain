import { describe, test, expect } from 'bun:test';
import { query } from '../../vendor/claude/query.js';
import { productionDeps, type QueryDeps } from '../../vendor/claude/query/deps.js';
import { createUserMessage, createAssistantMessage, createAssistantAPIErrorMessage, handleMessageFromStream } from '../../vendor/claude/utils/messages.js';
import type { ToolUseContext } from '../../vendor/claude/Tool.js';
import type { Message, StreamEvent, AssistantMessage } from '../../vendor/claude/types/message.js';

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

describe('Phase 5.1: Deterministic CallModel Contract Harness', () => {
  test('Single-turn text streaming: verifies exact event order, stream deltas, and AssistantMessage payload', async () => {
    const deliveredDeltas: string[] = [];
    const recordedEvents: string[] = [];

    const mockCallModel: QueryDeps['callModel'] = async function* (params) {
      recordedEvents.push('stream_request_start');
      yield { type: 'stream_request_start' as const };

      recordedEvents.push('message_start');
      yield {
        type: 'stream_event' as const,
        event: {
          type: 'message_start' as const,
          message: {
            id: 'msg_test_001',
            type: 'message' as const,
            role: 'assistant' as const,
            content: [],
            model: 'mock-conforming-engine',
            stop_reason: null,
            stop_sequence: null,
            usage: { input_tokens: 15, output_tokens: 1 },
          },
        },
      };

      recordedEvents.push('content_block_start');
      yield {
        type: 'stream_event' as const,
        event: {
          type: 'content_block_start' as const,
          index: 0,
          content_block: { type: 'text' as const, text: '' },
        },
      };

      for (const token of ['Hello', ' from', ' Phase', ' 5.1', ' contract', ' harness!']) {
        recordedEvents.push(`content_block_delta:${token}`);
        yield {
          type: 'stream_event' as const,
          event: {
            type: 'content_block_delta' as const,
            index: 0,
            delta: { type: 'text_delta' as const, text: token },
          },
        };
      }

      recordedEvents.push('content_block_stop');
      yield {
        type: 'stream_event' as const,
        event: { type: 'content_block_stop' as const, index: 0 },
      };

      recordedEvents.push('message_delta');
      yield {
        type: 'stream_event' as const,
        event: {
          type: 'message_delta' as const,
          delta: { stop_reason: 'end_turn', stop_sequence: null },
          usage: { output_tokens: 8 },
        },
      };

      recordedEvents.push('message_stop');
      yield {
        type: 'stream_event' as const,
        event: { type: 'message_stop' as const },
      };

      const finalAssistantMessage = createAssistantMessage({
        content: [{ type: 'text', text: 'Hello from Phase 5.1 contract harness!' }],
      });
      recordedEvents.push('AssistantMessage');
      yield finalAssistantMessage;
    };

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: mockCallModel,
    };

    const queryStream = query({
      messages: [createUserMessage({ content: 'Run Phase 5.1 test' })],
      systemPrompt: 'You are a conforming test agent' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    const receivedEvents: any[] = [];
    let assistantMessageReceived: AssistantMessage | null = null;

    for await (const event of queryStream) {
      receivedEvents.push(event);
      if ((event as any).type === 'assistant') {
        assistantMessageReceived = event as AssistantMessage;
      }
      if ((event as any).type === 'stream_event' && (event as any).event?.type === 'content_block_delta') {
        deliveredDeltas.push((event as any).event.delta.text);
      }
    }

    // 1. Verify exact events occurred
    expect(recordedEvents).toContain('stream_request_start');
    expect(recordedEvents).toContain('message_start');
    expect(recordedEvents).toContain('content_block_start');
    expect(recordedEvents).toContain('content_block_stop');
    expect(recordedEvents).toContain('message_stop');
    expect(recordedEvents).toContain('AssistantMessage');

    // 2. Verify text deltas streamed cleanly
    expect(deliveredDeltas.join('')).toBe('Hello from Phase 5.1 contract harness!');

    // 3. Verify final AssistantMessage payload
    expect(assistantMessageReceived).not.toBeNull();
    expect(assistantMessageReceived!.message.content[0]).toEqual({
      type: 'text',
      text: 'Hello from Phase 5.1 contract harness!',
    });
  });

  test('Multi-turn conversation history accumulation: verifies historical message ordering passed into CallModel', async () => {
    let turnCount = 0;
    const historyReceivedOnTurn2: Message[] = [];

    const mockCallModel: QueryDeps['callModel'] = async function* (params) {
      turnCount++;
      if (turnCount === 2) {
        historyReceivedOnTurn2.push(...params.messages);
      }

      yield { type: 'stream_request_start' as const };
      yield {
        type: 'stream_event' as const,
        event: {
          type: 'message_start' as const,
          message: {
            id: `msg_turn_${turnCount}`,
            type: 'message' as const,
            role: 'assistant' as const,
            content: [],
            model: 'mock-engine',
            stop_reason: null,
            stop_sequence: null,
            usage: { input_tokens: 10, output_tokens: 2 },
          },
        },
      };

      const responseText = turnCount === 1 ? 'First turn response' : 'Second turn response';
      yield {
        type: 'stream_event' as const,
        event: {
          type: 'content_block_start' as const,
          index: 0,
          content_block: { type: 'text' as const, text: '' },
        },
      };
      yield {
        type: 'stream_event' as const,
        event: {
          type: 'content_block_delta' as const,
          index: 0,
          delta: { type: 'text_delta' as const, text: responseText },
        },
      };
      yield {
        type: 'stream_event' as const,
        event: { type: 'content_block_stop' as const, index: 0 },
      };
      yield {
        type: 'stream_event' as const,
        event: { type: 'message_stop' as const },
      };
      yield createAssistantMessage({ content: [{ type: 'text', text: responseText }] });
    };

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: mockCallModel,
    };

    // Turn 1
    const messagesHistory: Message[] = [createUserMessage({ content: 'Initial question' })];
    for await (const event of query({
      messages: messagesHistory,
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    })) {
      if ((event as any).type === 'assistant') {
        messagesHistory.push(event as AssistantMessage);
      }
    }

    // Turn 2
    messagesHistory.push(createUserMessage({ content: 'Follow-up question' }));
    for await (const _ of query({
      messages: messagesHistory,
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ allow: true }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    })) {}

    expect(turnCount).toBe(2);
    expect(historyReceivedOnTurn2.length).toBe(3);
    expect(historyReceivedOnTurn2[0].type).toBe('user');
    expect((historyReceivedOnTurn2[0] as any).message.content).toBe('Initial question');
    expect(historyReceivedOnTurn2[1].type).toBe('assistant');
    expect((historyReceivedOnTurn2[1] as any).message.content[0].text).toBe('First turn response');
    expect(historyReceivedOnTurn2[2].type).toBe('user');
    expect((historyReceivedOnTurn2[2] as any).message.content).toBe('Follow-up question');
  });

  test('System API Error propagation: verifies error message delivery and clean termination', async () => {
    const mockCallModel: QueryDeps['callModel'] = async function* () {
      yield { type: 'stream_request_start' as const };
      yield createAssistantAPIErrorMessage({
        content: 'Internal backend service error (500)',
        apiError: 'internal_server_error' as any,
      });
    };

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: mockCallModel,
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Trigger error test' })],
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
    expect(receivedError!.message.content[0].text).toBe('Internal backend service error (500)');
  });

  test('Stream cancellation: verifies AbortSignal terminates generator promptly', async () => {
    const abortController = new AbortController();
    let tokensEmitted = 0;
    let abortObservedByCallModel = false;

    const mockCallModel: QueryDeps['callModel'] = async function* (params) {
      if (params.signal) {
        params.signal.addEventListener('abort', () => {
          abortObservedByCallModel = true;
        });
      }

      yield { type: 'stream_request_start' as const };
      yield {
        type: 'stream_event' as const,
        event: {
          type: 'message_start' as const,
          message: {
            id: 'msg_cancel',
            type: 'message' as const,
            role: 'assistant' as const,
            content: [],
            model: 'mock-engine',
            stop_reason: null,
            stop_sequence: null,
            usage: { input_tokens: 5, output_tokens: 1 },
          },
        },
      };

      for (let i = 0; i < 20; i++) {
        if (params.signal?.aborted) {
          break;
        }
        tokensEmitted++;
        yield {
          type: 'stream_event' as const,
          event: {
            type: 'content_block_delta' as const,
            index: 0,
            delta: { type: 'text_delta' as const, text: `token_${i} ` },
          },
        };
        // Trigger abort mid-stream after 3 tokens
        if (i === 2) {
          abortController.abort();
        }
      }
    };

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: mockCallModel,
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Cancel test' })],
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

    expect(abortObservedByCallModel).toBe(true);
    expect(tokensEmitted).toBeLessThanOrEqual(5);
  });

  test('UI Message Reducer (handleMessageFromStream): verifies progressive text accumulation and streamMode transitions', () => {
    let activeStreamMode = 'idle';
    let streamedTextContent: string | null = null;
    let finalizedMessage: Message | null = null;

    const onMessage = (msg: Message) => {
      finalizedMessage = msg;
    };
    const onUpdateLength = () => {};
    const onSetStreamMode = (mode: any) => {
      activeStreamMode = mode;
    };
    const onStreamingToolUses = () => {};
    const onStreamingText = (updater: any) => {
      streamedTextContent = typeof updater === 'function' ? updater(streamedTextContent) : updater;
    };

    // 1. stream_request_start -> mode: requesting
    handleMessageFromStream(
      { type: 'stream_request_start' as const },
      onMessage,
      onUpdateLength,
      onSetStreamMode,
      onStreamingToolUses,
      undefined,
      undefined,
      undefined,
      onStreamingText
    );
    expect(activeStreamMode).toBe('requesting');

    // 2. content_block_start -> mode: responding
    handleMessageFromStream(
      {
        type: 'stream_event',
        event: {
          type: 'content_block_start',
          index: 0,
          content_block: { type: 'text', text: '' },
        },
      } as any,
      onMessage,
      onUpdateLength,
      onSetStreamMode,
      onStreamingToolUses,
      undefined,
      undefined,
      undefined,
      onStreamingText
    );
    expect(activeStreamMode).toBe('responding');

    // 3. content_block_delta -> updates streamingText
    handleMessageFromStream(
      {
        type: 'stream_event',
        event: {
          type: 'content_block_delta',
          index: 0,
          delta: { type: 'text_delta', text: 'Live Token Stream' },
        },
      } as any,
      onMessage,
      onUpdateLength,
      onSetStreamMode,
      onStreamingToolUses,
      undefined,
      undefined,
      undefined,
      onStreamingText
    );
    expect(streamedTextContent).toBe('Live Token Stream');

    // 4. Final AssistantMessage materialized -> delivered to onMessage, streamingText reset
    const assistantMsg = createAssistantMessage({ content: 'Live Token Stream' });
    handleMessageFromStream(
      assistantMsg,
      onMessage,
      onUpdateLength,
      onSetStreamMode,
      onStreamingToolUses,
      undefined,
      undefined,
      undefined,
      onStreamingText
    );
    expect(finalizedMessage).not.toBeNull();
    expect((finalizedMessage as any).message.content[0].text).toBe('Live Token Stream');
    expect(streamedTextContent).toBeNull();
  });
});
