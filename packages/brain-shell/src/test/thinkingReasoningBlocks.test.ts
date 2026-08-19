import { describe, test, expect } from 'bun:test';
import { query } from '../../vendor/claude/query.js';
import { productionDeps, type QueryDeps } from '../../vendor/claude/query/deps.js';
import { createUserMessage, handleMessageFromStream } from '../../vendor/claude/utils/messages.js';
import type { Tool, ToolUseContext } from '../../vendor/claude/Tool.js';
import type { Message, AssistantMessage } from '../../vendor/claude/types/message.js';
import { MockBrainBackendClient, type BrainGenerationRequest } from '../client/BrainBackendClient.js';
import { createBrainCallModel } from '../adapter/brainCallModel.js';

function createMockToolUseContext(
  abortController: AbortController = new AbortController(),
  extraTools: Tool[] = [],
  thinkingMode: any = { mode: 'adaptive' }
): ToolUseContext {
  const appState: any = {
    toolPermissionContext: { additionalWorkingDirectories: new Map(), alwaysAllowRules: {} },
    mcp: { clients: [], tools: [] },
    sessionHooks: new Map(),
    fastMode: false,
    effortValue: 'high',
  };
  return {
    abortController,
    agentId: 'thinking_test_agent' as any,
    readFileState: { get: () => null, set: () => {}, has: () => false, delete: () => {} } as any,
    options: {
      tools: extraTools,
      mcpClients: [],
      mainLoopModel: 'claude-3-7-sonnet-20250219',
      thinkingConfig: thinkingMode,
      agentDefinitions: { activeAgents: [], allowedAgentTypes: [] },
    },
    getAppState: () => appState,
    setAppState: () => {},
    setInProgressToolUseIDs: () => {},
    setResponseLength: () => {},
    updateFileHistoryState: () => {},
    addNotification: () => {},
  } as any;
}

function createDummyTool(name: string, executeResult: (input: any) => Promise<any> = async () => 'OK'): Tool {
  return {
    name,
    description: `Dummy test tool for ${name}`,
    inputSchema: {
      safeParse: (input: any) => ({ success: true, data: input }),
      parse: (input: any) => input,
    } as any,
    call: async (input: any) => ({ data: await executeResult(input) }),
    mapToolResultToToolResultBlockParam: (data: any, toolUseID: string) => ({
      type: 'tool_result',
      tool_use_id: toolUseID,
      content: typeof data === 'string' ? data : JSON.stringify(data),
    }),
    needsPermissions: () => false,
  } as any;
}

describe('Phase 5.5: Thinking & Reasoning Blocks Integration Matrix', () => {
  test('Scenario 1: thinking_delta streams produce native thinking content blocks in Claude runtime', async () => {
    const brainClient = new MockBrainBackendClient(async function* () {
      yield { type: 'thinking', thinking: 'Analyzing the problem deeply...' };
      yield { type: 'token', token: 'Here is the direct answer.' };
      yield { type: 'finished' };
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Complex logic puzzle' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    let finalAssistantMsg: AssistantMessage | null = null;
    const thinkingDeltas: string[] = [];

    for await (const event of stream) {
      if (
        (event as any).type === 'stream_event' &&
        (event as any).event?.type === 'content_block_delta' &&
        (event as any).event.delta?.type === 'thinking_delta'
      ) {
        thinkingDeltas.push((event as any).event.delta.thinking);
      }
      if ((event as any).type === 'assistant') {
        finalAssistantMsg = event as AssistantMessage;
      }
    }

    expect(thinkingDeltas).toEqual(['Analyzing the problem deeply...']);
    expect(finalAssistantMsg).not.toBeNull();
    const blocks = finalAssistantMsg!.message.content;
    expect(blocks.length).toBe(2);
    expect(blocks[0].type).toBe('thinking');
    expect((blocks[0] as any).thinking).toBe('Analyzing the problem deeply...');
    expect(blocks[1].type).toBe('text');
    expect((blocks[1] as any).text).toBe('Here is the direct answer.');
  });

  test('Scenario 2: Thinking -> Text strict ordering invariant is preserved', async () => {
    const brainClient = new MockBrainBackendClient(async function* () {
      yield { type: 'thinking', thinking: 'Step 1: Compute invariant.' };
      yield { type: 'token', token: 'Step 2: Output result.' };
      yield { type: 'finished' };
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Order test' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    const blockTypeSequence: string[] = [];
    let finalAssistantMsg: AssistantMessage | null = null;

    for await (const event of stream) {
      if ((event as any).type === 'stream_event' && (event as any).event?.type === 'content_block_start') {
        blockTypeSequence.push((event as any).event.content_block.type);
      }
      if ((event as any).type === 'assistant') {
        finalAssistantMsg = event as AssistantMessage;
      }
    }

    expect(blockTypeSequence).toEqual(['thinking', 'text']);
    expect(finalAssistantMsg!.message.content[0].type).toBe('thinking');
    expect(finalAssistantMsg!.message.content[1].type).toBe('text');
  });

  test('Scenario 3: Multiple thinking deltas accumulate progressively into a single thinking block', async () => {
    const brainClient = new MockBrainBackendClient(async function* () {
      yield { type: 'thinking', thinking: 'Chunk 1: Initial thought. ' };
      yield { type: 'thinking', thinking: 'Chunk 2: Deeper reasoning. ' };
      yield { type: 'thinking', thinking: 'Chunk 3: Synthesis complete.' };
      yield { type: 'token', token: 'Done.' };
      yield { type: 'finished' };
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Accumulate thinking test' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    let finalAssistantMsg: AssistantMessage | null = null;
    for await (const event of stream) {
      if ((event as any).type === 'assistant') {
        finalAssistantMsg = event as AssistantMessage;
      }
    }

    const thinkingBlock: any = finalAssistantMsg!.message.content[0];
    expect(thinkingBlock.type).toBe('thinking');
    expect(thinkingBlock.thinking).toBe('Chunk 1: Initial thought. Chunk 2: Deeper reasoning. Chunk 3: Synthesis complete.');
  });

  test('Scenario 4: Thinking-only response without trailing text completes deterministically', async () => {
    const brainClient = new MockBrainBackendClient(async function* () {
      yield { type: 'thinking', thinking: 'Pure internal contemplation.' };
      yield { type: 'finished' };
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Ponder only' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    let finalAssistantMsg: AssistantMessage | null = null;
    for await (const event of stream) {
      if ((event as any).type === 'assistant') {
        finalAssistantMsg = event as AssistantMessage;
      }
    }

    expect(finalAssistantMsg).not.toBeNull();
    expect(finalAssistantMsg!.message.content.length).toBe(1);
    expect(finalAssistantMsg!.message.content[0].type).toBe('thinking');
    expect((finalAssistantMsg!.message.content[0] as any).thinking).toBe('Pure internal contemplation.');
  });

  test('Scenario 5: Thinking + Tool Call (thinking -> tool_use) within a single turn', async () => {
    const tool = createDummyTool('LookupTool', async () => 'Found 100 items');
    let callCount = 0;

    const brainClient = new MockBrainBackendClient(async function* () {
      callCount++;
      if (callCount === 1) {
        yield { type: 'thinking', thinking: 'I need to check database records first.' };
        yield {
          type: 'tool_use',
          toolUse: { id: 'call_lookup_1', name: 'LookupTool', input: { query: 'all' } },
        };
        yield { type: 'finished' };
      } else {
        yield { type: 'token', token: 'Records retrieved successfully.' };
        yield { type: 'finished' };
      }
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Find records' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(new AbortController(), [tool]),
      querySource: 'repl',
      deps,
    });

    let assistantTurn1Msg: AssistantMessage | null = null;
    for await (const event of stream) {
      if ((event as any).type === 'assistant' && !assistantTurn1Msg) {
        assistantTurn1Msg = event as AssistantMessage;
      }
    }

    expect(assistantTurn1Msg).not.toBeNull();
    const blocks = assistantTurn1Msg!.message.content;
    expect(blocks.length).toBe(2);
    expect(blocks[0].type).toBe('thinking');
    expect((blocks[0] as any).thinking).toBe('I need to check database records first.');
    expect(blocks[1].type).toBe('tool_use');
    expect((blocks[1] as any).name).toBe('LookupTool');
  });

  test('Scenario 6: Thinking + Tool Result + Post-Tool Thinking + Final Text round-trip', async () => {
    const tool = createDummyTool('ComputeStats', async () => 'Mean: 42.0');
    let callCount = 0;

    const brainClient = new MockBrainBackendClient(async function* () {
      callCount++;
      if (callCount === 1) {
        yield { type: 'thinking', thinking: 'I will call ComputeStats.' };
        yield {
          type: 'tool_use',
          toolUse: { id: 'call_stats_1', name: 'ComputeStats', input: {} },
        };
        yield { type: 'finished' };
      } else {
        yield { type: 'thinking', thinking: 'The tool returned Mean: 42.0. Now formatting final answer.' };
        yield { type: 'token', token: 'The average calculated is 42.0.' };
        yield { type: 'finished' };
      }
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Calculate average' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(new AbortController(), [tool]),
      querySource: 'repl',
      deps,
    });

    let finalMsg: AssistantMessage | null = null;
    for await (const event of stream) {
      if ((event as any).type === 'assistant') {
        finalMsg = event as AssistantMessage;
      }
    }

    expect(callCount).toBe(2);
    expect(finalMsg).not.toBeNull();
    expect(finalMsg!.message.content[0].type).toBe('thinking');
    expect((finalMsg!.message.content[0] as any).thinking).toContain('The tool returned Mean: 42.0');
    expect(finalMsg!.message.content[1].type).toBe('text');
    expect((finalMsg!.message.content[1] as any).text).toBe('The average calculated is 42.0.');
  });

  test('Scenario 7: Redacted thinking is handled without fabricating cryptographic signatures', async () => {
    const brainClient = new MockBrainBackendClient(async function* () {
      yield { type: 'redacted_thinking', redactedData: 'encrypted_opaque_payload_xyz' };
      yield { type: 'token', token: 'Public explanation.' };
      yield { type: 'finished' };
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Redacted test' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    let finalAssistantMsg: AssistantMessage | null = null;
    for await (const event of stream) {
      if ((event as any).type === 'assistant') {
        finalAssistantMsg = event as AssistantMessage;
      }
    }

    expect(finalAssistantMsg).not.toBeNull();
    const blocks = finalAssistantMsg!.message.content;
    expect(blocks[0].type).toBe('redacted_thinking');
    expect((blocks[0] as any).data).toBe('encrypted_opaque_payload_xyz');
    expect(blocks[1].type).toBe('text');
  });

  test('Scenario 8: Thinking disabled in ToolUseContext delivers mode: disabled to Brain', async () => {
    let receivedThinkingConfig: any = null;

    const brainClient = new MockBrainBackendClient(async function* (request) {
      receivedThinkingConfig = request.thinkingConfig;
      yield { type: 'token', token: 'Thinking was off.' };
      yield { type: 'finished' };
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'No thinking please' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(new AbortController(), [], { mode: 'off' }),
      querySource: 'repl',
      deps,
    });

    for await (const _ of stream) {}

    expect(receivedThinkingConfig).toEqual({ mode: 'disabled' });
  });

  test('Scenario 9: Custom budgetTokens values reach Brain client in request.thinkingConfig', async () => {
    let receivedThinkingConfig: any = null;

    const brainClient = new MockBrainBackendClient(async function* (request) {
      receivedThinkingConfig = request.thinkingConfig;
      yield { type: 'token', token: 'Budget recognized.' };
      yield { type: 'finished' };
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Thinking with budget' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(new AbortController(), [], { type: 'enabled', budgetTokens: 8192 }),
      querySource: 'repl',
      deps,
    });

    for await (const _ of stream) {}

    expect(receivedThinkingConfig).toEqual({ mode: 'enabled', budgetTokens: 8192 });
  });

  test('Scenario 10: Cancellation during thinking leaves no corrupted state', async () => {
    const ac = new AbortController();
    let tokensEmitted = 0;

    const brainClient = new MockBrainBackendClient(async function* (request) {
      for (let i = 0; i < 20; i++) {
        if (request.signal?.aborted) break;
        tokensEmitted++;
        yield { type: 'thinking', thinking: `Reasoning step ${i}... ` };
        if (i === 2) {
          ac.abort();
        }
      }
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Cancel thinking' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(ac),
      querySource: 'repl',
      deps,
    });

    for await (const _ of stream) {
      if (ac.signal.aborted) break;
    }

    expect(ac.signal.aborted).toBe(true);
    expect(tokensEmitted).toBeLessThanOrEqual(4);
  });

  test('Scenario 11: Multi-turn thinking history is normalized into BrainChatMessage[] accurately', async () => {
    let capturedTurn2Messages: any[] = [];

    const brainClient = new MockBrainBackendClient(async function* (request) {
      if (request.messages.length > 1) {
        capturedTurn2Messages = request.messages;
      }
      yield { type: 'thinking', thinking: 'Turn reasoning' };
      yield { type: 'token', token: 'Turn response' };
      yield { type: 'finished' };
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const conversation: Message[] = [createUserMessage({ content: 'Turn 1 prompt' })];

    // Turn 1
    for await (const event of query({
      messages: conversation,
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    })) {
      if ((event as any).type === 'assistant') {
        conversation.push(event as AssistantMessage);
      }
    }

    // Turn 2
    conversation.push(createUserMessage({ content: 'Turn 2 prompt' }));
    for await (const _ of query({
      messages: conversation,
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    })) {}

    expect(capturedTurn2Messages.length).toBe(3);
    expect(capturedTurn2Messages[0]).toEqual({ role: 'user', content: 'Turn 1 prompt' });
    const turn1Asst = capturedTurn2Messages[1];
    expect(turn1Asst.role).toBe('assistant');
    expect(Array.isArray(turn1Asst.content)).toBe(true);
    expect(turn1Asst.content[0]).toEqual({ type: 'thinking', thinking: 'Turn reasoning', signature: '' });
    expect(turn1Asst.content[1]).toEqual({ type: 'text', text: 'Turn response' });
    expect(capturedTurn2Messages[2]).toEqual({ role: 'user', content: 'Turn 2 prompt' });
  });

  test('Scenario 12: handleMessageFromStream reducer integration with thinking streams', () => {
    let streamedThinkingUpdates = 0;
    let lengthUpdates = 0;

    const onMessage = () => {};
    const onUpdateLength = (val: string) => {
      lengthUpdates++;
    };
    const onSetStreamMode = () => {};
    const onStreamingToolUses = () => {};
    const onStreamingThinking = (updater: any) => {
      streamedThinkingUpdates++;
    };

    // Feed thinking delta through handleMessageFromStream
    handleMessageFromStream(
      {
        type: 'stream_event',
        event: {
          type: 'content_block_delta',
          index: 0,
          delta: { type: 'thinking_delta', thinking: 'Thinking step' },
        },
      } as any,
      onMessage,
      onUpdateLength,
      onSetStreamMode,
      onStreamingToolUses,
      undefined,
      onStreamingThinking
    );

    expect(lengthUpdates).toBe(1);
  });
});
