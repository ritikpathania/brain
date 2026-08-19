import { describe, test, expect } from 'bun:test';
import { query } from '../../vendor/claude/query.js';
import { productionDeps, type QueryDeps } from '../../vendor/claude/query/deps.js';
import { createUserMessage } from '../../vendor/claude/utils/messages.js';
import type { Tool, ToolUseContext } from '../../vendor/claude/Tool.js';
import type { AssistantMessage } from '../../vendor/claude/types/message.js';
import { MockBrainBackendClient, type BrainGenerationRequest } from '../client/BrainBackendClient.js';
import { createBrainCallModel } from '../adapter/brainCallModel.js';

function createMockToolUseContext(abortController: AbortController = new AbortController(), extraTools: Tool[] = []): ToolUseContext {
  const appState: any = {
    toolPermissionContext: { additionalWorkingDirectories: new Map(), alwaysAllowRules: {} },
    mcp: { clients: [], tools: [] },
    sessionHooks: new Map(),
    fastMode: false,
    effortValue: 'high',
  };
  return {
    abortController,
    agentId: 'hermetic_test_agent' as any,
    readFileState: { get: () => null, set: () => {}, has: () => false, delete: () => {} } as any,
    options: {
      tools: extraTools,
      mcpClients: [],
      mainLoopModel: 'claude-3-7-sonnet-20250219',
      thinkingConfig: { mode: 'off' },
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

describe('Phase 5.4: Tool Execution Round-Trip Test Matrix', () => {
  test('Scenario 1: Single tool round-trip (Brain tool_use -> Claude Tool.call -> ToolResult -> Brain final text)', async () => {
    let callCount = 0;
    const receivedRequests: BrainGenerationRequest[] = [];
    let toolExecuted = false;

    const mockTool = createDummyTool('CommandTool', async (input) => {
      toolExecuted = true;
      return `Executed: ${input.command}`;
    });

    const brainClient = new MockBrainBackendClient(async function* (request) {
      callCount++;
      receivedRequests.push(request);

      if (callCount === 1) {
        // Assert tools were forwarded in the Brain request
        expect(request.tools?.length).toBeGreaterThanOrEqual(1);
        expect(request.tools?.some((t) => t.name === 'CommandTool')).toBe(true);

        // Emit tool call
        yield {
          type: 'tool_use',
          toolUse: {
            id: 'call_cmd_001',
            name: 'CommandTool',
            input: { command: 'echo hello_brain' },
          },
        };
        yield { type: 'finished' };
      } else if (callCount === 2) {
        // Turn 2: Verify tool_result is in message history
        const lastMsg: any = request.messages[request.messages.length - 1];
        expect(lastMsg.role).toBe('user');
        expect(Array.isArray(lastMsg.content)).toBe(true);
        expect(lastMsg.content[0].type).toBe('tool_result');
        expect(lastMsg.content[0].content).toContain('Executed: echo hello_brain');

        yield { type: 'token', token: 'Command succeeded with output: hello_brain' };
        yield { type: 'finished' };
      }
    });

    const brainCallModel = createBrainCallModel(brainClient);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: brainCallModel,
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Run command' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(new AbortController(), [mockTool]),
      querySource: 'repl',
      deps,
    });

    let finalAssistantMsg: AssistantMessage | null = null;
    for await (const event of stream) {
      if ((event as any).type === 'assistant') {
        finalAssistantMsg = event as AssistantMessage;
      }
    }

    expect(toolExecuted).toBe(true);
    expect(callCount).toBe(2);
    expect(finalAssistantMsg).not.toBeNull();
    expect(finalAssistantMsg!.message.content[0].text).toBe('Command succeeded with output: hello_brain');
  });

  test('Scenario 2: Tool requiring permission (Approved flow)', async () => {
    let permissionRequested = false;
    const sensitiveTool = createDummyTool('BashMutating', async () => 'mutation_applied');
    sensitiveTool.needsPermissions = () => true;

    let callCount = 0;
    const brainClient = new MockBrainBackendClient(async function* () {
      callCount++;
      if (callCount === 1) {
        yield {
          type: 'tool_use',
          toolUse: { id: 'call_mut_001', name: 'BashMutating', input: { command: 'rm -rf /tmp/test' } },
        };
        yield { type: 'finished' };
      } else {
        yield { type: 'token', token: 'Mutation confirmed.' };
        yield { type: 'finished' };
      }
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Delete temp file' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => {
        permissionRequested = true;
        return { behavior: 'allow' };
      },
      toolUseContext: createMockToolUseContext(new AbortController(), [sensitiveTool]),
      querySource: 'repl',
      deps,
    });

    for await (const _ of stream) {}

    expect(permissionRequested).toBe(true);
    expect(callCount).toBe(2);
  });

  test('Scenario 3: Tool rejection (User denies permission -> Brain gracefully handles rejection)', async () => {
    const sensitiveTool = createDummyTool('DangerousTool', async () => 'should_not_run');
    sensitiveTool.needsPermissions = () => true;

    let callCount = 0;
    let toolResultIsError = false;

    const brainClient = new MockBrainBackendClient(async function* (request) {
      callCount++;
      if (callCount === 1) {
        yield {
          type: 'tool_use',
          toolUse: { id: 'call_dang_001', name: 'DangerousTool', input: {} },
        };
        yield { type: 'finished' };
      } else {
        const lastMsg: any = request.messages[request.messages.length - 1];
        if (Array.isArray(lastMsg.content)) {
          toolResultIsError = lastMsg.content[0].is_error === true;
        }
        yield { type: 'token', token: 'Operation was cancelled per your request.' };
        yield { type: 'finished' };
      }
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Run dangerous action' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'deny', message: 'Permission denied by user' }),
      toolUseContext: createMockToolUseContext(new AbortController(), [sensitiveTool]),
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
    expect(toolResultIsError).toBe(true);
    expect(finalMsg).not.toBeNull();
    expect(finalMsg!.message.content[0].text).toBe('Operation was cancelled per your request.');
  });

  test('Scenario 4: Tool execution failure (Tool.call throws -> error ToolResultBlock -> Brain explains failure)', async () => {
    const failingTool = createDummyTool('FailingTool', async () => {
      throw new Error('Disk full error (ENOSPC)');
    });

    let callCount = 0;
    let receivedErrorInBrain = false;

    const brainClient = new MockBrainBackendClient(async function* (request) {
      callCount++;
      if (callCount === 1) {
        yield {
          type: 'tool_use',
          toolUse: { id: 'call_fail_001', name: 'FailingTool', input: {} },
        };
        yield { type: 'finished' };
      } else {
        const lastMsg: any = request.messages[request.messages.length - 1];
        if (Array.isArray(lastMsg.content)) {
          receivedErrorInBrain = lastMsg.content[0].is_error === true || lastMsg.content[0].content.includes('Disk full');
        }
        yield { type: 'token', token: 'I encountered an error: Disk full. Please free space.' };
        yield { type: 'finished' };
      }
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Run failing tool' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(new AbortController(), [failingTool]),
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
    expect(receivedErrorInBrain).toBe(true);
    expect(finalMsg!.message.content[0].text).toContain('Disk full');
  });

  test('Scenario 5: Multiple tool calls in a single turn', async () => {
    const toolA = createDummyTool('ToolA', async () => 'ResultA');
    const toolB = createDummyTool('ToolB', async () => 'ResultB');

    let callCount = 0;
    let totalToolResults = 0;

    const brainClient = new MockBrainBackendClient(async function* (request) {
      callCount++;
      if (callCount === 1) {
        // Emit 2 tool calls in the same turn
        yield {
          type: 'tool_use',
          toolUse: { id: 'call_a', name: 'ToolA', input: {} },
        };
        yield {
          type: 'tool_use',
          toolUse: { id: 'call_b', name: 'ToolB', input: {} },
        };
        yield { type: 'finished' };
      } else {
        const userMessages = request.messages.filter((m) => m.role === 'user');
        for (const u of userMessages) {
          if (Array.isArray(u.content)) {
            totalToolResults += u.content.filter((b: any) => b.type === 'tool_result').length;
          }
        }
        yield { type: 'token', token: 'Combined both ToolA and ToolB results.' };
        yield { type: 'finished' };
      }
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Run both tools' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(new AbortController(), [toolA, toolB]),
      querySource: 'repl',
      deps,
    });

    for await (const _ of stream) {}

    expect(callCount).toBe(2);
    expect(totalToolResults).toBe(2);
  });

  test('Scenario 6: Tool call interleaved with streaming text', async () => {
    const testTool = createDummyTool('InfoTool', async () => 'Data 42');

    let callCount = 0;
    const textDeltas: string[] = [];

    const brainClient = new MockBrainBackendClient(async function* () {
      callCount++;
      if (callCount === 1) {
        yield { type: 'token', token: 'Fetching information...' };
        yield {
          type: 'tool_use',
          toolUse: { id: 'call_info', name: 'InfoTool', input: {} },
        };
        yield { type: 'finished' };
      } else {
        yield { type: 'token', token: 'The retrieved data is: Data 42.' };
        yield { type: 'finished' };
      }
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Fetch data' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(new AbortController(), [testTool]),
      querySource: 'repl',
      deps,
    });

    for await (const event of stream) {
      if (
        (event as any).type === 'stream_event' &&
        (event as any).event?.type === 'content_block_delta' &&
        (event as any).event.delta?.type === 'text_delta'
      ) {
        textDeltas.push((event as any).event.delta.text);
      }
    }

    expect(callCount).toBe(2);
    expect(textDeltas).toContain('Fetching information...');
    expect(textDeltas).toContain('The retrieved data is: Data 42.');
  });

  test('Scenario 7: Cancellation during tool lifecycle cleans up state', async () => {
    const ac = new AbortController();
    const slowTool = createDummyTool('SlowTool', async () => {
      // Simulate user hit Ctrl+C during tool execution
      ac.abort();
      return 'slow_result';
    });

    let callCount = 0;
    const brainClient = new MockBrainBackendClient(async function* () {
      callCount++;
      yield {
        type: 'tool_use',
        toolUse: { id: 'call_slow', name: 'SlowTool', input: {} },
      };
      yield { type: 'finished' };
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Run slow tool' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(ac, [slowTool]),
      querySource: 'repl',
      deps,
    });

    for await (const _ of stream) {}

    expect(ac.signal.aborted).toBe(true);
    expect(callCount).toBe(1); // Did not dispatch subsequent turn after abort
  });

  test('Scenario 8: Multi-turn conversation maintaining full tool context across turns', async () => {
    const tool = createDummyTool('MathTool', async (input) => `MathResult(${input.expr})`);

    let totalCallModelInvocations = 0;
    const messagesPerTurn: number[] = [];

    const brainClient = new MockBrainBackendClient(async function* (request) {
      totalCallModelInvocations++;
      messagesPerTurn.push(request.messages.length);

      if (totalCallModelInvocations === 1) {
        // Turn 1 initial: call tool
        yield {
          type: 'tool_use',
          toolUse: { id: 'call_math_1', name: 'MathTool', input: { expr: '2+2' } },
        };
        yield { type: 'finished' };
      } else if (totalCallModelInvocations === 2) {
        // Turn 1 follow-up: finish
        yield { type: 'token', token: 'Calculated 4.' };
        yield { type: 'finished' };
      } else if (totalCallModelInvocations === 3) {
        // Turn 2 follow-up user prompt
        yield { type: 'token', token: 'Previous answer was 4.' };
        yield { type: 'finished' };
      }
    });

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(brainClient),
    };

    const conversation: any[] = [createUserMessage({ content: 'What is 2+2?' })];

    // Turn 1
    for await (const event of query({
      messages: conversation,
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(new AbortController(), [tool]),
      querySource: 'repl',
      deps,
    })) {
      if ((event as any).type === 'assistant' || (event as any).type === 'user') {
        conversation.push(event);
      }
    }

    // Turn 2
    conversation.push(createUserMessage({ content: 'What did you just compute?' }));
    for await (const event of query({
      messages: conversation,
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(new AbortController(), [tool]),
      querySource: 'repl',
      deps,
    })) {
      if ((event as any).type === 'assistant') {
        conversation.push(event);
      }
    }

    expect(totalCallModelInvocations).toBe(3);
    // Turn 1 initial: 1 msg. Turn 1 tool result: 3 msgs. Turn 2: 5 msgs.
    expect(messagesPerTurn[0]).toBe(1);
    expect(messagesPerTurn[1]).toBe(3);
    expect(messagesPerTurn[2]).toBe(5);
  });
});
